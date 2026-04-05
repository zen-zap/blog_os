//! in src/virtio/mod.rs

pub mod pci;

use crate::memory::BitmapFrameAllocator;
use crate::{debug, error, virtio_debug, warn};
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

lazy_static! {
	pub static ref PAGE_MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
	static ref DMA_FREE_LIST: Mutex<Vec<PhysFrame>> = Mutex::new(Vec::new());
}

pub static FRAME_ALLOCATOR: Mutex<BitmapFrameAllocator> = Mutex::new(BitmapFrameAllocator::new());

pub struct OsHal;

pub static mut PHYSICAL_MEMORY_OFFSET: u64 = 0;

unsafe impl Hal for OsHal {
	fn dma_alloc(
		pages: usize,
		_direction: BufferDirection,
	) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
		virtio_debug!("Allocating DMA buffer ({} pages)", pages);

		if pages > 1 {
			warn!("Single page buffers only supported");
			panic!("dma_alloc: multipage contiguous allocation not supported yet");
		}

		// before allocating .. try using any returned physical frames
		if let Some(frame) = DMA_FREE_LIST.lock().pop() {
			// this frame was in use earlier .. so it contains the old data
			// but here we have the liberty to safely overwrite the data in the frame
			let paddr = frame.start_address();
			let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });
			virtio_debug!("Reusing returned frame:");
			virtio_debug!("  - Physical Address (for device): {:#x}", paddr);
			virtio_debug!("  - Virtual Address (for CPU):  {:#x}", vaddr);

			return (paddr.as_u64() as usize, NonNull::new(vaddr.as_mut_ptr()).unwrap());
		}

		let mut frame_allocator = FRAME_ALLOCATOR.lock();

		let frame = frame_allocator
			.allocate_frame()
			.expect("Failed to allocate frame for DMA -- Out of physical frames");

		let paddr = frame.start_address();

		let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });

		virtio_debug!("Allocating DMA buffer ({} pages):", pages);
		virtio_debug!("  - Physical Address (for device): {:#x}", paddr);
		virtio_debug!("  - Virtual Address (for CPU):  {:#x}", vaddr);

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
		if pages != 1 {
			debug!("Dealloc ignored: pages={} (only single pages supported)", pages);
			return 0;
		}

		let frame = PhysFrame::containing_address(PhysAddr::new(paddr as u64));
		DMA_FREE_LIST.lock().push(frame);
		virtio_debug!(
			"Returned frame paddr={:#x} to free list (len={})",
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

		virtio_debug!("Mapping device MMIO region:");
		virtio_debug!("  - Physical Address: {:#x}", paddr);
		virtio_debug!("  - Virtual Address:  {:#x}", vaddr);
		virtio_debug!("  - Size: {} bytes", size);

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

		virtio_debug!("Translating buffer address for device:");
		virtio_debug!("  - Virtual Address (from CPU): {:#x}", vaddr);
		virtio_debug!("  - Physical Address (to device): {:#x}", phyaddr);

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
