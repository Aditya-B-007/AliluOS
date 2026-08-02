//! # AliluOS B+ Tree Secondary Memory Filesystem Subsystem (`fs.rs`)
//!
//! - **WHAT**: Persistent Filesystem interface powered by an On-Disk B+ Tree index (`btree.rs`) and RAM Virtual Address Table (`vat.rs`).
//! - **WHY**: Fulfills both user requirements:
//!   1. Files are saved in persistent secondary memory (disk blocks) organized via a B+ Tree data structure so data survives system reboot/power-off.
//!   2. Dynamic expansion uses non-contiguous 1024-byte block allocation without CPU-wasting contiguous copy operations, and files are loaded into RAM on-demand via the Virtual Address Table (VAT).
//! - **WHEN**: Called by shell commands (`create`, `folder`, `write`, `read`, `list`, `delete`, `edit`).

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::btree::{BTREE, BPlusEntryMeta};
use crate::vat::VAT;

/// Legacy Node type kept for compatibility.
pub enum Node {
    File,
    Directory,
}

/// The Overall Filesystem Tree Interface.
pub struct FileSystem {
    initialized: bool,
}

impl FileSystem {
    pub const fn new() -> Self {
        FileSystem { initialized: false }
    }

    /// Initializes disk hardware, B+ tree root, and Virtual Address Table.
    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            BTREE.lock().init();

            // Create root directory "/" if not already present
            let mut btree = BTREE.lock();
            if btree.search("/").is_none() {
                let _ = btree.insert("/", BPlusEntryMeta {
                    is_directory: true,
                    size: 0,
                    created_at: 0,
                    data_blocks: Vec::new(),
                });
            }
        }
    }

    /// Converts CWD and relative/absolute path strings into a canonical string path.
    pub fn resolve_path(&self, cwd: &[String], path: &str) -> Vec<String> {
        let mut segments = Vec::new();

        if path.starts_with('/') {
            // Absolute path
        } else {
            // Relative path
            segments.extend(cwd.iter().cloned());
        }

        for item in path.split('/') {
            if item.is_empty() || item == "." {
                continue;
            }
            if item == ".." {
                segments.pop();
            } else {
                segments.push(String::from(item));
            }
        }
        segments
    }

    /// Builds a canonical string path from path segments (e.g. `["docs", "test.txt"]` -> `"/docs/test.txt"`).
    pub fn canonical_path(&self, segments: &[String]) -> String {
        if segments.is_empty() {
            String::from("/")
        } else {
            let mut path = String::new();
            for s in segments {
                path.push('/');
                path.push_str(s);
            }
            path
        }
    }

    /// Splits resolved path segments into (parent_segments, target_name).
    pub fn split_parent_and_name<'a>(&self, resolved: &'a [String]) -> (&'a [String], &'a str) {
        if resolved.is_empty() {
            (&[], "")
        } else {
            let (parent, name) = resolved.split_at(resolved.len() - 1);
            (parent, &name[0])
        }
    }

    /// Finds whether target directory path exists in B+ tree disk index.
    pub fn find_directory(&self, segments: &[String]) -> Option<()> {
        let path_str = self.canonical_path(segments);
        if path_str == "/" {
            return Some(());
        }
        let btree = BTREE.lock();
        if let Some(meta) = btree.search(&path_str) {
            if meta.is_directory {
                Some(())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Lists directory entries in alphabetical B+ Tree sorted order.
    pub fn list_directory(&self, segments: &[String]) -> Result<Vec<(String, bool)>, &'static str> {
        let path_str = self.canonical_path(segments);
        let btree = BTREE.lock();
        if path_str != "/" && btree.search(&path_str).is_none() {
            return Err("Directory not found in B+ Tree");
        }
        Ok(btree.list_directory_entries(&path_str))
    }

    /// Creates a directory folder using B+ Tree on-disk insertion.
    pub fn create_directory(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut btree = BTREE.lock();
        if btree.search(&path_str).is_some() {
            return Err("Name already exists in this folder");
        }

        btree.insert(&path_str, BPlusEntryMeta {
            is_directory: true,
            size: 0,
            created_at: ticks,
            data_blocks: Vec::new(),
        })
    }

    /// Creates an empty file node in the on-disk B+ Tree index.
    pub fn create_file(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut btree = BTREE.lock();
        if btree.search(&path_str).is_some() {
            return Err("Name already exists in this folder");
        }

        btree.insert(&path_str, BPlusEntryMeta {
            is_directory: false,
            size: 0,
            created_at: ticks,
            data_blocks: Vec::new(),
        })
    }

    /// Writes text content to file via dynamic block allocation in VAT and B+ Tree.
    pub fn write_file(&mut self, parent_segments: &[String], name: &str, content: &str) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let ticks = unsafe { crate::interrupts::timer_ticks() };
        let mut vat = VAT.lock();
        vat.write_file_dynamic(&path_str, content, ticks)
    }

    /// Reads file text content on-demand from disk into RAM via Virtual Address Table.
    pub fn read_file(&self, parent_segments: &[String], name: &str) -> Result<String, &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut vat = VAT.lock();
        vat.read_file_on_demand(&path_str)
    }

    /// Removes a file or directory node from B+ tree index and frees allocated blocks.
    pub fn delete_node(&mut self, parent_segments: &[String], name: &str) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut vat = VAT.lock();
        vat.delete_entry(&path_str)
    }
}

/// Global synchronized static instance of the B+ Tree Filesystem.
pub static FS: crate::vga::Locked<FileSystem> = crate::vga::Locked::new(FileSystem::new());
