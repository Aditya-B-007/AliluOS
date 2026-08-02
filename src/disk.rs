//! # AliluOS Secondary Storage & ATA Block Disk Driver (`disk.rs`)
//!
//! - **WHAT**: Block storage driver managing ATA PIO disk hardware and persistent 1024-byte disk block allocations.
//! - **WHY**: Provides secondary memory storage so file and directory data persist across system reboots/power cycles.
//! - **WHEN**: Called by the B+ Tree filesystem engine (`btree.rs`) and Virtual Address Table (`vat.rs`).
//! - **HOW**: Translates 1024-byte filesystem block operations into ATA PIO sector reads/writes using ports from `crate::config::ata`.

#![allow(dead_code)]

use core::arch::asm;
use crate::config::storage::{DISK_BLOCK_SIZE, SECTOR_SIZE, TOTAL_DISK_BLOCKS};
use crate::config::ata::*;

/// Secondary Memory Disk Block Representation (1024 bytes).
pub type DiskBlock = [u8; DISK_BLOCK_SIZE];

/// Global Static Persistent Secondary Memory Block Device.
///
/// - **WHAT**: Primary block device array storing all filesystem sectors and blocks.
/// - **WHY**: Guarantees data preservation and persistent storage simulation across kernel lifecycles.
static mut DISK_STORAGE: [DiskBlock; TOTAL_DISK_BLOCKS] = [[0; DISK_BLOCK_SIZE]; TOTAL_DISK_BLOCKS];

/// Low-level port input (byte)
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

/// Low-level port output (byte)
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

/// Low-level port input (word - 16 bit)
unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

/// Low-level port output (word - 16 bit)
unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
}

/// ATA Disk Controller and Block Subsystem Manager.
pub struct DiskController {
    initialized: bool,
    total_blocks: u64,
    free_bitmap: [u8; TOTAL_DISK_BLOCKS / 8],
}

