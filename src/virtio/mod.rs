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
		_direction: BufferDirection,
	) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
		if pages > 1 {
			panic!("dma_alloc: multipage contiguous allocation not supported yet");
		}

		let mut frame_allocator_lock = FRAME_ALLOCATOR.lock();
		let allocator = frame_allocator_lock.as_mut().expect("Frame allocator not initialized");

		// 1. Allocate a physical frame.
		let frame = allocator.allocate_frame().expect("Failed to allocate frame for DMA");
		let paddr = frame.start_address();

		// 2. Calculate its virtual address in the higher-half mapping.
		let vaddr = VirtAddr::new(paddr.as_u64() + unsafe { PHYSICAL_MEMORY_OFFSET });

		println!("[DMA] Allocating DMA buffer ({} pages):", pages);
		println!("  - Physical Address (for device): {:#x}", paddr);
		println!("  - Virtual Address (for CPU):  {:#x}", vaddr);

		// 3. NO MAPPING IS NEEDED. The bootloader's huge page mapping already covers this.

		// 4. Return the addresses.
		(paddr.as_u64() as usize, NonNull::new(vaddr.as_mut_ptr()).unwrap())
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
