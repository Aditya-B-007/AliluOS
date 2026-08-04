//! # AliluOS B+ Tree Secondary Memory Filesystem Subsystem (`ring1/fs.rs`)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::btree::{BTREE, BPlusEntryMeta};
use crate::vat::VAT;

pub enum Node {
    File,
    Directory,
}

pub struct FileSystem {
    initialized: bool,
}

impl FileSystem {
    pub const fn new() -> Self {
        FileSystem { initialized: false }
    }

    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            BTREE.lock().init();

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

    pub fn resolve_path(&self, cwd: &[String], path: &str) -> Vec<String> {
        let mut segments = Vec::new();

        if path.starts_with('/') {
        } else {
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

    pub fn split_parent_and_name<'a>(&self, resolved: &'a [String]) -> (&'a [String], &'a str) {
        if resolved.is_empty() {
            (&[], "")
        } else {
            let (parent, name) = resolved.split_at(resolved.len() - 1);
            (parent, &name[0])
        }
    }

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

    pub fn list_directory(&self, segments: &[String]) -> Result<Vec<(String, bool)>, &'static str> {
        let path_str = self.canonical_path(segments);
        let btree = BTREE.lock();
        if path_str != "/" && btree.search(&path_str).is_none() {
            return Err("Directory not found in B+ Tree");
        }
        Ok(btree.list_directory_entries(&path_str))
    }

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

    pub fn write_file(&mut self, parent_segments: &[String], name: &str, content: &str) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let ticks = unsafe { crate::interrupts::timer_ticks() };
        let mut vat = VAT.lock();
        vat.write_file_dynamic(&path_str, content, ticks)
    }

    pub fn read_file(&self, parent_segments: &[String], name: &str) -> Result<String, &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut vat = VAT.lock();
        vat.read_file_on_demand(&path_str)
    }

    pub fn delete_node(&mut self, parent_segments: &[String], name: &str) -> Result<(), &'static str> {
        let mut full_segments = parent_segments.to_vec();
        full_segments.push(String::from(name));
        let path_str = self.canonical_path(&full_segments);

        let mut vat = VAT.lock();
        vat.delete_entry(&path_str)
    }
}

pub static FS: crate::vga::Locked<FileSystem> = crate::vga::Locked::new(FileSystem::new());
