//! # AliluOS Architecture & Hardware Configuration (`ring0/config.rs`)
//!
//! - **WHAT**: Centralized hardware, architecture, port addresses, privilege ring selectors, and memory parameters.
//! - **WHY**: Isolates chip/platform specifics and segment privilege levels across Ring 0, Ring 1, and Ring 2.
//! - **WHEN**: Referenced by kernel drivers, Process Resource Manager, threads, scheduler, disk, B+ tree, and VAT.

#![allow(dead_code)]

/// x86_64 Segment Selectors & Privilege Ring Architecture
pub mod gdt {
    pub const KERNEL_CODE_SEL: u16 = 0x08; // Ring 0, DPL=0
    pub const KERNEL_DATA_SEL: u16 = 0x10; // Ring 0, DPL=0
    pub const DRIVER_CODE_SEL: u16 = 0x1B; // Ring 1, DPL=1 (0x18 | 3... 0x18 | 1 = 0x19, DPL=1)
    pub const DRIVER_DATA_SEL: u16 = 0x23; // Ring 1, DPL=1
    pub const USER_CODE_SEL:   u16 = 0x2B; // Ring 2, DPL=2 (0x28 | 2 = 0x2A/0x2B)
    pub const USER_DATA_SEL:   u16 = 0x33; // Ring 2, DPL=2
    pub const TSS_SEL:         u16 = 0x3B; // Task State Segment
    pub const SYSCALL_INT_VECTOR: u8 = 0x80;
}

/// Single-Process Resource Management Architecture Parameters
pub mod process {
    /// Maximum number of active threads managed by the single kernel process
    pub const MAX_THREADS: usize = 16;
    /// Dedicated stack size per kernel thread (64 KiB)
    pub const THREAD_STACK_SIZE: usize = 64 * 1024;
    /// Primary RAM Memory Quota default limit (4 MiB)
    pub const RAM_QUOTA_BYTES: usize = 4 * 1024 * 1024;
    /// Secondary Disk Storage Quota default limit (2 MiB)
    pub const DISK_QUOTA_BYTES: usize = 2 * 1024 * 1024;
    /// Network Bandwidth Rate Limit (1 MB/s)
    pub const NET_BANDWIDTH_LIMIT_BYTES_PER_SEC: u64 = 1_000_000;
}

/// Preemptive Multi-Threaded Scheduler Parameters
pub mod scheduler {
    /// Scheduling Time Slice Quantum in PIT timer ticks (10 ticks = 100 ms)
    pub const TIME_SLICE_TICKS: u64 = 10;
    /// Default thread priority level
    pub const DEFAULT_PRIORITY: u8 = 1;
}

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
    /// Total block capacity of disk storage volume (4096 blocks = 4 MiB volume)
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
