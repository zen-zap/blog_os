// the library is a separate compilation unit so we need to specify the #![no_std] again
#![allow(unused, dead_code)]
#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(associated_type_defaults)]
#![feature(trivial_bounds)]
pub mod allocator;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod scanc;
pub mod serial;
pub mod task;
pub mod shell;
pub mod utils;
pub mod vga_buffer;
pub mod virtio;

extern crate alloc;
extern crate static_assertions as sa;

use core::panic::PanicInfo;

pub use crate::fs::simple_fs::GLOBALFSType;
pub static GLOBAL_FS: OnceCell<Mutex<GLOBALFSType>> = OnceCell::uninit();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
/// QemuExitCode:
/// - Success: 0x10
/// - Failure: 0x11
///
/// They shouldn't clash with the default exit codes of QEMU
pub enum QemuExitCode {
	Success = 0x10,
	Failed = 0x11,
}

/// function to exit QEMU
/// Takes in a QemuExitCode as its argument
pub fn exit_qemu(exit_code: QemuExitCode) {
	use x86_64::instructions::port::Port;

	unsafe {
		let mut port = Port::new(0xf4); // creates a new Port at 0xf4, which is the iobase of the isa-debug-exit device
		port.write(exit_code as u32);
	}
}

use bootloader_api::{BootInfo, entry_point};
use conquer_once::spin::OnceCell;
use spin::Mutex;

/// to initialize the IDT for exception handling
pub fn init() {
	gdt::init();
	interrupts::init_idt();

	unsafe {
		interrupts::PICS.lock().initialize();
	}

	x86_64::instructions::interrupts::enable(); // to enable the interrupts
	// executes the "sti" instruction called Set interrupts to enable external interrupts!
	// there is also our default hardware timer Intel 8253 .. we have to be careful .. simply
	// enabling this results in a double fault
}

/// thin wrapper around hlt instruction
pub fn hlt_loop() -> ! {
	loop {
		x86_64::instructions::hlt();
	}
}
