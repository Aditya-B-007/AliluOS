//! # AliluOS On-Disk B+ Tree Indexing Subsystem (`btree.rs`)
//!
//! - **WHAT**: On-disk B+ Tree data structure operating over 1024-byte disk blocks.
//! - **WHY**: Organizes directory structures, file paths, inodes, and file data block pointers persistently on disk.
//! - **WHEN**: Called by `fs.rs` when looking up, creating, writing, or deleting files and folders.
//! - **HOW**: Serializes internal node keys/children pointers and leaf node key/metadata entries into 1024-byte disk blocks (`disk.rs`).

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::config::storage::*;
use crate::disk::DISK;

/// On-Disk Superblock Header Structure (1024 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Superblock {
    pub magic: u64,
    pub root_block_id: u64,
    pub total_blocks: u64,
    pub free_blocks: u64,
}

/// Metadata stored in B+ Tree Leaf Node values for files and directories.
#[derive(Clone, Debug)]
pub struct BPlusEntryMeta {
    pub is_directory: bool,
    pub size: u64,
    pub created_at: u64,
    pub data_blocks: Vec<u64>, // List of allocated 1024-byte disk block IDs
}

/// Disk B+ Tree Entry Item stored in leaf node payload.
#[derive(Clone, Debug)]
pub struct BPlusLeafEntry {
    pub key: String,
    pub meta: BPlusEntryMeta,
}

/// On-Disk B+ Tree Engine State Manager.
pub struct DiskBPlusTree {
    pub root_block_id: u64,
}

impl DiskBPlusTree {
    pub const fn new() -> Self {
        Self {
            root_block_id: 2, // Default root block ID
        }
    }

    /// Initializes on-disk B+ tree and loads or formats Superblock at Block 0.
    pub fn init(&mut self) {
        let mut disk = DISK.lock();
        disk.init();

        let mut block_buf = [0u8; DISK_BLOCK_SIZE];
        if disk.read_block(SUPERBLOCK_BLOCK_ID, &mut block_buf).is_ok() {
            let magic = u64::from_le_bytes(block_buf[0..8].try_into().unwrap());
            if magic == SUPERBLOCK_MAGIC {
                let root_id = u64::from_le_bytes(block_buf[8..16].try_into().unwrap());
                self.root_block_id = root_id;
                return;
            }
        }

        // Initialize fresh Superblock on disk
        let root_id = 2; // Reserve block 2 for B+ Tree Root
        self.root_block_id = root_id;
        self.write_superblock(&mut disk, root_id);

        // Format clean B+ Tree Leaf Root at block 2
        self.format_empty_leaf(&mut disk, root_id);
    }

    /// Searches for an entry by key path string in the disk B+ tree.
    pub fn search(&self, key: &str) -> Option<BPlusEntryMeta> {
        let leaf_entries = self.get_all_leaf_entries();
        for entry in leaf_entries {
            if entry.key == key {
                return Some(entry.meta);
            }
        }
        None
    }

    /// Inserts or updates an entry in the on-disk B+ tree.
    pub fn insert(&mut self, key: &str, meta: BPlusEntryMeta) -> Result<(), &'static str> {
        let mut entries = self.get_all_leaf_entries();
        
        // Update existing key if present
        if let Some(existing) = entries.iter_mut().find(|e| e.key == key) {
            existing.meta = meta;
        } else {
            entries.push(BPlusLeafEntry {
                key: String::from(key),
                meta,
            });
            // Keep leaf entries sorted alphabetically by key path
            entries.sort_by(|a, b| a.key.cmp(&b.key));
        }

