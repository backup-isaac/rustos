use core::alloc::Layout;
use core::fmt;

use crate::allocator::linked_list::LinkedList;
use crate::allocator::util::*;
use crate::allocator::LocalAlloc;

/// A simple allocator that allocates based on size classes.
///   bin 0 (2^3 bytes)    : handles allocations in (0, 2^3]
///   bin 1 (2^4 bytes)    : handles allocations in (2^3, 2^4]
///   ...
///   bin 29 (2^22 bytes): handles allocations in (2^31, 2^32]
///
///   map_to_bin(size) -> k
///

pub struct Allocator {
    bins: [LinkedList; 30],
}

fn absorb_memory(allocator: &mut Allocator, start: usize, end: usize) {
    let mut bin = 29;
    let mut block_size = 1 << 32;
    let mut curr = start;
    while block_size >= 8 {
        while curr + block_size <= end {
            unsafe { allocator.bins[bin].push(curr as *mut usize); }
            curr += block_size;
        }
        if bin > 0 {
            bin -= 1;
        } else {
            break;
        }
        block_size /= 2;
    }
}

impl Allocator {
    /// Creates a new bin allocator that will allocate memory from the region
    /// starting at address `start` and ending at address `end`.
    pub fn new(start: usize, end: usize) -> Allocator {
        let mut alloc = Allocator {
            bins: [LinkedList::new(); 30],
        };
        absorb_memory(&mut alloc, start, end);
        return alloc;
    }
}

impl LocalAlloc for Allocator {
    /// Allocates memory. Returns a pointer meeting the size and alignment
    /// properties of `layout.size()` and `layout.align()`.
    ///
    /// If this method returns an `Ok(addr)`, `addr` will be non-null address
    /// pointing to a block of storage suitable for holding an instance of
    /// `layout`. In particular, the block will be at least `layout.size()`
    /// bytes large and will be aligned to `layout.align()`. The returned block
    /// of storage may or may not have its contents initialized or zeroed.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure that `layout.size() > 0` and that
    /// `layout.align()` is a power of two. Parameters not meeting these
    /// conditions may result in undefined behavior.
    ///
    /// # Errors
    ///
    /// Returning null pointer (`core::ptr::null_mut`)
    /// indicates that either memory is exhausted
    /// or `layout` does not meet this allocator's
    /// size or alignment constraints.
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let mut bin = 0;
        let mut block_size = 8;
        let target_size = if layout.size().next_power_of_two() > layout.align() {
            if layout.size().next_power_of_two() > 8 {
                layout.size().next_power_of_two()
            } else {
                8
            }
        } else if layout.align() > 8 {
            layout.align()
        } else {
            8
        };
        let target_align = if layout.align() > 8 {
            layout.align()
        } else {
            8
        };
        while bin < 30 {
            let mut good_node = None;
            for node in self.bins[bin].iter_mut() {
                let start = node.value() as usize;
                let start_of_alloc = align_up(start, target_align);
                if start_of_alloc + target_size <= start + block_size {
                    if start == start_of_alloc {
                        node.pop();
                        absorb_memory(self, start_of_alloc + target_size, start + block_size);
                        return start_of_alloc as *mut u8;
                    }
                    good_node = Some(node);
                }
            }
            if let Some(n) = good_node {
                let start = n.value() as usize;
                let start_of_alloc = align_up(start, target_align);
                n.pop();
                absorb_memory(self, start, start_of_alloc);
                absorb_memory(self, start_of_alloc + target_size, start + block_size);
                return start_of_alloc as *mut u8;
            }
            bin += 1;
            block_size *= 2;
        }
        core::ptr::null_mut()
    }

    /// Deallocates the memory referenced by `ptr`.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure the following:
    ///
    ///   * `ptr` must denote a block of memory currently allocated via this
    ///     allocator
    ///   * `layout` must properly represent the original layout used in the
    ///     allocation call that returned `ptr`
    ///
    /// Parameters not meeting these conditions may result in undefined
    /// behavior.
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let target_size = if layout.size().next_power_of_two() > layout.align() {
            if layout.size().next_power_of_two() > 8 {
                layout.size().next_power_of_two()
            } else {
                8
            }
        } else if layout.align() > 8 {
            layout.align()
        } else {
            8
        };
        absorb_memory(self, ptr as usize, ptr as usize + target_size);
    }
}

// FIXME: Implement `Debug` for `Allocator`.
impl fmt::Debug for Allocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BinAllocator {{ bins: {:?} }}", self.bins)
    }
}