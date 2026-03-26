#![allow(dead_code, unused, unreachable_code)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use blog_os::fs::simple_fs::{FileSystem, FileSystemError, SFS, GLOBALFSType};
use blog_os::{
	allocator,
	debug,
	debug_if,
	error,
	fs_debug,
	info,
	interrupts::InterruptIndex::Keyboard,
	memory::{self, BootInfoFrameAllocator, translate_addr},
	memory_debug,
	pci_debug,
	print,
	println,
	task::{Task, executor::Executor, keyboard, simple_executor::SimpleExecutor},
	trace,
	trace_function,
	trace_here,
	virtio::{FRAME_ALLOCATOR, OsHal, PAGE_MAPPER, pci, pci::PciConfigIo},
	virtio_debug,
	warn,
	shell::shell_task,
};
use bootloader::{BootInfo, entry_point};
use core::{arch::asm, panic::PanicInfo};
use conquer_once::spin::OnceCell;
use spin::Mutex;
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

use blog_os::GLOBAL_FS;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
	info!("Kernel starting up...");

	info!("Boot Info Received:");
	info!("  - Physical Memory Offset: {:#x}", boot_info.physical_memory_offset);
	debug!("  - Memory Map:");
	for region in boot_info.memory_map.iter() {
		debug!(
			"    - Start: {:#010x}, End: {:#010x}, Size: {} KB, Type: {:?}",
			region.range.start_addr(),
			region.range.end_addr(),
			region.range.end_addr().saturating_sub(region.range.start_addr()) / 1024,
			region.region_type
		);
	}
	info!("=================");

	blog_os::init(); // for the exception things
	memory_debug!("Initializing memory subsystem...");

	let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

	// Set the physical memory offset for VirtIO
	unsafe {
		blog_os::virtio::PHYSICAL_MEMORY_OFFSET = boot_info.physical_memory_offset;
	}
	virtio_debug!("Set physical memory offset: {:#x}", boot_info.physical_memory_offset);

	let mut mapper = unsafe { memory::init(phys_mem_offset) };
	// get the physical frames that you wanna map
	let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
	memory_debug!("Initialized frame allocator and page mapper");

	*FRAME_ALLOCATOR.lock() = Some(frame_allocator);
	*PAGE_MAPPER.lock() = Some(mapper);

	{
		let mut mapper_lock = PAGE_MAPPER.lock();
		let mut allocator_lock = FRAME_ALLOCATOR.lock();
		// here we do the mapping of the physical frames
		allocator::init_heap(mapper_lock.as_mut().unwrap(), allocator_lock.as_mut().unwrap())
			.expect("heap initialization failed!");
	}
	memory_debug!("Heap initialization completed");

	info!("Initializing PCI and finding devices");
	let pci_config_access = PciConfigIo;
	let mut pci_root = PciRoot::new(pci_config_access);

	if let Some(device_function) = pci::scan(&mut pci_root) {
		let mut pci_root_mut = pci_root;
		let transport = PciTransport::new::<OsHal, _>(&mut pci_root_mut, device_function)
			.expect("Failed to create PCI transport");

		info!("PCI transport created successfully");

		let mut blk_dev =
			VirtIOBlk::<OsHal, _>::new(transport).expect("failed to create blk driver");

		info!("Block Device Initialized! Capacity: {} sectors", blk_dev.capacity());

		// 1. Create a buffer for one sector (512 bytes).
		let mut buffer = [0u8; 512];

		// 2. Call the simple, blocking read_blocks method.
		// This function will not return until the read is complete.
		virtio_debug!("Reading block 0...");
		blk_dev.read_blocks(0, &mut buffer).expect("read_blocks failed");

		// 3. The data is now in the buffer.
		virtio_debug!("Successfully read block 0! (First 16 bytes: {:02x?})", &buffer[0..16]);

		// Removed the tests on the blocks here since they corrupted the superblock

		info!("Initializing Simple File System...");

		let mut fs = match SFS::mount(blk_dev) {
			Ok(fs) => {
				info!("Filesystem mounted successfully");
				fs
			},
			Err(_) => {
				warn!("Mount failed or filesystem not found! Formatting disk...");

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

		// --- BEGIN SFS C-W-R-D TEST ---
		info!("--- Starting SFS C-W-R-D Test ---");
		let filename = "test.txt";

		fs_debug!("Attempting cleanup of '{}'...", filename);
		match fs.delete_file(filename) {
			Ok(_) => { fs_debug!("Cleanup successful."); },
			Err(_) => { fs_debug!("File not present, no cleanup needed."); },
		}

		fs_debug!("Creating '{}'...", filename);
		let handle = match fs.create_file(filename) {
			Ok(h) => {
				info!("File created successfully with handle {:?}", h);
				h
			},
			Err(e) => {
				error!("Failed to create file: {:?}", e);
				blog_os::hlt_loop(); // Can't continue test
			},
		};

		let lines_to_write = &["Hello from your SFS!", "This is the second line."];
		fs_debug!("Writing content to file...");
		match fs.write_file_lines(handle, lines_to_write) {
			Ok(_) => info!("Content written successfully."),
			Err(e) => {
				error!("Failed to write to file: {:?}", e);
				blog_os::hlt_loop(); // Can't continue test
			},
		}

		fs_debug!("Reading content back from file...");
		match fs.read_file(handle) {
			Ok(content) => {
				info!("Read success! Content:\n---\n{}\n---", content);

				// Verification check
				let expected_content = lines_to_write.join("\n");
				if content == expected_content {
					info!("Verification SUCCESS: Content matches!");
				} else {
					error!("Verification FAILED: Content mismatch!");
				}
			},
			Err(e) => {
				error!("Failed to read from file: {:?}", e);
				blog_os::hlt_loop(); // Can't continue test
			},
		}

		fs_debug!("Listing root directory...");
		match fs.list_file(".") {
			Ok(files) => info!("Files in root: {:?}", files),
			Err(e) => error!("Failed to list files: {:?}", e),
		}

		fs_debug!("Deleting '{}'...", filename);
		match fs.delete_file(filename) {
			Ok(_) => info!("File deleted successfully."),
			Err(e) => error!("Failed to delete file: {:?}", e),
		}

		fs_debug!("Listing root directory after delete...");
		match fs.list_file(".") {
			Ok(files) => info!("Files in root: {:?}", files),
			Err(e) => error!("Failed to list files: {:?}", e),
		}

		info!("--- SFS Test Complete ---");
		// --- END SFS C-W-R-D TEST ---

		info!("Moving filesystem to global static ... ");
		GLOBAL_FS.try_init_once(|| Mutex::new(fs)).expect("Failed to initialize GLOBAL_FS");
		// we need to make sure this transfer of file system happens other anything else attempts
		// to use the filesystem for something
		info!("Filesystem is now global");

	} else {
		error!("No VirtIO block device found!");
	}


	let mut executor = Executor::new();

	executor.spawn(Task::new(example_task()));
	/*executor.spawn(Task::new(keyboard::print_keypresses()));*/
	executor.spawn(Task::new(shell_task::run_shell_task()));
	executor.run();

	debug!("Kernel initialization complete - starting task executor");
	blog_os::hlt_loop();
}

// #[cfg(test)]
// fn maybe_run_tests() {
// 	// When running `cargo test` the harness provides `test_main`
// 	// via `#![reexport_test_harness_main = "test_main"]`.
// 	test_main();
// }
//
// // no op in normal boot
// #[cfg(not(test))]
// fn maybe_run_tests() {}
//

/// our panic handler in general mode
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	error!("KERNEL PANIC: {}\n", info);

	// reading RIP [current instruction pointer]
	let rip: u64;
	unsafe {
		asm!(
			"lea {rip}, [rip]", // load the effective address of the next instruction
			rip = out(reg) rip,
			options(nomem, nostack, preserves_flags),
		);
	}

	error!("RIP: {:#018x}", rip);

	// stack backtrace
	error!("\nStack Backtrace:");
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
		error!("  {:#018x}", ret);
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

#[test_case]
fn test_interrupt_handler() {
    // Test that breakpoint exception works
    x86_64::instructions::interrupts::int3(); // Should not panic
}

async fn async_number_69() -> u32 {
	69
}

async fn example_task() {
	let number = async_number_69().await;
	debug!("async number: {}", number);
}
