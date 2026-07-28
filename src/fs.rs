//! # AliluOS B-Tree Filesystem Subsystem (`fs.rs`)
//!
//! - **WHAT**: Hierarchical memory filesystem using a B-Tree index structure (`BTreeMap<String, Node>`).
//! - **WHY**: Provides fast, logarithmic ($O(\log N)$) sorted directory searches, insertions, and deletions,
//!   establishing the core B-Tree directory indexing layout required for upcoming secondary memory disk block persistence.
//! - **WHEN**: Executed when shell commands (`create`, `folder`, `write`, `read`, `list`, `delete`, `edit`) query or modify files.
//! - **HOW**: Thread-safely wrapped in `pub static FS: Locked<FileSystem>`. Operations traverse path segments recursively over nested `DirectoryNode` B-Tree maps.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Enumeration representing a node in the filesystem directory tree.
///
/// - **WHAT**: Sum type representing either a `FileNode` or a `DirectoryNode`.
/// - **WHY**: Allows subdirectories and files to be stored uniformly within `BTreeMap<String, Node>`.
/// - **WHEN**: Evaluated when listing, inspecting, or traversing directory trees.
#[derive(Clone)]
pub enum Node {
    File(FileNode),
    Directory(DirectoryNode),
}

impl Node {
    /// Returns the human-readable string name of the node.
    pub fn name(&self) -> &str {
        match self {
            Node::File(f) => &f.name,
            Node::Directory(d) => &d.name,
        }
    }

    /// Returns `true` if this node is a `DirectoryNode`.
    pub fn is_directory(&self) -> bool {
        matches!(self, Node::Directory(_))
    }
}

/// Represents a standard text/binary file node.
///
/// - **WHAT**: Stores filename, UTF-8 text content, and creation timer ticks.
/// - **WHY**: Holds actual file payload created by `create`, `write`, or `edit`.
#[derive(Clone)]
pub struct FileNode {
    pub name: String,
    pub content: String,
    pub created_at: u64,
}

/// Directory Node with B-Tree Entry Indexing.
///
/// - **WHAT**: Represents a folder containing a `BTreeMap<String, Node>` index of child files and subdirectories.
/// - **WHY**: Replaces linear flat arrays (`Vec<Node>`) with a B-Tree structure, providing sorted keys and logarithmic search efficiency ($O(\log N)$).
/// - **WHEN**: Traversed during path resolution and modified during folder/file creation and deletion.
/// - **HOW**: Keyed by string entry names (`String`), storing `Node` instances as values.
#[derive(Clone)]
pub struct DirectoryNode {
    pub name: String,
    pub children: BTreeMap<String, Node>,
    pub created_at: u64,
}

impl DirectoryNode {
    /// Constructs a new empty `DirectoryNode`.
    pub fn new(name: &str, ticks: u64) -> Self {
        Self {
            name: String::from(name),
            children: BTreeMap::new(),
            created_at: ticks,
        }
    }

    /// Recursively looks up an immutable node reference along path segments using B-Tree lookups.
    ///
    /// - **WHAT**: Traverses directory hierarchy following path segments (e.g. `["docs", "sub", "file.txt"]`).
    /// - **WHY**: Enables immutable operations like `read_file` or `list_directory`.
    /// - **WHEN**: Invoked during file reads or directory listings.
    /// - **HOW**: Uses `BTreeMap::get()` at each level, descending recursively until the target is reached.
    pub fn find_node(&self, path: &[&str]) -> Option<&Node> {
        if path.is_empty() {
            return None;
        }
        let next_segment = path[0];
        let child = self.children.get(next_segment)?;
        if path.len() == 1 {
            Some(child)
        } else {
            match child {
                Node::Directory(dir) => dir.find_node(&path[1..]),
                Node::File(_) => None,
            }
        }
    }

