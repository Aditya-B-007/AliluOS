//! # AliluOS Secondary Storage & ATA Block Disk Driver (`ring1/disk.rs`)

#![allow(dead_code)]

use core::arch::asm;
use crate::config::storage::{DISK_BLOCK_SIZE, SECTOR_SIZE, TOTAL_DISK_BLOCKS};
use crate::config::ata::*;

pub type DiskBlock = [u8; DISK_BLOCK_SIZE];

static mut DISK_STORAGE: [DiskBlock; TOTAL_DISK_BLOCKS] = [[0; DISK_BLOCK_SIZE]; TOTAL_DISK_BLOCKS];

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
}

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

    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.set_bit(0, true);
            self.set_bit(1, true);
        }
    }

    pub fn read_block(&self, block_id: u64, buf: &mut DiskBlock) -> Result<(), &'static str> {
        if block_id >= TOTAL_DISK_BLOCKS as u64 {
            return Err("Block ID out of bounds");
        }
        
        if unsafe { self.read_ata_block(block_id, buf).is_ok() } {
            Ok(())
        } else {
            unsafe {
                buf.copy_from_slice(&DISK_STORAGE[block_id as usize]);
            }
            Ok(())
        }
    }

    pub fn write_block(&mut self, block_id: u64, buf: &DiskBlock) -> Result<(), &'static str> {
        if block_id >= TOTAL_DISK_BLOCKS as u64 {
            return Err("Block ID out of bounds");
        }

        unsafe {
            DISK_STORAGE[block_id as usize].copy_from_slice(buf);
            let _ = self.write_ata_block(block_id, buf);
        }
        Ok(())
    }

    pub fn allocate_block(&mut self) -> Result<u64, &'static str> {
        for block_id in 2..TOTAL_DISK_BLOCKS {
            if !self.get_bit(block_id) {
                self.set_bit(block_id, true);
                let empty_block = [0u8; DISK_BLOCK_SIZE];
                let _ = self.write_block(block_id as u64, &empty_block);
                return Ok(block_id as u64);
            }
        }
        Err("Disk storage full")
    }

    pub fn free_block(&mut self, block_id: u64) {
        if block_id >= 2 && block_id < TOTAL_DISK_BLOCKS as u64 {
            self.set_bit(block_id as usize, false);
        }
    }

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

    unsafe fn read_ata_block(&self, block_id: u64, buf: &mut DiskBlock) -> Result<(), ()> {
        let lba = block_id * 2;
        self.read_ata_sector(lba, &mut buf[0..SECTOR_SIZE])?;
        self.read_ata_sector(lba + 1, &mut buf[SECTOR_SIZE..DISK_BLOCK_SIZE])?;
        Ok(())
    }

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
        outb(PRIMARY_ATA_COMMAND_STATUS, 0x20);

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
        outb(PRIMARY_ATA_COMMAND_STATUS, 0x30);

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

pub static DISK: crate::vga::Locked<DiskController> = crate::vga::Locked::new(DiskController::new());
