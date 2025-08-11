#![allow(dead_code, unused, unreachable_code)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use blog_os::fs::simple_fs::{FileSystem, FileSystemError, SFS};
use blog_os::{
	allocator,
	interrupts::InterruptIndex::Keyboard,
	memory::{self, BootInfoFrameAllocator, translate_addr},
	print, println,
	task::{Task, executor::Executor, keyboard, simple_executor::SimpleExecutor},
	virtio::{FRAME_ALLOCATOR, OsHal, PAGE_MAPPER, pci, pci::PciConfigIo},
};
use bootloader::{BootInfo, entry_point};
use core::{arch::asm, panic::PanicInfo};
use virtio_drivers::{
	Hal, PhysAddr,
	device::blk::VirtIOBlk,
	transport::{
		mmio::VirtIOHeader,
		pci::{PciTransport, bus::PciRoot},
	},
};
use x86_64::{
	VirtAddr,
	registers::control::Cr2,
	structures::paging::{Page, PageTable, Translate, page_table::FrameError::FrameNotPresent},
};
use zerocopy::IntoBytes;

extern crate alloc;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
	println!("Hello zen-zap{}", "!");

	println!("[INFO] Boot Info Received:");
	println!("  - Physical Memory Offset: {:#x}", boot_info.physical_memory_offset);
	println!("  - Memory Map:");
	for region in boot_info.memory_map.iter() {
		println!(
			"    - Start: {:#010x}, End: {:#010x}, Size: {} KB, Type: {:?}",
			region.range.start_addr(),
			region.range.end_addr(),
			region.range.end_addr().saturating_sub(region.range.start_addr()) / 1024,
			region.region_type
		);
	}
	println!("=================");

	blog_os::init(); // for the exception things

	let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

	// Set the physical memory offset for VirtIO
	unsafe {
		blog_os::virtio::PHYSICAL_MEMORY_OFFSET = boot_info.physical_memory_offset;
	}

	let mut mapper = unsafe { memory::init(phys_mem_offset) };
	let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

	*FRAME_ALLOCATOR.lock() = Some(frame_allocator);
	*PAGE_MAPPER.lock() = Some(mapper);

	{
		let mut mapper_lock = PAGE_MAPPER.lock();
		let mut allocator_lock = FRAME_ALLOCATOR.lock();

		allocator::init_heap(mapper_lock.as_mut().unwrap(), allocator_lock.as_mut().unwrap())
			.expect("heap initialization failed!");
	}

	println!("[PCI] Initializing PCI and finding devices");
	let pci_config_access = PciConfigIo;
	let mut pci_root = PciRoot::new(pci_config_access);

	if let Some(device_function) = pci::scan(&mut pci_root) {
		let mut pci_root_mut = pci_root;
		let transport = PciTransport::new::<OsHal, _>(&mut pci_root_mut, device_function)
			.expect("Failed to create PCI transport");

		println!("[VirtIO] PCI transport created successfully.");

		let mut blk_dev =
			VirtIOBlk::<OsHal, _>::new(transport).expect("failed to create blk driver");

		println!("[VirtIO] Block Device Initialized! Capacity: {} sectors", blk_dev.capacity());

		// 1. Create a buffer for one sector (512 bytes).
		let mut buffer = [0u8; 512];

		// 2. Call the simple, blocking read_blocks method.
		// This function will not return until the read is complete.
		println!("[VirtIO] Reading block 0...");
		blk_dev.read_blocks(0, &mut buffer).expect("read_blocks failed");

		// 3. The data is now in the buffer.
		println!("[VirtIO] Successfully read block 0! (First 16 bytes: {:02x?})", &buffer[0..16]);

		// Test write then read
		println!("[VirtIO] Testing write/read...");

		let test_data = b"hello world! this is a test message from blog_os kernel!";
		let mut write_buffer = [0u8; 512];
		write_buffer[..test_data.len()].copy_from_slice(test_data);

		println!("[VirtIO] Writing test data to block 0...");
		blk_dev.write_blocks(0, &write_buffer).expect("write_blocks failed");

		let mut read_buffer = [0u8; 512];
		println!("[VirtIO] Reading back from block 0...");
		blk_dev.read_blocks(0, &mut read_buffer).expect("read_blocks failed");

		println!(
			"[VirtIO] Read back: '{}'",
			core::str::from_utf8(&read_buffer[..test_data.len()]).unwrap_or("invalid utf8")
		);

		if read_buffer[..test_data.len()] == write_buffer[..test_data.len()] {
			println!("[VirtIO] Write/Read test PASSED!");
		} else {
			println!("[VirtIO] Write/Read test FAILED!");
		}

		println!("[SFS] Initializing...");

		let mut fs = match SFS::mount(blk_dev) {
			Ok(fs) => {
				println!("[SFS] Filesystem mounted successfully");
				fs
			},
			Err(_) => {
				println!("[SFS] Mount failed or filesystem not found! Formatting disk...");

				// We need to re-create the block device
				let mut pci_root_for_format = PciRoot::new(pci_config_access);
				let transport =
					PciTransport::new::<OsHal, _>(&mut pci_root_for_format, device_function)
						.expect("Failed to re-create transport for format");

				let blk_dev_for_format = VirtIOBlk::<OsHal, _>::new(transport)
					.expect("Failed to re-create blk_dev for format");

				let mut fs = SFS::format(blk_dev_for_format).expect("Failed to format disk.");

				fs.init_root_directory().expect("Failed to init root directory");

				fs
			},
		};

		println!("[SFS] Testing File creation..");
		match fs.create_file("hello.txt") {
			Ok(handle) => println!("File created with handle {:?}", handle),
			Err(e) => println!("Failed to create file: {:?}", e),
		}

		// You can try creating it again to test the "FileExists" error path
		match fs.create_file("hello.txt") {
			Ok(_) => println!("[FS] This should not happen!"),
			Err(e) => println!("[FS] Correctly failed to create existing file: {:?}", e),
		}
	} else {
		println!("[PCI] No VirtIO block device found.");
	}

	let mut executor = Executor::new();

	executor.spawn(Task::new(example_task()));
	executor.spawn(Task::new(keyboard::print_keypresses()));
	executor.run();

	#[cfg(test)]
	test_main();

	println!("It did not crash!");
	blog_os::hlt_loop();
}

/// our panic handler in general mode
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	println!("KERNEL PANIC: {}\n", info);

	// reading RIP [current instruction pointer]
	let rip: u64;
	unsafe {
		asm!(
			"lea {rip}, [rip]", // load the effective address of the next instruction
			rip = out(reg) rip,
			options(nomem, nostack, preserves_flags),
		);
	}

	println!("RIP: {:#018x}", rip);

	// stack backtrace
	println!("\nStack Backtrace:");
	let mut rbp: u64;
	unsafe {
		asm!(
			"mov {rbp}, rbp",
			rbp = out(reg) rbp,
			options(nomem, preserves_flags),
		)
	}

	let mut stack_trace_count = 0;

	while rbp != 0 && stack_trace_count < 20 {
		// return address is saved at [RBP + 8]
		let ret = unsafe { *((rbp + 8) as *const u64) };
		println!("  {:#018x}", ret);
		// the previous frame's RBP is at [RBP]
		rbp = unsafe { *(rbp as *const u64) };

		stack_trace_count += 1;
	}

	// halt it forever,
	blog_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	blog_os::test_panic_handler(info)
}

#[test_case]
fn one_one_assertion() {
	assert_eq!(1, 1);
}

async fn async_number_69() -> u32 {
	69
}

async fn example_task() {
	let number = async_number_69().await;
	println!("async number: {}", number);
}
