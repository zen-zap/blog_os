//! in src/virtio/mod.rs

pub mod pci;

use crate::memory::BootInfoFrameAllocator;
use crate::println;
use alloc::vec::Vec;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal};
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame};
use x86_64::{
	PhysAddr, VirtAddr,
	structures::paging::{FrameAllocator, OffsetPageTable},
};

// Global reference to the frame allocator
// gotta set it in kernel init function
lazy_static! {
	pub static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
	pub static ref PAGE_MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
	static ref DMA_FREE_LIST: Mutex<Vec<PhysFrame>> = Mutex::new(Vec::new());
}

pub struct OsHal;

pub static mut PHYSICAL_MEMORY_OFFSET: u64 = 0;

unsafe impl Hal for OsHal {
	fn dma_alloc(
		pages: usize,
		_direction: BufferDirection,
	) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
		println!("[DMA] Single Page DMA allocation");

		if pages > 1 {
			println!("Single Page buffers only supported");
			panic!("dma_alloc: multipage contiguous allocation not supported yet");
		}

		// before allocating .. try using any returned physical frames
		if let Some(frame) = DMA_FREE_LIST.lock().pop() {
			// this frame was in use earlier .. so it contains the old data
			// but here we have the liberty to safely overwrite the data in the frame
			let paddr = frame.start_address();
			let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });
			println!("[DMA] Reusing returned frame:");
			println!("  - Physical Address (for device): {:#x}", paddr);
			println!("  - Virtual Address (for CPU):  {:#x}", vaddr);

			return (paddr.as_u64() as usize, NonNull::new(vaddr.as_mut_ptr()).unwrap());
		}

		let mut frame_allocator_lock = FRAME_ALLOCATOR.lock();
		let allocator = frame_allocator_lock.as_mut().expect("Frame allocator not initialized");

		// 1. Allocate a physical frame.
		let frame = allocator
			.allocate_frame()
			.expect("Failed to allocate frame for DMA -- Out of physical frames");

		let paddr = frame.start_address();

		// 2. Calculate its virtual address in the higher-half mapping.
		let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });

		println!("[DMA] Allocating fresh frame ({} pages):", pages);
		println!("  - Physical Address (for device): {:#x}", paddr);
		println!("  - Virtual Address (for CPU):  {:#x}", vaddr);

		// NO MAPPING IS NEEDED. The bootloader's huge page mapping already covers this.
		// Here, there is no work with Pages. The Frame is an actual block of physical memory --
		// here 4 KiB in size.

		// Here, we return the physical address
		(paddr.as_u64() as usize, NonNull::new(vaddr.as_mut_ptr()).unwrap())
	}
	unsafe fn dma_dealloc(
		paddr: virtio_drivers::PhysAddr,
		vaddr: NonNull<u8>,
		pages: usize,
	) -> i32 {
		//println!("[DMA] Warning: Leaking DMA memory at paddr={:#x}, pages={}", paddr, pages);

		if pages != 1 {
			println!("[DMA] Dealloc ignored: pages={} (supported for pages < 1)", pages);
			return 0;
		}

		let frame = PhysFrame::containing_address(PhysAddr::new(paddr as u64));
		DMA_FREE_LIST.lock().push(frame);
		println!(
			"[DMA] Returned frame paddr={:#x} to free list (len={})",
			frame.start_address(),
			DMA_FREE_LIST.lock().len()
		);

		// no need to unmap the frame here .. we can just use this frame again later on

		0
	}

	unsafe fn mmio_phys_to_virt(
		paddr: virtio_drivers::PhysAddr,
		size: usize,
	) -> NonNull<u8> {
		// For MMIO, we use identity mapping with the physical memory offset
		// This avoids issues with huge pages in the bootloader's page tables
		let paddr = PhysAddr::new(paddr as u64);
		let vaddr = VirtAddr::new(paddr.as_u64() + PHYSICAL_MEMORY_OFFSET);

		println!("[MMAP] Mapping device MMIO region:");
		println!("  - Physical Address: {:#x}", paddr);
		println!("  - Virtual Address:  {:#x}", vaddr);
		println!("  - Size: {} bytes", size);

		// For MMIO regions, the bootloader should have already set up appropriate mappings
		// We just return the virtual address
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

		println!("[SHARE] Translating buffer address for device:");
		println!("  - Virtual Address (from CPU): {:#x}", vaddr);
		println!("  - Physical Address (to device): {:#x}", phyaddr);

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