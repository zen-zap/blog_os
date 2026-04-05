struct ListNode {
	size: usize,
	next: Option<&'static mut ListNode>, // optional pointer to the next node
}

impl ListNode {
	const fn new(size: usize) -> Self {
		ListNode { size, next: None }
	}

	/// returns the start_address
	fn start_addr(&self) -> usize {
		self as *const Self as usize
	}

	/// returns the end_address by adding size to start_address
	fn end_addr(&self) -> usize {
		self.start_addr() + self.size
	}
}

use super::align_up;
use core::mem;

pub struct LinkedListAllocator {
	head: ListNode,
}

impl LinkedListAllocator {
	/// Creates an empty LinkedListAllocator
	pub const fn new() -> Self {
		Self { head: ListNode::new(0) }
	}

	/// Initialize the allocator with the given heap bounds
	///
	/// This function is unsafe because the caller must guarantee that the given
	/// heap bounds are valid and that the heap is unused. This method must be
	/// called only once.
	pub unsafe fn init(
		&mut self,
		heap_start: usize,
		heap_size: usize,
	) {
		unsafe {
			self.add_free_region(heap_start, heap_size);
		}
	}
	/// Adds the given memory region to the front of the list
	unsafe fn add_free_region(
		&mut self,
		addr: usize,
		size: usize,
	) {
		// ensure that the freed region is capable of holding ListNode
		assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
		assert!(size >= mem::size_of::<ListNode>());

		// create a new list node and append it at the start of the list
		let mut node = ListNode::new(size);
		node.next = self.head.next.take();
		// let's create a pointer that could point to a ListNode
		// It's upto us to ensure that the pointer is used correctly
		// This absurd thing is allowed since we're within an unsafe block
		// You have to ensure your own safety
		let node_ptr = addr as *mut ListNode;

		unsafe {
			node_ptr.write(node);
			self.head.next = Some(&mut *node_ptr)
		}
	}

	/// Looks for a free region with the given size and alignment and removes it from the list
	///
	/// Purpose: Finding an entry and removing it from the list
	///
	/// Returns a tuple of the list node and the start address of the allocation
	///
	/// If a region is suitable for an allocation with the given size and alignment, the region
	/// is removed from the list and returned together with the alloc_start address
	fn find_region(
		&mut self,
		size: usize,
		align: usize,
	) -> Option<(&'static mut ListNode, usize)> {
		// reference to the current node, updated after each iteration
		// initially set this to dummy head node
		let mut current = &mut self.head;
		// look for a large enough memory region in the linked list
		while let Some(ref mut region) = current.next {
			if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
				// region suitable for the allocation -> remove node from list
				let next = region.next.take();
				let suitable_region = Some((current.next.take().unwrap(), alloc_start));
				current.next = next;
				return suitable_region;
			} else {
				// region not suitable -> continue to next region
				current = current.next.as_mut().unwrap();
			}
		}

		// No suitable region found
		None
	}

	/// Try to use the given region for an allocation with given size and alignment
	///
	/// Returns the allocation start address on success
	fn alloc_from_region(
		region: &ListNode,
		size: usize,
		align: usize,
	) -> Result<usize, ()> {
		let alloc_start = align_up(region.start_addr(), align);
		let alloc_end = alloc_start.checked_add(size).ok_or(())?;

		if alloc_end > region.end_addr() {
			// region too small
			return Err(());
		}

		let excess_size = region.end_addr() - alloc_end;
		if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
			// rest of the region too small to hold a ListNode
			// required because the allocation splits the region in a used and free part
			return Err(());
		}

		// suitable region for allocation
		Ok(alloc_start)
	}

	/// Adjust the given layout so that the resulting allocated memory region is also
	/// capable of storing a `ListNode`
	///
	/// Returns the adjusted size and alignment as (size, align) tuple
	fn size_align(layout: Layout) -> (usize, usize) {
		let layout = layout
			.align_to(mem::align_of::<ListNode>())
			.expect("Adjusting alignment failed!")
			.pad_to_align(); // ensures size % align == 0
		// align_to -> raises the requested alignment to at least that of a ListNode, so that the
		// block can
		// hold a node safely

		let size = layout.size().max(mem::size_of::<ListNode>());

		(size, layout.align())
	}
}

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
	unsafe fn alloc(
		&self,
		layout: Layout,
	) -> *mut u8 {
		// perform layout adjustments
		let (size, align) = LinkedListAllocator::size_align(layout);
		let mut allocator = self.lock();

		if let Some((region, alloc_start)) = allocator.find_region(size, align) {
			let alloc_end = alloc_start.checked_add(size).expect("Overflow");
			let excess_size = region.end_addr() - alloc_end;

			if excess_size > 0 {
				unsafe {
					allocator.add_free_region(alloc_end, excess_size);
				}
			}

			alloc_start as *mut u8
		} else {
			ptr::null_mut()
		}
	}

	unsafe fn dealloc(
		&self,
		ptr: *mut u8,
		layout: Layout,
	) {
		// perform layout adjustments
		let (size, _) = LinkedListAllocator::size_align(layout);

		unsafe { self.lock().add_free_region(ptr as usize, size) }
	}
}

// Okay so, we did reuse the freed memory here, but the heap memory is still fragmented,
// we do not merge the freed memory for a very large allocation.

// The actual linked list allocator does merge them by keeping the list in sorted order of their
// start addresses ....
