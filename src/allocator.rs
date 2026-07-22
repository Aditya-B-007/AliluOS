use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

// The size of our kernel heap memory pool (100 KiB)
pub const HEAP_SIZE: usize = 100 * 1024;

// The static buffer representing the raw physical/virtual heap memory pool.
// This allows us to have a heap without needing complex page-table mappings yet.
static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// A simple thread-safe wrapper using a spinlock.
/// Needed because the `GlobalAlloc` trait requires the allocator to be thread-safe (Sync).
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

// Minimal implementation of a Spinlock Mutex to keep us completely dependency-free.
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
            // Spin until the lock is acquired
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

/// A node in the free list.
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

/// Fallback Allocator using a linked list of free blocks.
/// Used for allocations larger than 2048 bytes or when size-class blocks are exhausted.
struct FallbackAllocator {
    head: ListNode,
}

impl FallbackAllocator {
    const fn new() -> Self {
        FallbackAllocator {
            head: ListNode::new(0),
        }
    }

    /// Initializes the fallback allocator with the given memory range.
    unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds a free memory region back into the allocator's free list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // Ensure the address is properly aligned
        assert_eq!(align_up(addr, core::mem::align_of::<ListNode>()), addr);
        assert!(size >= core::mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next;
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = node_ptr;
    }

    /// Looks for a free region that fits the requested size and alignment.
    /// Returns the start address of the region if found.
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

    /// Tries to allocate from a specific free region, returning the aligned start address.
    fn alloc_from_region(&self, region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_address(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_address() {
            return Err(());
        }

        let excess_size = region.end_address() - alloc_end;
        if excess_size > 0 && excess_size < core::mem::size_of::<ListNode>() {
            // Cannot use the remaining space for another list node due to alignment/size limitations
            return Err(());
        }

        Ok(alloc_start)
    }
}

/// The block size classes we support.
/// Any allocations matching or below these sizes will use the fast path.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Fixed-Size Block Allocator.
/// Directs small allocations to block-size lists and falls back to Linked List.
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

    /// Initializes the allocator with the start address and size of the heap.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback.init(heap_start, heap_size);
    }

    /// Helper function to allocate using the fallback allocator.
    unsafe fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        if let Some((node, alloc_start)) = self.fallback.find_region(layout.size(), layout.align()) {
            let node_ptr = node as usize;
            let node_size = (*node).size;
            let alloc_end = alloc_start + layout.size();
            let excess_size = (node_ptr + node_size) - alloc_end;

            if excess_size > 0 {
                // Return unused excess memory back to the fallback pool
                self.fallback.add_free_region(alloc_end, excess_size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }
}

/// Rounds up a virtual address to the nearest aligned boundary.
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

/// Helper function to match an allocation layout to the nearest block size list index.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let head = allocator.list_heads[index];
                if !head.is_null() {
                    // Fast path: Pop a block from the free list
                    allocator.list_heads[index] = (*head).next;
                    head as *mut u8
                } else {
                    // Free list is empty, allocate a new block from the fallback allocator
                    let block_size = BLOCK_SIZES[index];
                    let block_align = block_size; // Block size is a power of 2, so alignment is simple
                    let new_layout = Layout::from_size_align(block_size, block_align).unwrap();
                    let new_block = allocator.fallback_alloc(new_layout);
                    if new_block.is_null() {
                        ptr::null_mut()
                    } else {
                        new_block
                    }
                }
            }
            None => {
                // Allocation size is too large (> 2048 bytes), route directly to fallback allocator
                allocator.fallback_alloc(layout)
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                // Return block to the corresponding size-class free list
                let new_node = ptr as *mut ListNode;
                let next_node = allocator.list_heads[index];
                new_node.write(ListNode {
                    size: BLOCK_SIZES[index],
                    next: next_node,
                });
                allocator.list_heads[index] = new_node;
            }
            None => {
                // Free the block back to the fallback allocator
                allocator.fallback.add_free_region(ptr as usize, layout.size());
            }
        }
    }
}

// Register the allocator globally in the Rust runtime
#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

/// Initializes the heap with our predefined static array memory range.
pub fn init_heap() {
    unsafe {
        let heap_start = HEAP_SPACE.as_ptr() as usize;
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }
}
