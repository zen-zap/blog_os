#![allow(dead_code, unused, unreachable_code)]
#![no_std]
#![no_main]

use acpi::{
	AcpiTable,
	platform::{AcpiPlatform, InterruptModel, interrupt::IoApic},
	sdt::madt::{self, MadtEntry},
};
use alloc::{
	boxed::Box,
	rc::Rc,
	vec::{self, Vec},
};
use bootloader_api::{
	BootInfo,
	config::{BootloaderConfig, Mapping},
	entry_point,
};
use conquer_once::spin::OnceCell;
use core::{arch::asm, error, panic::PanicInfo};
use creo::{
	acpi::AcpiHandler,
	apic,
	fs::{
		self,
		simple_fs::{FileSystem, FileSystemError, GLOBALFSType, SFS},
	},
};
use creo::{
	allocator, debug, debug_if, error, framebuffer, fs_debug, info,
	interrupts::InterruptIndex::Keyboard,
	memory::{self, BitmapFrameAllocator, translate_addr},
	memory_debug, pci_debug, print, println,
	shell::shell_task,
	task::{
		Task, executor::Executor, keyboard, simple_executor::SimpleExecutor, user::enter_user_mode,
	},
	trace, trace_function, trace_here,
	virtio::{FRAME_ALLOCATOR, OsHal, PAGE_MAPPER, pci, pci::PciConfigIo},
	virtio_debug, warn,
};
use spin::Mutex;
use virtio_drivers::{
	Hal, PhysAddr,
	device::blk::VirtIOBlk,
	transport::{
		mmio::VirtIOHeader,
		pci::{PciTransport, VirtioPciError, bus::PciRoot},
	},
};
use x86_64::{
	VirtAddr,
	registers::control::Cr2,
	structures::paging::{Page, PageTable, Translate, page_table::FrameError::FrameNotPresent},
};
use zerocopy::IntoBytes;

extern crate alloc;

use creo::GLOBAL_FS;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
	let mut config = BootloaderConfig::new_default();
	config.mappings.physical_memory = Some(Mapping::Dynamic);
	config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
	let phy_offset_val = boot_info
		.physical_memory_offset
		.into_option()
		.expect("Physical memory mapping failed in bootloader");
	info!("  - Physical Memory Offset: {:#x}", phy_offset_val);

	let rsdp_addr = boot_info.rsdp_addr.into_option().expect("Bootloader failed to find ACPI RSDP");
	info!("  - ACPI RSDP Address: {:#x}", rsdp_addr);

	let framebuffer = boot_info
		.framebuffer
		.as_mut()
		.expect("Unable to extract framebuffer information from boot_info");

	// thing to note here
	// if this panics now
	// we would go to the panic handler
	// which uses info!
	// which in turn uses the print functionality ..
	// but we still don't have the framebuffer setup so no info here?

	// we have a framebuffer now, so we can safely send it to graphics writer for use
	framebuffer::GRAPHICS_WRITER
		.lock()
		.replace(framebuffer::GraphicsWriter::new(framebuffer));

	creo::init();

	let phys_mem_offset = VirtAddr::new(phy_offset_val);
	unsafe { creo::virtio::PHYSICAL_MEMORY_OFFSET = phy_offset_val }

	let mut mapper = unsafe { memory::init(phys_mem_offset) };
	unsafe {
		FRAME_ALLOCATOR.lock().init(&boot_info.memory_regions);
	}
	*PAGE_MAPPER.lock() = Some(mapper);

	{
		let mut mapper_lock = PAGE_MAPPER.lock();
		let mut allocator_lock = FRAME_ALLOCATOR.lock();
		allocator::init_heap(mapper_lock.as_mut().unwrap(), &mut *allocator_lock)
			.expect("heap initialization failed!");
	}

	apic::init(rsdp_addr, phy_offset_val);

	x86_64::instructions::interrupts::enable();

	fs::filesystem::init_filesystem();

	let mut executor = Executor::new();
	executor.spawn(Task::new(creo::shell::shell_task::run_shell_task()));
	debug!("Kernel initialization complete.");

	// enter_ring_3();

	executor.run();

	creo::hlt_loop();
}

fn enter_ring_3() -> ! {
	creo::info!("Allocating User Space Memory");

	let user_code_addr = VirtAddr::new(0x_0000_2000_0000);
	let user_stack_addr = VirtAddr::new(0x_0000_2000_1000);
	// 4KiB away -- standard size of a memory page

	let mut mapper_lock = creo::virtio::PAGE_MAPPER.lock();
	let mut allocator_lock = creo::virtio::FRAME_ALLOCATOR.lock();

	unsafe {
		creo::memory::map_user_page(
			mapper_lock.as_mut().unwrap(),
			&mut *allocator_lock,
			user_code_addr,
		)
		.expect("Failed to map user code page");

		creo::memory::map_user_page(
			mapper_lock.as_mut().unwrap(),
			&mut *allocator_lock,
			user_stack_addr,
		)
		.expect("Failed to map user stack page");
	}

	drop(mapper_lock);
	drop(allocator_lock);

	creo::info!("Writing User Space Application");

	// this is the machine code for `jmp $` -- an infinite loop
	unsafe {
		let code_ptr = user_code_addr.as_mut_ptr::<u8>();
		code_ptr.write(0xEB);
		code_ptr.add(1).write(0xFE);
	}

	let user_stack_ptr = user_stack_addr.as_u64() + 4096;

	creo::info!("Executing jump to Ring 3");

	let mut user_code = creo::gdt::GDT.1.user_code_selector;
	let mut user_data = creo::gdt::GDT.1.user_data_selector;

	user_code.0 |= 3;
	user_data.0 |= 3;

	unsafe {
		creo::task::user::enter_user_mode(
			user_code.0,
			user_data.0,
			user_code_addr.as_u64(),
			user_stack_ptr,
		);
	}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	error!("==================================================");
	error!("                 KERNEL PANIC                     ");
	error!("==================================================");

	// extract the file and line number if available
	if let Some(location) = info.location() {
		error!("Location : {}:{}", location.file(), location.line());
	} else {
		error!("Location : Unknown");
	}

	error!("Message  : {}", info.message());
	error!("--------------------------------------------------");

	let rip: u64;
	let rbp: u64;
	let rsp: u64;

	// instruction, base, and stack pointers for the current frame
	unsafe {
		asm!(
			"lea {}, [rip]",
			"mov {}, rbp",
			"mov {}, rsp",
			out(reg) rip,
			out(reg) rbp,
			out(reg) rsp,
			options(nomem, nostack, preserves_flags),
		);
	}

	error!("CPU State (Inside Panic Handler):");
	error!("  RIP: {:#018x}", rip);
	error!("  RBP: {:#018x}", rbp);
	error!("  RSP: {:#018x}", rsp);
	error!("--------------------------------------------------");
	error!("Stack Backtrace (Use llvm-addr2line to decode):");

	let mut current_rbp = rbp;
	let mut depth = 0;

	// walking the base pointer chain
	while current_rbp != 0 && depth < 20 {
		let ret_addr = unsafe { *((current_rbp + 8) as *const u64) };

		if ret_addr == 0 {
			break;
		}

		error!("  [{:>2}] {:#018x}", depth, ret_addr);

		// dereference the current RBP to get the caller's RBP
		current_rbp = unsafe { *(current_rbp as *const u64) };
		depth += 1;
	}

	error!("==================================================");

	creo::hlt_loop();
}