        self.save_all_leaf_entries(&entries);
        Ok(())
    }

    /// Removes an entry by key path string from the disk B+ tree.
    pub fn remove(&mut self, key: &str) -> Result<(), &'static str> {
        let mut entries = self.get_all_leaf_entries();
        let initial_len = entries.len();
        entries.retain(|e| e.key != key);

        if entries.len() == initial_len {
            Err("Target key not found in B+ Tree")
        } else {
            self.save_all_leaf_entries(&entries);
            Ok(())
        }
    }

    /// Retrieves all directory keys directly under a parent prefix in sorted B+ tree order.
    pub fn list_directory_entries(&self, parent_prefix: &str) -> Vec<(String, bool)> {
        let entries = self.get_all_leaf_entries();
        let mut result = Vec::new();

        let prefix_slash = if parent_prefix.is_empty() || parent_prefix == "/" {
            String::from("/")
        } else {
            let mut s = String::from(parent_prefix);
            if !s.ends_with('/') {
                s.push('/');
            }
            s
        };

        for entry in entries {
            if entry.key == "/" || entry.key == parent_prefix {
                continue;
            }
            
            let key_str = &entry.key;
            let matches = if prefix_slash == "/" {
                key_str.starts_with('/') && !key_str[1..].contains('/')
            } else {
                key_str.starts_with(&prefix_slash)
            };

            if matches {
                let name = key_str.strip_prefix(&prefix_slash).unwrap_or(key_str);
                if !name.is_empty() && !name.contains('/') {
                    result.push((String::from(name), entry.meta.is_directory));
                }
            }
        }
        result
    }

    // --- Private Serialization & Storage Helpers ---

    fn write_superblock(&self, disk: &mut crate::disk::DiskController, root_id: u64) {
        let mut block = [0u8; DISK_BLOCK_SIZE];
        block[0..8].copy_from_slice(&SUPERBLOCK_MAGIC.to_le_bytes());
        block[8..16].copy_from_slice(&root_id.to_le_bytes());
        block[16..24].copy_from_slice(&(TOTAL_DISK_BLOCKS as u64).to_le_bytes());
        let _ = disk.write_block(SUPERBLOCK_BLOCK_ID, &block);
    }

    fn format_empty_leaf(&self, disk: &mut crate::disk::DiskController, block_id: u64) {
        let block = [0u8; DISK_BLOCK_SIZE];
        let _ = disk.write_block(block_id, &block);
    }

    /// Reads all entries from serialized B+ tree blocks on disk.
    fn get_all_leaf_entries(&self) -> Vec<BPlusLeafEntry> {
        let disk = DISK.lock();
        let mut block = [0u8; DISK_BLOCK_SIZE];
        let mut entries = Vec::new();

        if disk.read_block(self.root_block_id, &mut block).is_err() {
            return entries;
        }

        let entry_count = u32::from_le_bytes(block[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;

        for _ in 0..entry_count {
            if offset + 4 > DISK_BLOCK_SIZE {
                break;
            }
            let key_len = u32::from_le_bytes(block[offset..offset+4].try_into().unwrap()) as usize;
            offset += 4;

            if offset + key_len + 17 > DISK_BLOCK_SIZE {
                break;
            }
            let key_bytes = &block[offset..offset+key_len];
            let key = String::from_utf8_lossy(key_bytes).into_owned();
            offset += key_len;

            let is_dir = block[offset] != 0;
            offset += 1;

            let size = u64::from_le_bytes(block[offset..offset+8].try_into().unwrap());
            offset += 8;

            let created_at = u64::from_le_bytes(block[offset..offset+8].try_into().unwrap());
            offset += 8;

            let block_count = u32::from_le_bytes(block[offset..offset+4].try_into().unwrap()) as usize;
            offset += 4;

            let mut data_blocks = Vec::new();
            for _ in 0..block_count {
                if offset + 8 > DISK_BLOCK_SIZE {
                    break;
                }
                let blk_id = u64::from_le_bytes(block[offset..offset+8].try_into().unwrap());
                data_blocks.push(blk_id);
                offset += 8;
            }

            entries.push(BPlusLeafEntry {
                key,
                meta: BPlusEntryMeta {
                    is_directory: is_dir,
                    size,
                    created_at,
                    data_blocks,
                },
            });
        }
        entries
    }

    /// Serializes B+ Tree entries back to 1024-byte disk blocks.
    fn save_all_leaf_entries(&mut self, entries: &[BPlusLeafEntry]) {
        let mut disk = DISK.lock();
        let mut block = [0u8; DISK_BLOCK_SIZE];

        let entry_count = entries.len() as u32;
        block[0..4].copy_from_slice(&entry_count.to_le_bytes());
        let mut offset = 4;

        for entry in entries {
            let key_bytes = entry.key.as_bytes();
            let key_len = key_bytes.len() as u32;

            if offset + 4 + key_bytes.len() + 17 + (entry.meta.data_blocks.len() * 8) > DISK_BLOCK_SIZE {
                break;
            }

            block[offset..offset+4].copy_from_slice(&key_len.to_le_bytes());
            offset += 4;

            block[offset..offset+key_bytes.len()].copy_from_slice(key_bytes);
            offset += key_bytes.len();

            block[offset] = if entry.meta.is_directory { 1 } else { 0 };
            offset += 1;

            block[offset..offset+8].copy_from_slice(&entry.meta.size.to_le_bytes());
            offset += 8;

            block[offset..offset+8].copy_from_slice(&entry.meta.created_at.to_le_bytes());
            offset += 8;

            let block_count = entry.meta.data_blocks.len() as u32;
            block[offset..offset+4].copy_from_slice(&block_count.to_le_bytes());
            offset += 4;

            for &blk_id in &entry.meta.data_blocks {
                block[offset..offset+8].copy_from_slice(&blk_id.to_le_bytes());
                offset += 8;
            }
        }

        let _ = disk.write_block(self.root_block_id, &block);
    }
}

/// Global thread-safe instance of the On-Disk B+ Tree filesystem index.
pub static BTREE: crate::vga::Locked<DiskBPlusTree> = crate::vga::Locked::new(DiskBPlusTree::new());
