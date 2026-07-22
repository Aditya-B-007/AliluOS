use alloc::string::String;
use alloc::vec::Vec;

/// A node in our hierarchical filesystem directory tree.
#[derive(Clone)]
pub enum Node {
    File(FileNode),
    Directory(DirectoryNode),
}

impl Node {
    /// Returns the name of the node.
    pub fn name(&self) -> &str {
        match self {
            Node::File(f) => &f.name,
            Node::Directory(d) => &d.name,
        }
    }

    /// Helper to check if a node is a directory.
    pub fn is_directory(&self) -> bool {
        matches!(self, Node::Directory(_))
    }
}

/// Node representing a standard file.
#[derive(Clone)]
pub struct FileNode {
    pub name: String,
    pub content: String,
    pub created_at: u64, // Stored system timer ticks at creation
}

/// Node representing a directory containing files or nested subdirectories.
#[derive(Clone)]
pub struct DirectoryNode {
    pub name: String,
    pub children: Vec<Node>,
    pub created_at: u64,
}

impl DirectoryNode {
    pub fn new(name: &str, ticks: u64) -> Self {
        Self {
            name: String::from(name),
            children: Vec::new(),
            created_at: ticks,
        }
    }

    /// Recursively finds a reference to a node given a slice of path segments.
    pub fn find_node(&self, path: &[&str]) -> Option<&Node> {
        if path.is_empty() {
            return None;
        }
        let next_segment = path[0];
        let child = self.children.iter().find(|c| c.name() == next_segment)?;
        if path.len() == 1 {
            Some(child)
        } else {
            match child {
                Node::Directory(dir) => dir.find_node(&path[1..]),
                Node::File(_) => None,
            }
        }
    }

    /// Recursively finds a mutable reference to a node given a slice of path segments.
    pub fn find_node_mut(&mut self, path: &[&str]) -> Option<&mut Node> {
        if path.is_empty() {
            return None;
        }
        let next_segment = path[0];
        let child = self.children.iter_mut().find(|c| c.name() == next_segment)?;
        if path.len() == 1 {
            Some(child)
        } else {
            match child {
                Node::Directory(dir) => dir.find_node_mut(&path[1..]),
                Node::File(_) => None,
            }
        }
    }
}

/// The overall Hierarchical Filesystem.
pub struct FileSystem {
    pub root: DirectoryNode,
}

impl FileSystem {
    pub const fn new() -> Self {
        // We start with an empty root directory "/"
        FileSystem {
            root: DirectoryNode {
                name: String::new(), // root directory has empty name in path segments
                children: Vec::new(),
                created_at: 0,
            },
        }
    }

    /// Resolves path segments (absolute or relative to CWD) into reference path strings.
    pub fn resolve_path(&self, cwd: &[String], path: &str) -> Vec<String> {
        let mut segments = Vec::new();

        // Check if path is absolute
        if path.starts_with('/') {
            // Absolute path: start from root
        } else {
            // Relative path: start with CWD
            segments.extend(cwd.iter().cloned());
        }

        // Process path segments split by '/'
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

    /// Helper to find a DirectoryNode reference by segments.
    pub fn find_directory(&self, segments: &[String]) -> Option<&DirectoryNode> {
        if segments.is_empty() {
            return Some(&self.root);
        }
        let refs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
        match self.root.find_node(&refs) {
            Some(Node::Directory(dir)) => Some(dir),
            _ => None,
        }
    }

    /// Helper to find a mutable DirectoryNode reference by segments.
    pub fn find_directory_mut(&mut self, segments: &[String]) -> Option<&mut DirectoryNode> {
        if segments.is_empty() {
            return Some(&mut self.root);
        }
        let refs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
        match self.root.find_node_mut(&refs) {
            Some(Node::Directory(dir)) => Some(dir),
            _ => None,
        }
    }

    /// Lists child names of the directory specified by segments.
    pub fn list_directory(&self, segments: &[String]) -> Result<Vec<(String, bool)>, &'static str> {
        if let Some(dir) = self.find_directory(segments) {
            let list = dir.children.iter().map(|c| {
                let is_dir = matches!(c, Node::Directory(_));
                (String::from(c.name()), is_dir)
            }).collect();
            Ok(list)
        } else {
            Err("Directory not found")
        }
    }

    /// Creates a directory folder.
    pub fn create_directory(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if parent.children.iter().any(|c| c.name() == name) {
                return Err("Name already exists in this folder");
            }
            parent.children.push(Node::Directory(DirectoryNode::new(name, ticks)));
            Ok(())
        } else {
            Err("Parent directory not found")
        }
    }

    /// Creates a file.
    pub fn create_file(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if parent.children.iter().any(|c| c.name() == name) {
                return Err("Name already exists in this folder");
            }
            parent.children.push(Node::File(FileNode {
                name: String::from(name),
                content: String::new(),
                created_at: ticks,
            }));
            Ok(())
        } else {
            Err("Parent directory not found")
        }
    }

    /// Writes text to a file.
    pub fn write_file(&mut self, parent_segments: &[String], name: &str, content: &str) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if let Some(Node::File(file)) = parent.children.iter_mut().find(|c| c.name() == name) {
                file.content = String::from(content);
                Ok(())
            } else {
                Err("File not found")
            }
        } else {
            Err("Parent directory not found")
        }
    }

    /// Reads contents of a file.
    pub fn read_file(&self, parent_segments: &[String], name: &str) -> Result<String, &'static str> {
        if let Some(parent) = self.find_directory(parent_segments) {
            if let Some(Node::File(file)) = parent.children.iter().find(|c| c.name() == name) {
                Ok(file.content.clone())
            } else {
                Err("File not found or is a directory")
            }
        } else {
            Err("Directory not found")
        }
    }

    /// Deletes a file or directory node.
    pub fn delete_node(&mut self, parent_segments: &[String], name: &str) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if let Some(index) = parent.children.iter().position(|c| c.name() == name) {
                parent.children.remove(index);
                Ok(())
            } else {
                Err("Target not found")
            }
        } else {
            Err("Directory not found")
        }
    }
}

// Thread-safe wrapper using spinlock Mutex.
pub struct Locked<T> {
    inner: spin::Mutex<T>,
}

impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.inner.lock()
    }
}

mod spin {
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::cell::UnsafeCell;
    use core::ops::{Deref, DerefMut};

    pub struct Mutex<T> {
        locked: AtomicBool,
        value: UnsafeCell<T>,
    }

    unsafe impl<T: Send> Sync for Mutex<T> {}

    impl<T> Mutex<T> {
        pub const fn new(value: T) -> Self {
            Self {
                locked: AtomicBool::new(false),
                value: UnsafeCell::new(value),
            }
        }

        pub fn lock(&self) -> MutexGuard<T> {
            while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
                core::hint::spin_loop();
            }
            MutexGuard { mutex: self }
        }
    }

    pub struct MutexGuard<'a, T> {
        mutex: &'a Mutex<T>,
    }

    impl<'a, T> Deref for MutexGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.mutex.value.get() }
        }
    }

    impl<'a, T> DerefMut for MutexGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *self.mutex.value.get() }
        }
    }

    impl<'a, T> Drop for MutexGuard<'a, T> {
        fn drop(&mut self) {
            self.mutex.locked.store(false, Ordering::Release);
        }
    }
}

// Global reference instance of our In-Memory Hierarchical File System.
pub static FS: Locked<FileSystem> = Locked::new(FileSystem::new());