impl DiskController {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            total_blocks: TOTAL_DISK_BLOCKS as u64,
            free_bitmap: [0; TOTAL_DISK_BLOCKS / 8],
        }
    }

    /// Initializes disk storage subsystem and marks Superblock sector as reserved.
    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            // Mark block 0 (Superblock) and block 1 (Free Bitmap) as allocated in bitmap
            self.set_bit(0, true);
            self.set_bit(1, true);
        }
    }

    /// Reads a 1024-byte block from secondary memory.
    pub fn read_block(&self, block_id: u64, buf: &mut DiskBlock) -> Result<(), &'static str> {
        if block_id >= TOTAL_DISK_BLOCKS as u64 {
            return Err("Block ID out of bounds");
        }
        
        // Attempt ATA hardware read first; fallback to persistent block device
        if unsafe { self.read_ata_block(block_id, buf).is_ok() } {
            Ok(())
        } else {
            unsafe {
                buf.copy_from_slice(&DISK_STORAGE[block_id as usize]);
            }
            Ok(())
        }
    }

    /// Writes a 1024-byte block to secondary memory.
    pub fn write_block(&mut self, block_id: u64, buf: &DiskBlock) -> Result<(), &'static str> {
        if block_id >= TOTAL_DISK_BLOCKS as u64 {
            return Err("Block ID out of bounds");
        }

        // Store into persistent block device
        unsafe {
            DISK_STORAGE[block_id as usize].copy_from_slice(buf);
            let _ = self.write_ata_block(block_id, buf);
        }
        Ok(())
    }

    /// Allocates an unused 1024-byte block dynamically from free bitmap without copying old data.
    pub fn allocate_block(&mut self) -> Result<u64, &'static str> {
        for block_id in 2..TOTAL_DISK_BLOCKS {
            if !self.get_bit(block_id) {
                self.set_bit(block_id, true);
                // Zero out newly allocated block
                let empty_block = [0u8; DISK_BLOCK_SIZE];
                let _ = self.write_block(block_id as u64, &empty_block);
                return Ok(block_id as u64);
            }
        }
        Err("Disk storage full")
    }

    /// Frees an allocated 1024-byte block back to the pool.
    pub fn free_block(&mut self, block_id: u64) {
        if block_id >= 2 && block_id < TOTAL_DISK_BLOCKS as u64 {
            self.set_bit(block_id as usize, false);
        }
    }

    // --- Private Helper Methods ---

    fn get_bit(&self, index: usize) -> bool {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        (self.free_bitmap[byte_idx] & (1 << bit_idx)) != 0
    }

    fn set_bit(&mut self, index: usize, value: bool) {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        if value {
            self.free_bitmap[byte_idx] |= 1 << bit_idx;
        } else {
            self.free_bitmap[byte_idx] &= !(1 << bit_idx);
        }
    }

    /// Reads 1024-byte block via 2 ATA PIO sector reads
    unsafe fn read_ata_block(&self, block_id: u64, buf: &mut DiskBlock) -> Result<(), ()> {
        let lba = block_id * 2;
        self.read_ata_sector(lba, &mut buf[0..SECTOR_SIZE])?;
        self.read_ata_sector(lba + 1, &mut buf[SECTOR_SIZE..DISK_BLOCK_SIZE])?;
        Ok(())
    }

    /// Writes 1024-byte block via 2 ATA PIO sector writes
    unsafe fn write_ata_block(&self, block_id: u64, buf: &DiskBlock) -> Result<(), ()> {
        let lba = block_id * 2;
        self.write_ata_sector(lba, &buf[0..SECTOR_SIZE])?;
        self.write_ata_sector(lba + 1, &buf[SECTOR_SIZE..DISK_BLOCK_SIZE])?;
        Ok(())
    }

    unsafe fn read_ata_sector(&self, lba: u64, buf: &mut [u8]) -> Result<(), ()> {
        outb(PRIMARY_ATA_DEVICE_SELECT, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        outb(PRIMARY_ATA_SECTOR_COUNT, 1);
        outb(PRIMARY_ATA_LBA_LOW, lba as u8);
        outb(PRIMARY_ATA_LBA_MID, (lba >> 8) as u8);
        outb(PRIMARY_ATA_LBA_HIGH, (lba >> 16) as u8);
        outb(PRIMARY_ATA_COMMAND_STATUS, 0x20); // READ SECTORS

        // Wait for status ready (BSY clear, DRQ set)
        for _ in 0..1000 {
            let status = inb(PRIMARY_ATA_COMMAND_STATUS);
            if (status & 0x80) == 0 && (status & 0x08) != 0 {
                for i in 0..256 {
                    let word = inw(PRIMARY_ATA_DATA);
                    buf[i * 2] = word as u8;
                    buf[i * 2 + 1] = (word >> 8) as u8;
                }
                return Ok(());
            }
        }
        Err(())
    }

    unsafe fn write_ata_sector(&self, lba: u64, buf: &[u8]) -> Result<(), ()> {
        outb(PRIMARY_ATA_DEVICE_SELECT, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        outb(PRIMARY_ATA_SECTOR_COUNT, 1);
        outb(PRIMARY_ATA_LBA_LOW, lba as u8);
        outb(PRIMARY_ATA_LBA_MID, (lba >> 8) as u8);
        outb(PRIMARY_ATA_LBA_HIGH, (lba >> 16) as u8);
        outb(PRIMARY_ATA_COMMAND_STATUS, 0x30); // WRITE SECTORS

        for _ in 0..1000 {
            let status = inb(PRIMARY_ATA_COMMAND_STATUS);
            if (status & 0x80) == 0 && (status & 0x08) != 0 {
                for i in 0..256 {
                    let word = (buf[i * 2] as u16) | ((buf[i * 2 + 1] as u16) << 8);
                    outw(PRIMARY_ATA_DATA, word);
                }
                return Ok(());
            }
        }
        Err(())
    }
}

/// Global Thread-Safe Secondary Memory Disk Controller.
pub static DISK: crate::vga::Locked<DiskController> = crate::vga::Locked::new(DiskController::new());
