#![allow(dead_code, unused, unreachable_code)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![feature(custom_test_frameworks)]
#![test_runner(creo::test_runner)]

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use bootloader_api::{
	BootInfo,
	config::{BootloaderConfig, Mapping},
	entry_point,
};
use conquer_once::spin::OnceCell;
use core::{arch::asm, panic::PanicInfo};
use creo::fs::simple_fs::{FileSystem, FileSystemError, GLOBALFSType, SFS};
use creo::{
	allocator, debug, debug_if, error, fs_debug, info,
	interrupts::InterruptIndex::Keyboard,
	memory::{self, BootInfoFrameAllocator, translate_addr},
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

const RUN_FILESYSTEM_SELF_CHECK: bool = false;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
	let mut config = BootloaderConfig::new_default();
	config.mappings.physical_memory = Some(Mapping::Dynamic);
	config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn run_filesystem_self_check(fs: &mut GLOBALFSType) {
	info!("--- Starting SFS C-W-R-D Test ---");
	let filename = "test.txt";

	fs_debug!("Attempting cleanup of '{}'...", filename);
	match fs.delete_file(filename) {
		Ok(_) => {
			fs_debug!("Cleanup successful.");
		},
		Err(_) => {
			fs_debug!("File not present, no cleanup needed.");
		},
	}

	fs_debug!("Creating '{}'...", filename);
	let handle = match fs.create_file(filename) {
		Ok(h) => {
			info!("File created successfully with handle {:?}", h);
			h
		},
		Err(e) => {
			error!("Failed to create file: {:?}", e);
			creo::hlt_loop();
		},
	};

	let lines_to_write = &["Hello from your SFS!", "This is the second line."];
	fs_debug!("Writing content to file...");
	match fs.write_file_lines(handle, lines_to_write) {
		Ok(_) => info!("Content written successfully."),
		Err(e) => {
			error!("Failed to write to file: {:?}", e);
			creo::hlt_loop();
		},
	}

	fs_debug!("Reading content back from file...");
	match fs.read_file(handle) {
		Ok(content) => {
			info!("Read success! Content:\n---\n{}\n---", content);

			let expected_content = lines_to_write.join("\n");
			if content == expected_content {
				info!("Verification SUCCESS: Content matches!");
			} else {
				error!("Verification FAILED: Content mismatch!");
			}
		},
		Err(e) => {
			error!("Failed to read from file: {:?}", e);
			creo::hlt_loop();
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
}

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
	let phy_offset_val = boot_info
		.physical_memory_offset
		.into_option()
		.expect("Physical memory mapping failed in bootloader");

	info!("  - Physical Memory Offset: {:#x}", phy_offset_val);
	debug!("  - Memory Map:");
	for region in boot_info.memory_regions.iter() {
		debug!(
			"    - Start: {:#010x}, End: {:#010x}, Size: {} KB, Type: {:?}",
			region.start,
			region.end,
			region.end.saturating_sub(region.start) / 1024,
			region.kind
		);
	}
	info!("=================");

	creo::init(); // for the exception things

	let phys_mem_offset = VirtAddr::new(phy_offset_val);

	// Set the physical memory offset for VirtIO
	unsafe { creo::virtio::PHYSICAL_MEMORY_OFFSET = phy_offset_val }

	let mut mapper = unsafe { memory::init(phys_mem_offset) };
	// get the physical frames that you wanna map
	let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

	*FRAME_ALLOCATOR.lock() = Some(frame_allocator);
	*PAGE_MAPPER.lock() = Some(mapper);

	{
		let mut mapper_lock = PAGE_MAPPER.lock();
		let mut allocator_lock = FRAME_ALLOCATOR.lock();
		// here we do the mapping of the physical frames
		allocator::init_heap(mapper_lock.as_mut().unwrap(), allocator_lock.as_mut().unwrap())
			.expect("heap initialization failed!");
	}

	let pci_config_access = PciConfigIo;
	let mut pci_root = PciRoot::new(pci_config_access);

	if let Some(device_function) = pci::scan(&mut pci_root) {
		let mut pci_root_mut = pci_root;
		let transport = PciTransport::new::<OsHal, _>(&mut pci_root_mut, device_function)
			.expect("Failed to create PCI transport");

		let mut blk_dev =
			VirtIOBlk::<OsHal, _>::new(transport).expect("failed to create blk driver");

		let mut fs = match SFS::mount(blk_dev) {
			Ok(fs) => fs,
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

		if RUN_FILESYSTEM_SELF_CHECK {
			run_filesystem_self_check(&mut fs);
		}

		GLOBAL_FS
			.try_init_once(|| Mutex::new(fs))
			.expect("Failed to initialize GLOBAL_FS");
	} else {
		error!("No VirtIO block device found!");
	}

	let mut executor = Executor::new();

	creo::info!("Allocating User Space Memory");

	let user_code_addr = VirtAddr::new(0x_0000_2000_0000);
	let user_stack_addr = VirtAddr::new(0x_0000_2000_1000);
	// 4KiB away -- standard size of a memory page

	let mut mapper_lock = creo::virtio::PAGE_MAPPER.lock();
	let mut allocator_lock = creo::virtio::FRAME_ALLOCATOR.lock();

	unsafe {
		creo::memory::map_user_page(
			mapper_lock.as_mut().unwrap(),
			allocator_lock.as_mut().unwrap(),
			user_code_addr,
		)
		.expect("Failed to map user code page");

		creo::memory::map_user_page(
			mapper_lock.as_mut().unwrap(),
			allocator_lock.as_mut().unwrap(),
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

	debug!("Kernel initialization complete - starting task executor");
	executor.run();

	creo::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	error!("KERNEL PANIC: {}\n", info);

	let rip: u64;
	unsafe {
		asm!(
			"lea {rip}, [rip]",
			rip = out(reg) rip,
			options(nomem, nostack, preserves_flags),
		);
	}

	error!("RIP: {:#018x}", rip);

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
		let ret = unsafe { *((rbp + 8) as *const u64) };
		error!("  {:#018x}", ret);
		rbp = unsafe { *(rbp as *const u64) };

		stack_trace_count += 1;
	}

	creo::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	creo::test_panic_handler(info)
}

#[test_case]
fn one_one_assertion() {
	assert_eq!(1, 1);
}

#[test_case]
fn test_interrupt_handler() {
	x86_64::instructions::interrupts::int3();
}

async fn async_number_69() -> u32 {
	69
}

async fn example_task() {
	let number = async_number_69().await;
	debug!("async number: {}", number);
}
