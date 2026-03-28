// in kernel/src/memory.rs

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use virtio_drivers::device::console::Size;
use x86_64::{
	PhysAddr, VirtAddr,
	registers::control::Cr3,
	structures::paging::{
		FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags as Flags,
		PhysFrame, Size4KiB, frame, mapper::MapToError, page_table::FrameError,
	},
};

use crate::GLOBAL_FS;

/// Returns a mutable reference to the active level 4 table.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
	let (level_4_table_frame, _) = Cr3::read();
	// Cr3 holds the physical address of the highest-level page table
	let phys = level_4_table_frame.start_address();
	let virt = physical_memory_offset + phys.as_u64();
	let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

	unsafe { &mut *page_table_ptr }
}

/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn translate_addr(
	addr: VirtAddr,
	physical_memory_offset: VirtAddr,
) -> Option<PhysAddr> {
	translate_addr_inner(addr, physical_memory_offset)
}

/// Private function that is called by `translate_addr`.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats
/// the whole body of unsafe functions as an unsafe block. This function must
/// only be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(
	addr: VirtAddr,
	physical_memory_offset: VirtAddr,
) -> Option<PhysAddr> {
	// read the active level 4 frame from the CR3 register
	let (level_4_table_frame, _) = Cr3::read();

	// holds the 9-bit page table indexes
	let table_indexes = [addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()];

	let mut frame = level_4_table_frame;

	// traverse the multilevel page table
	for &index in &table_indexes {
		// convert the frame into a page table reference
		let virt = physical_memory_offset + frame.start_address().as_u64();
		let table_ptr: *const PageTable = virt.as_ptr();
		let table = unsafe { &*table_ptr };

		// read the page table entry and update "frame"
		let entry = &table[index];

		frame = match entry.frame() {
			Ok(frame) => frame,
			Err(FrameError::FrameNotPresent) => return None,
			Err(FrameError::HugeFrame) => panic!("Huge Frames are not supported"),
		};
	}

	// calculate the physical address by adding the page offset
	Some(frame.start_address() + u64::from(addr.page_offset()))
}

/// Initialize a new OffsetPageTable.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
	unsafe {
		let level_4_table = active_level_4_table(physical_memory_offset);
		OffsetPageTable::new(level_4_table, physical_memory_offset)
		// instance stays valid for the complete runtime of our kernel
	}
}

/// Allocates a physical frame and maps it to the given virtual address
/// with Ring 3 permissions for User Mode.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the VirtAddr is a valid.
pub unsafe fn map_user_page(
	mapper: &mut impl Mapper<Size4KiB>,
	frame_allocator: &mut impl FrameAllocator<Size4KiB>,
	addr: VirtAddr,
) -> Result<(), MapToError<Size4KiB>> {
	let page = Page::containing_address(addr);

	let frame = frame_allocator.allocate_frame().ok_or(MapToError::FrameAllocationFailed)?;

	let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE;

	unsafe {
		mapper.map_to(page, frame, flags, frame_allocator)?.flush();
	}

	Ok(())
}

/// Uses a bitmap to track usage of physical RAM
///
/// Can keep track of upto 4GB of physical RAM
pub struct BitmapFrameAllocator {
	/// Each bit represents a reserved physical frame of 4KiB
	bitmap: [u64; 16384],
}

// is there a contract for these functions?
// why is this not defined under a trait?
impl BitmapFrameAllocator {
	/// Creates a new BitmapFrameAllocator
	///
	/// Defaults to all 1s -- hardware-reserved initially
	/// until the bootloader tells us which regions are safe to use
	pub const fn new() -> Self {
		Self { bitmap: [u64::MAX; 16384] }
	}

	/// Initializes the bitmap using the bootloaders memory map
	///
	/// # Safety
	/// Function is unsafe because the caller must guarantee that the passed memory map is valid.
	pub unsafe fn init(
		&mut self,
		memory_map: &'static MemoryRegions,
	) {
		for mem_region in memory_map.iter() {
			if mem_region.kind == MemoryRegionKind::Usable {
				// scaling the addresses
				let st_frame = mem_region.start / 4096;
				let en_frame = mem_region.end / 4096;

				for frame in st_frame..en_frame {
					self.mark_free(frame as usize)
				}
			}
		}
	}

	/// Helper function to clear a specific bit (free)
	fn mark_free(
		&mut self,
		frame_index: usize,
	) {
		let array_idx = frame_index / 64;
		let bit_idx = frame_index % 64;

		if array_idx < self.bitmap.len() {
			self.bitmap[array_idx] &= !(1 << bit_idx);
		}
	}

	/// Returns a physical frame to the allocator so that it can be used later
	pub fn deallocate_frame(
		&mut self,
		frame: PhysFrame,
	) {
		let frame_index = (frame.start_address().as_u64() / 4096) as usize;
		self.mark_free(frame_index);
	}
}

impl Default for BitmapFrameAllocator {
	fn default() -> Self {
		BitmapFrameAllocator::new()
	}
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
	fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
		for (array_idx, block) in self.bitmap.iter_mut().enumerate() {
			if *block != u64::MAX {
				let bit_idx = (!*block).trailing_zeros();

				*block |= 1 << bit_idx; // mark as used

				// each u64 block holds 64 bits (64 frames)
				let frame_idx = (array_idx * 64) + (bit_idx as usize);
				let paddr = PhysAddr::new((frame_idx as u64) * 4096);

				return Some(PhysFrame::containing_address(paddr));
			}
		}

		None
	}
}