    /// Recursively looks up a mutable node reference along path segments using B-Tree lookups.
    ///
    /// - **WHAT**: Traverses directory hierarchy returning `&mut Node`.
    /// - **WHY**: Enables mutable operations like `write_file`, `create_file`, or `delete_node`.
    /// - **WHEN**: Invoked during file writing, node creation, or node deletion.
    /// - **HOW**: Uses `BTreeMap::get_mut()` at each level to traverse down the tree.
    pub fn find_node_mut(&mut self, path: &[&str]) -> Option<&mut Node> {
        if path.is_empty() {
            return None;
        }
        let next_segment = path[0];
        let child = self.children.get_mut(next_segment)?;
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

/// The Overall Filesystem Tree State.
///
/// - **WHAT**: Holds the root directory node (`/`) of AliluOS.
/// - **WHY**: Provides high-level path resolution, CRUD APIs, and directory traversal routines.
pub struct FileSystem {
    pub root: DirectoryNode,
}

impl FileSystem {
    pub const fn new() -> Self {
        FileSystem {
            root: DirectoryNode {
                name: String::new(),
                children: BTreeMap::new(),
                created_at: 0,
            },
        }
    }

    /// Resolves absolute (`/path`) or relative (`path`) string inputs against Current Working Directory (CWD).
    ///
    /// - **WHAT**: Converts path strings containing `/`, `.`, and `..` into a canonical vector of path segments.
    /// - **WHY**: Normalizes user input paths so commands like `enter ..` or `create ./file` resolve correctly.
    /// - **WHEN**: Called prior to any file/folder CRUD operation.
    /// - **HOW**: Splits path string by `/`, handles `..` via `pop()`, and appends path elements to vector.
    pub fn resolve_path(&self, cwd: &[String], path: &str) -> Vec<String> {
        let mut segments = Vec::new();

        if path.starts_with('/') {
            // Absolute path: start from root
        } else {
            // Relative path: start with CWD
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

    /// Splits resolved path segments into (parent_segments, target_name).
    pub fn split_parent_and_name<'a>(&self, resolved: &'a [String]) -> (&'a [String], &'a str) {
        if resolved.is_empty() {
            (&[], "")
        } else {
            let (parent, name) = resolved.split_at(resolved.len() - 1);
            (parent, &name[0])
        }
    }

    /// Finds immutable reference to target `DirectoryNode` by path segments.
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

    /// Finds mutable reference to target `DirectoryNode` by path segments.
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

    /// Lists directory entries in alphabetical B-Tree sorted order.
    ///
    /// - **WHAT**: Returns vector of `(String, bool)` tuples representing child names and directory flags.
    /// - **WHY**: Powered by `BTreeMap::iter()`, output is automatically sorted without requiring explicit sorting loops.
    /// - **WHEN**: Triggered by shell `list` command.
    pub fn list_directory(&self, segments: &[String]) -> Result<Vec<(String, bool)>, &'static str> {
        if let Some(dir) = self.find_directory(segments) {
            let list = dir.children.iter().map(|(name, node)| {
                let is_dir = matches!(node, Node::Directory(_));
                (name.clone(), is_dir)
            }).collect();
            Ok(list)
        } else {
            Err("Directory not found")
        }
    }

    /// Creates a directory folder using B-Tree insertion.
    ///
    /// - **WHAT**: Inserts a new `DirectoryNode` into parent's `children` BTreeMap.
    /// - **WHY**: Expands directory tree.
    /// - **WHEN**: Triggered by shell `folder` command.
    /// - **HOW**: Checks `parent.children.contains_key(name)`, returning error if name exists, otherwise calls `insert()`.
    pub fn create_directory(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if parent.children.contains_key(name) {
                return Err("Name already exists in this folder");
            }
            parent.children.insert(String::from(name), Node::Directory(DirectoryNode::new(name, ticks)));
            Ok(())
        } else {
            Err("Parent directory not found")
        }
    }

    /// Creates a file node using B-Tree insertion.
    ///
    /// - **WHAT**: Inserts a new `FileNode` into parent's `children` BTreeMap.
    /// - **WHY**: Adds file entry to directory index.
    /// - **WHEN**: Triggered by shell `create` command.
    pub fn create_file(&mut self, parent_segments: &[String], name: &str, ticks: u64) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if parent.children.contains_key(name) {
                return Err("Name already exists in this folder");
            }
            parent.children.insert(String::from(name), Node::File(FileNode {
                name: String::from(name),
                content: String::new(),
                created_at: ticks,
            }));
            Ok(())
        } else {
            Err("Parent directory not found")
        }
    }

    /// Overwrites string content of a file in the B-Tree index.
    ///
    /// - **WHAT**: Updates `content` field of matching `FileNode`.
    /// - **WHEN**: Triggered by shell `write` command or `:wq` editor save.
    pub fn write_file(&mut self, parent_segments: &[String], name: &str, content: &str) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if let Some(Node::File(file)) = parent.children.get_mut(name) {
                file.content = String::from(content);
                Ok(())
            } else {
                Err("File not found")
            }
        } else {
            Err("Parent directory not found")
        }
    }

    /// Reads string content of a file from the B-Tree index.
    ///
    /// - **WHAT**: Returns copy of `content` field from matching `FileNode`.
    /// - **WHEN**: Triggered by shell `read` command or entering `edit` mode.
    pub fn read_file(&self, parent_segments: &[String], name: &str) -> Result<String, &'static str> {
        if let Some(parent) = self.find_directory(parent_segments) {
            if let Some(Node::File(file)) = parent.children.get(name) {
                Ok(file.content.clone())
            } else {
                Err("File not found or is a directory")
            }
        } else {
            Err("Directory not found")
        }
    }

    /// Removes a file or directory node from the B-Tree index.
    ///
    /// - **WHAT**: Deletes entry from parent's `children` BTreeMap via `remove()`.
    /// - **WHEN**: Triggered by shell `delete` command.
    pub fn delete_node(&mut self, parent_segments: &[String], name: &str) -> Result<(), &'static str> {
        if let Some(parent) = self.find_directory_mut(parent_segments) {
            if parent.children.remove(name).is_some() {
                Ok(())
            } else {
                Err("Target not found")
            }
        } else {
            Err("Directory not found")
        }
    }
}

/// Global thread-safe spinlock wrapper.
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

/// Global synchronized static instance of the AliluOS B-Tree Filesystem.
pub static FS: Locked<FileSystem> = Locked::new(FileSystem::new());
