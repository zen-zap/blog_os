//! in src/virtio/mod.rs

pub mod pci;

use crate::memory::BootInfoFrameAllocator;
use core::ops::Sub;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal};
use x86_64::{
	PhysAddr, VirtAddr,
	structures::paging::{
		FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
		mapper::MapToError,
	},
};

// Global reference to the mapper and frame allocator
// gotta set them in kernels init function
lazy_static! {
	pub static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
	pub static ref PAGE_MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
}

pub struct OsHal;

pub static mut PHYSICAL_MEMORY_OFFSET: u64 = 0;

unsafe impl Hal for OsHal {
	fn dma_alloc(
		pages: usize,
		direction: BufferDirection,
	) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
		let mut frame_allocator = FRAME_ALLOCATOR.lock();
		let mut mapper = PAGE_MAPPER.lock();

		let allocator = frame_allocator.as_mut().expect("Frame allocator not initialized");
		let mapper = mapper.as_mut().expect("Page Mapper not initialized");

		if pages > 1 {
			panic!("dma_alloc: multipage contiguous allocation not supported yet");
		}

		// allocating a physical frame
		let frame = allocator.allocate_frame().expect("Failed to allocate frame for DMA");
		let paddr = frame.start_address().as_u64();

		// virtual address for this physical frame
		let vaddr = VirtAddr::new(paddr + unsafe { PHYSICAL_MEMORY_OFFSET });
		let page = Page::containing_address(vaddr);
		// the DMA buffer needs to be writable
		let flags = PageTableFlags::WRITABLE | PageTableFlags::PRESENT;

		// mapping the page
		unsafe {
			mapper
				.map_to(page, frame, flags, allocator)
				.expect("Failed to map DMA page")
				.flush();
		}

		let virtio_paddr = paddr as usize;
		(virtio_paddr, NonNull::new(vaddr.as_mut_ptr()).unwrap())
	}

	unsafe fn dma_dealloc(
		paddr: virtio_drivers::PhysAddr,
		vaddr: NonNull<u8>,
		pages: usize,
	) -> i32 {
		panic!("dma_dealloc: paddr={:#x}, pages={} (leaking memory)", paddr, pages);
		// maybe use trace! crate but it might be unusable in no-std envs
		0
	}

	unsafe fn mmio_phys_to_virt(
		paddr: virtio_drivers::PhysAddr,
		size: usize,
	) -> NonNull<u8> {
		// This function is for mapping the device's control registers.
		let paddr = PhysAddr::new(paddr as u64);
		let vaddr = VirtAddr::new(paddr.as_u64()); // We can use identity mapping for MMIO

		let start_page: Page = Page::containing_address(vaddr);
		let end_page: Page = Page::containing_address(vaddr + (size - 1));

		let mut mapper = PAGE_MAPPER.lock();
		let mut frame_allocator = FRAME_ALLOCATOR.lock();
		let mapper = mapper.as_mut().expect("Page Mapper not initialized");
		let frame_allocator = frame_allocator.as_mut().expect("Frame allocator not initialized");

		for page in Page::range_inclusive(start_page, end_page) {
			let frame = PhysFrame::containing_address(paddr + (page - start_page) * 4096);

			// These flags are crucial for device memory:
			// - PRESENT: The page is mapped.
			// - WRITABLE: We need to write to device registers.
			// - NO_EXECUTE: We should never execute code from device memory.
			// - WRITE_THROUGH: Ensures writes go directly to the device and are not cached.
			let flags = PageTableFlags::PRESENT
				| PageTableFlags::WRITABLE
				| PageTableFlags::NO_EXECUTE
				| PageTableFlags::WRITE_THROUGH;

			// Create the mapping in the page table.
			mapper
				.map_to(page, frame, flags, frame_allocator)
				.expect("Failed to map MMIO page")
				.flush();
		}

		NonNull::new(vaddr.as_mut_ptr()).unwrap()
	}

	unsafe fn share(
		buffer: NonNull<[u8]>,
		direction: BufferDirection,
	) -> virtio_drivers::PhysAddr {
		// This is where your `translate_addr` function comes in!
		let vaddr = VirtAddr::new(buffer.as_ptr() as *mut u8 as u64);

		// We use the offset you've already calculated to translate.
		let offset = VirtAddr::new(PHYSICAL_MEMORY_OFFSET);

		// This is the function you wrote in memory.rs!

		let phyaddr = crate::memory::translate_addr(vaddr, offset)
			.expect("Failed to translate virtual address for sharing");

		phyaddr.as_u64() as usize
	}

	unsafe fn unshare(
		paddr: virtio_drivers::PhysAddr,
		buffer: NonNull<[u8]>,
		direction: BufferDirection,
	) {
		// Do nothing
	}
}
