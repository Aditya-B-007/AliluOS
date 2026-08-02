//! # AliluOS In-Memory Virtual Address Table & Demand Paging Engine (`vat.rs`)
//!
//! - **WHAT**: Virtual Address Table (VAT) maintained in RAM tracking open files, virtual block offsets, and page caches.
//! - **WHY**: Fulfills Requirement 2: dynamic non-contiguous disk block allocation without CPU-wasting contiguous block copies,
//!   combined with on-demand page loading into RAM when requested by the user.
//! - **WHEN**: Consulted during `read`, `write`, `create`, `edit`, and `delete` file operations.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::config::storage::*;
use crate::disk::{DISK, DiskBlock};
use crate::btree::{BTREE, BPlusEntryMeta};

/// Virtual Address Table Entry in RAM.
#[derive(Clone, Debug)]
pub struct VatEntry {
    pub file_id: usize,
    pub path: String,
    pub size: u64,
    pub data_blocks: Vec<u64>, // List of physical 1024-byte disk block IDs
    pub is_loaded: bool,
    pub is_dirty: bool,
}

/// Cached RAM Page loaded on-demand from disk.
#[derive(Clone)]
pub struct PageCache {
    pub block_id: u64,
    pub data: DiskBlock,
    pub dirty: bool,
}

/// In-Memory Virtual Address Table Subsystem.
pub struct VirtualAddressTable {
    pub entries: Vec<VatEntry>,
    pub cache: Vec<PageCache>,
    pub next_file_id: usize,
}

impl VirtualAddressTable {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            cache: Vec::new(),
            next_file_id: 1,
        }
    }

    /// Registers a new file or retrieves an existing Virtual Address Table entry.
    pub fn get_or_register_entry(&mut self, path: &str) -> Option<&mut VatEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            return Some(&mut self.entries[pos]);
        }

        // Fetch B+ Tree metadata from disk
        let btree = BTREE.lock();
        if let Some(meta) = btree.search(path) {
            let id = self.next_file_id;
            self.next_file_id += 1;
            self.entries.push(VatEntry {
                file_id: id,
                path: String::from(path),
                size: meta.size,
                data_blocks: meta.data_blocks,
                is_loaded: false,
                is_dirty: false,
            });
            let idx = self.entries.len() - 1;
            Some(&mut self.entries[idx])
        } else {
            None
        }
    }

    /// Reads file data on-demand from secondary memory into RAM page cache.
    pub fn read_file_on_demand(&mut self, path: &str) -> Result<String, &'static str> {
        let btree = BTREE.lock();
        let meta = btree.search(path).ok_or("File not found in B+ Tree index")?;
        if meta.is_directory {
            return Err("Target is a directory");
        }
        drop(btree);

        let mut content = Vec::new();
        let mut disk = DISK.lock();

        for &block_id in &meta.data_blocks {
            let mut block = [0u8; DISK_BLOCK_SIZE];
            if disk.read_block(block_id, &mut block).is_ok() {
                content.extend_from_slice(&block);
            }
        }

        // Truncate to exact recorded file size
        if (meta.size as usize) < content.len() {
            content.truncate(meta.size as usize);
        }

        String::from_utf8(content).map_err(|_| "Failed to decode file content UTF-8")
    }

    /// Writes file payload by dynamically allocating non-contiguous 1024-byte blocks.
    ///
    /// - **Dynamic Non-Contiguous Expansion**: Allocates new disk blocks as payload grows,
    ///   appending block IDs to VAT and B+ tree without moving or re-copying existing blocks.
    pub fn write_file_dynamic(&mut self, path: &str, text: &str, ticks: u64) -> Result<(), &'static str> {
        let text_bytes = text.as_bytes();
        let total_size = text_bytes.len() as u64;

        let needed_blocks = if total_size == 0 {
            0
        } else {
            ((total_size as usize + DISK_BLOCK_SIZE - 1) / DISK_BLOCK_SIZE)
        };

        let mut btree = BTREE.lock();
        let existing_meta = btree.search(path);

        let mut disk = DISK.lock();
        let mut allocated_blocks = Vec::new();

        // Reuse previously allocated blocks or allocate new ones
        if let Some(ref meta) = existing_meta {
            for &blk in &meta.data_blocks {
                if allocated_blocks.len() < needed_blocks {
                    allocated_blocks.push(blk);
                } else {
                    disk.free_block(blk);
                }
            }
        }

        // Allocate additional non-contiguous blocks if needed
        while allocated_blocks.len() < needed_blocks {
            let new_blk = disk.allocate_block()?;
            allocated_blocks.push(new_blk);
        }

        // Write slice chunks into 1024-byte disk blocks
        for (i, &blk_id) in allocated_blocks.iter().enumerate() {
            let start = i * DISK_BLOCK_SIZE;
            let end = (start + DISK_BLOCK_SIZE).min(text_bytes.len());
            let mut block_buf = [0u8; DISK_BLOCK_SIZE];
            block_buf[0..(end - start)].copy_from_slice(&text_bytes[start..end]);
            let _ = disk.write_block(blk_id, &block_buf);
        }

        // Commit updated metadata into B+ Tree index on disk
        let new_meta = BPlusEntryMeta {
            is_directory: false,
            size: total_size,
            created_at: ticks,
            data_blocks: allocated_blocks,
        };
        
        btree.insert(path, new_meta)?;
        Ok(())
    }

    /// Removes file entry and frees physical 1024-byte blocks.
    pub fn delete_entry(&mut self, path: &str) -> Result<(), &'static str> {
        let mut btree = BTREE.lock();
        if let Some(meta) = btree.search(path) {
            let mut disk = DISK.lock();
            for &blk_id in &meta.data_blocks {
                disk.free_block(blk_id);
            }
            btree.remove(path)?;
            
            // Remove from VAT RAM entries
            self.entries.retain(|e| e.path != path);
            Ok(())
        } else {
            Err("Target path not found")
        }
    }
}

/// Global Thread-Safe Instance of the Virtual Address Table.
pub static VAT: crate::vga::Locked<VirtualAddressTable> = crate::vga::Locked::new(VirtualAddressTable::new());
