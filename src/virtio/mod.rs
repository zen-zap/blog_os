//! in src/virtio/mod.rs

pub mod pci;

use crate::memory::BootInfoFrameAllocator;
use crate::println;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal};
use x86_64::structures::paging::{Mapper, Page, PageTableFlags};
use x86_64::{
	PhysAddr, VirtAddr,
	structures::paging::{FrameAllocator, OffsetPageTable},
};

// Global reference to the frame allocator
// gotta set it in kernel init function
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
		if pages > 1 {
			panic!("dma_alloc: multipage contiguous allocation not supported yet");
		}

		let mut frame_allocator = FRAME_ALLOCATOR.lock();
		let allocator = frame_allocator.as_mut().expect("Frame allocator not initialized");

		// Allocate a physical frame
		let frame = allocator.allocate_frame().expect("Failed to allocate frame for DMA");
		let paddr = frame.start_address();

		// SIMPLE FIX: Use the bootloader's identity mapping instead of creating new mappings
		// The bootloader maps all physical memory to virtual addresses starting at PHYSICAL_MEMORY_OFFSET
		let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });

		let virtio_paddr = paddr.as_u64() as usize;
		(virtio_paddr, NonNull::new(vaddr.as_mut_ptr()).unwrap())
	}
	unsafe fn dma_dealloc(
		paddr: virtio_drivers::PhysAddr,
		vaddr: NonNull<u8>,
		pages: usize,
	) -> i32 {
		println!("[VirtIO] Warning: Leaking DMA memory at paddr={:#x}, pages={}", paddr, pages);
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
