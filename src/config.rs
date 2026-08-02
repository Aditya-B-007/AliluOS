//! # AliluOS Architecture & Hardware Configuration (`config.rs`)
//!
//! - **WHAT**: Centralized hardware, architecture, port addresses, memory layouts, and filesystem constants.
//! - **WHY**: Isolates chip/platform specifics to facilitate easy porting across different CPU architectures (x86_64, ARM, RISC-V).
//! - **WHEN**: Referenced by kernel hardware drivers (`vga`, `keyboard`, `interrupts`, `allocator`, `network`), secondary disk drivers (`disk`), B+ tree engine (`btree`), and RAM Virtual Address Table (`vat`).

#![allow(dead_code)]

/// Storage & B+ Tree Filesystem Architecture Configuration
pub mod storage {
    /// On-disk B+ tree block size (1024 bytes = 1 KiB per block)
    pub const DISK_BLOCK_SIZE: usize = 1024;
    /// Standard ATA Sector Size in bytes
    pub const SECTOR_SIZE: usize = 512;
    /// Number of ATA sectors per 1024-byte filesystem block
    pub const SECTORS_PER_BLOCK: usize = DISK_BLOCK_SIZE / SECTOR_SIZE;
    /// Superblock sector/block ID at disk offset 0
    pub const SUPERBLOCK_BLOCK_ID: u64 = 0;
    /// Superblock magic identifier signature ("ALILUOS1")
    pub const SUPERBLOCK_MAGIC: u64 = 0x414C494C554F5331;
    /// Total block capacity of disk storage volume (e.g. 4096 blocks = 4 MiB volume)
    pub const TOTAL_DISK_BLOCKS: usize = 4096;
    /// Maximum children/keys branching factor per B+ Tree node
    pub const BTREE_MAX_KEYS: usize = 8;
    /// Maximum filename key length in bytes
    pub const MAX_KEY_LEN: usize = 64;
    /// Maximum active open file entries in RAM Virtual Address Table
    pub const MAX_VAT_ENTRIES: usize = 64;
    /// Page Cache capacity in RAM Virtual Address Table
    pub const PAGE_CACHE_CAPACITY: usize = 32;
}

/// ATA / IDE Disk Controller Hardware Ports
pub mod ata {
    pub const PRIMARY_ATA_DATA: u16 = 0x1F0;
    pub const PRIMARY_ATA_ERROR: u16 = 0x1F1;
    pub const PRIMARY_ATA_SECTOR_COUNT: u16 = 0x1F2;
    pub const PRIMARY_ATA_LBA_LOW: u16 = 0x1F3;
    pub const PRIMARY_ATA_LBA_MID: u16 = 0x1F4;
    pub const PRIMARY_ATA_LBA_HIGH: u16 = 0x1F5;
    pub const PRIMARY_ATA_DEVICE_SELECT: u16 = 0x1F6;
    pub const PRIMARY_ATA_COMMAND_STATUS: u16 = 0x1F7;
    pub const PRIMARY_ATA_CONTROL: u16 = 0x3F6;
}

/// Memory & Heap Architecture Parameters
pub mod memory {
    pub const HEAP_SIZE: usize = 5 * 1024 * 1024; // 5 MiB
}

/// VGA Text Display Hardware Ports and Memory Address
pub mod vga {
    pub const BUFFER_ADDRESS: usize = 0xB8000;
    pub const BUFFER_WIDTH: usize = 80;
    pub const BUFFER_HEIGHT: usize = 25;
    pub const CRTC_ADDR_PORT: u16 = 0x3D4;
    pub const CRTC_DATA_PORT: u16 = 0x3D5;
}

/// Keyboard Hardware PS/2 Ports
pub mod keyboard {
    pub const DATA_PORT: u16 = 0x60;
    pub const STATUS_PORT: u16 = 0x64;
}

/// Interrupt Controller (PIC/PIT) Ports and Parameters
pub mod interrupts {
    pub const PIC1_COMMAND: u16 = 0x20;
    pub const PIC1_DATA: u16 = 0x21;
    pub const PIC2_COMMAND: u16 = 0xA0;
    pub const PIC2_DATA: u16 = 0xA1;
    pub const PIT_COMMAND: u16 = 0x43;
    pub const PIT_DATA0: u16 = 0x40;
    pub const DEBUG_EXIT_PORT: u16 = 0xF4;
}

/// PCI & Network Subsystem Ports
pub mod network {
    pub const PCI_CONFIG_ADDR: u16 = 0xCF8;
    pub const PCI_CONFIG_DATA: u16 = 0xCFC;
}
