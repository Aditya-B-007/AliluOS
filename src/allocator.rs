//! # AliluOS Heap Memory Allocator (`allocator.rs`)
//!
//! - **WHAT**: Bare-metal dynamic memory allocator implementing Rust's `GlobalAlloc` trait.
//! - **WHY**: Provides support for dynamic allocation data structures (`String`, `Vec`, `Box`, `BTreeMap`) in `#![no_std]` Rust kernel space.
//! - **WHEN**: Called dynamically whenever Rust kernel code allocates or frees heap memory (`String::new()`, `Vec::push()`, `BTreeMap::insert()`).
//! - **HOW**: Combines a fast-path Fixed-Size Block Allocator (for size classes 8B to 2048B) with a Linked-List Fallback Allocator over a 100 KiB static heap pool.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

/// The total size of the kernel heap memory pool in bytes (100 KiB).
///
/// - WHAT: Defines total bytes reserved for dynamic kernel data structures.
/// - WHY: Ensures sufficient memory for file system nodes, shell buffers, and games while keeping kernel lightweight.
pub const HEAP_SIZE: usize = 100 * 1024;

/// Static memory buffer representing raw physical heap space.
///
/// - WHAT: Byte array allocated in the kernel's BSS segment.
/// - WHY: Avoids complex page table mapping requirements during early kernel initialization.
/// - WHEN: Initialized during kernel startup by `init_heap()`.
static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// Thread-safe spinlock wrapper primitive for the global allocator.
///
/// - WHAT: Wraps allocator data structures in a `spin::Mutex`.
/// - WHY: Rust's `GlobalAlloc` trait requires `Sync` thread-safety guarantees.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

// Minimal dependency-free Spinlock Mutex primitive
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

/// Node header structure placed at the beginning of unallocated free memory regions.
///
/// - WHAT: Stores metadata (`size`, `next` pointer) inside the free memory space itself.
/// - WHY: Eliminates external metadata tracking overhead.
struct ListNode {
    size: usize,
    next: *mut ListNode,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode {
            size,
            next: ptr::null_mut(),
        }
    }

    fn start_address(&self) -> usize {
        self as *const Self as usize
    }

    fn end_address(&self) -> usize {
        self.start_address() + self.size
    }
}

/// Fallback Allocator using a linked list of free memory regions.
///
/// - WHAT: Serves large allocations (> 2048B) and replenishes fixed-size block pools.
/// - WHY: Prevents memory fragmentation for arbitrary allocation sizes.
/// - WHEN: Triggered when requested allocation size exceeds 2048 bytes or block lists are empty.
/// - HOW: Searches free list for a block matching requested size and alignment.
struct FallbackAllocator {
    head: ListNode,
}

impl FallbackAllocator {
    const fn new() -> Self {
        FallbackAllocator {
            head: ListNode::new(0),
        }
    }

    unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(align_up(addr, core::mem::align_of::<ListNode>()), addr);
        assert!(size >= core::mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next;
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = node_ptr;
    }

    fn find_region(&mut self, size: usize, align: usize) -> Option<(*mut ListNode, usize)> {
        let mut current = &mut self.head;

        while let Some(next_node) = unsafe { current.next.as_mut() } {
            if let Ok(alloc_start) = self.alloc_from_region(next_node, size, align) {
                let next_next = next_node.next;
                current.next = next_next;
                return Some((next_node, alloc_start));
            }
            current = unsafe { &mut *current.next };
        }
        None
    }

    fn alloc_from_region(&self, region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_address(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_address() {
            return Err(());
        }

        let excess_size = region.end_address() - alloc_end;
        if excess_size > 0 && excess_size < core::mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }
}

/// Fixed-size block allocation classes (8, 16, 32, 64, 128, 256, 512, 1024, 2048 bytes).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Fixed-Size Block Allocator implementation.
///
/// - WHAT: Fast-path memory allocator routing small allocations to size-class free lists.
/// - WHY: Small dynamic allocations (`String` nodes, `Vec` buffers, `BTreeMap` nodes) are frequent in Rust.
///   Size-class pooling achieves $O(1)$ allocation and deallocation performance.
pub struct FixedSizeBlockAllocator {
    list_heads: [*mut ListNode; BLOCK_SIZES.len()],
    fallback: FallbackAllocator,
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        FixedSizeBlockAllocator {
            list_heads: [ptr::null_mut(); BLOCK_SIZES.len()],
            fallback: FallbackAllocator::new(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback.init(heap_start, heap_size);
    }

    unsafe fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        if let Some((node, alloc_start)) = self.fallback.find_region(layout.size(), layout.align()) {
            let node_ptr = node as usize;
            let node_size = (*node).size;
            let alloc_end = alloc_start + layout.size();
            let excess_size = (node_ptr + node_size) - alloc_end;

            if excess_size > 0 {
                self.fallback.add_free_region(alloc_end, excess_size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    /// Allocates dynamic heap memory.
    ///
    /// - WHAT: Returns a raw pointer to an uninitialized memory block matching `layout`.
    /// - WHY: Fulfills Rust runtime memory allocation requests.
    /// - WHEN: Called on `Box::new()`, `Vec::push()`, `String::from()`, etc.
    /// - HOW:
    ///   1. Identifies matching block size class.
    ///   2. If block free list has available node, pops and returns pointer ($O(1)$ fast path).
    ///   3. Otherwise, delegates to fallback allocator.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let head = allocator.list_heads[index];
                if !head.is_null() {
                    allocator.list_heads[index] = (*head).next;
                    head as *mut u8
                } else {
                    let block_size = BLOCK_SIZES[index];
                    let block_align = block_size;
                    let new_layout = Layout::from_size_align(block_size, block_align).unwrap();
                    let new_block = allocator.fallback_alloc(new_layout);
                    if new_block.is_null() {
                        ptr::null_mut()
                    } else {
                        new_block
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    /// Deallocates dynamic heap memory.
    ///
    /// - WHAT: Returns memory pointed to by `ptr` back to free list pool.
    /// - WHY: Prevents memory leaks in long-running kernel environment.
    /// - WHEN: Called automatically when heap-allocated variables go out of scope and drop.
    /// - HOW: Pushes block node onto matching size-class free list or returns to fallback pool.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let new_node = ptr as *mut ListNode;
                let next_node = allocator.list_heads[index];
                new_node.write(ListNode {
                    size: BLOCK_SIZES[index],
                    next: next_node,
                });
                allocator.list_heads[index] = new_node;
            }
            None => {
                allocator.fallback.add_free_region(ptr as usize, layout.size());
            }
        }
    }
}

/// Registered Global Allocator Instance.
#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

/// Initializes kernel heap memory.
///
/// - WHAT: Binds static array `HEAP_SPACE` to `ALLOCATOR`.
/// - WHY: Must be executed during kernel boot before any dynamic strings, vectors, or B-Trees are constructed.
/// - WHEN: Called in `Kernel::initialize()`.
/// - HOW: Passes starting virtual address of `HEAP_SPACE` and size (100 KiB) to `ALLOCATOR.init()`.
pub fn init_heap() {
    unsafe {
        let heap_start = HEAP_SPACE.as_ptr() as usize;
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }
}
