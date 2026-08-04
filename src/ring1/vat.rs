//! # AliluOS In-Memory Virtual Address Table & Demand Paging Engine (`ring1/vat.rs`)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::config::storage::*;
use crate::disk::{DISK, DiskBlock};
use crate::btree::{BTREE, BPlusEntryMeta};

#[derive(Clone, Debug)]
pub struct VatEntry {
    pub file_id: usize,
    pub path: String,
    pub size: u64,
    pub data_blocks: Vec<u64>,
    pub is_loaded: bool,
    pub is_dirty: bool,
}

#[derive(Clone)]
pub struct PageCache {
    pub block_id: u64,
    pub data: DiskBlock,
    pub dirty: bool,
}

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

    pub fn get_or_register_entry(&mut self, path: &str) -> Option<&mut VatEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            return Some(&mut self.entries[pos]);
        }

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

        if (meta.size as usize) < content.len() {
            content.truncate(meta.size as usize);
        }

        String::from_utf8(content).map_err(|_| "Failed to decode file content UTF-8")
    }

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

        if let Some(ref meta) = existing_meta {
            for &blk in &meta.data_blocks {
                if allocated_blocks.len() < needed_blocks {
                    allocated_blocks.push(blk);
                } else {
                    disk.free_block(blk);
                }
            }
        }

        while allocated_blocks.len() < needed_blocks {
            let new_blk = disk.allocate_block()?;
            allocated_blocks.push(new_blk);
        }

        for (i, &blk_id) in allocated_blocks.iter().enumerate() {
            let start = i * DISK_BLOCK_SIZE;
            let end = (start + DISK_BLOCK_SIZE).min(text_bytes.len());
            let mut block_buf = [0u8; DISK_BLOCK_SIZE];
            block_buf[0..(end - start)].copy_from_slice(&text_bytes[start..end]);
            let _ = disk.write_block(blk_id, &block_buf);
        }

        let new_meta = BPlusEntryMeta {
            is_directory: false,
            size: total_size,
            created_at: ticks,
            data_blocks: allocated_blocks,
        };
        
        btree.insert(path, new_meta)?;
        Ok(())
    }

    pub fn delete_entry(&mut self, path: &str) -> Result<(), &'static str> {
        let mut btree = BTREE.lock();
        if let Some(meta) = btree.search(path) {
            let mut disk = DISK.lock();
            for &blk_id in &meta.data_blocks {
                disk.free_block(blk_id);
            }
            btree.remove(path)?;
            self.entries.retain(|e| e.path != path);
            Ok(())
        } else {
            Err("Target path not found")
        }
    }
}

pub static VAT: crate::vga::Locked<VirtualAddressTable> = crate::vga::Locked::new(VirtualAddressTable::new());
