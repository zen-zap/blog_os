// in src/allocator.rs

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use linked_list_allocator::LockedHeap;

pub const HEAP_START: usize = 0x_4444_4444_0000; // some range from virtual memory
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB heap size

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {

    /// allocator function
    ///
    /// Causes deallocator to panic if this doesn't return any memory
	unsafe fn alloc(
		&self,
		_layout: Layout,
	) -> *mut u8 {
		null_mut()
	}

    /// deallocator function
    ///
    /// Panics if allocator never returns any memory
	unsafe fn dealloc(
		&self,
		_ptr: *mut u8,
		_layout: Layout,
	) {
		panic!("dealloc should never be called if the allocator never returns any memory");
	}
}

/// Okay so that was it for the allocator .. now you gotta tell the compiler to use this
//#[global_allocator]
//static ALLOCATOR: Dummy = Dummy;


/// This thing is protected by a Spinlock or Mutex to avoid deadlocks
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Above we did the HEAP_START using some address from virtual memory .. but that would give a
// page_fault unless we map our virtual memory to some physical memory

use x86_64::{
	VirtAddr,
	structures::paging::{
		FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
	},
};

/*
 * VERY CRUCIAL DETAIL:
 *
 * You map the entire physical memory to some range in virtual memory ... 
 * Sometimes, the virtual memory you use might not be mapped to some physical memory.
 * You gotta map that and then use it
 *
 **/



/// function to initialize the heap for the allocator
///
/// This maps the heap pages using the Mapper API from x86_64
pub fn init_heap(
	mapper: &mut impl Mapper<Size4KiB>, 
	frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {

    let page_range = {

        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
    
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {

        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {

            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };

        // initialize the heap only after mapping the heap pages

        unsafe {
            ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
        }
    }

    Ok(())
}
