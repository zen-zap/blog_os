//! in kernel/src/lib.rs
//!
//! This is the root of the kernel library. It exposes all core modules,
//! manages global state (like the file system), and provides primary
//! initialization routines for the CPU state.

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(associated_type_defaults)]
#![feature(trivial_bounds)]
#![allow(unused, dead_code)]

extern crate alloc;
extern crate static_assertions as sa;

pub mod acpi;
pub mod allocator;
pub mod apic;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod scanc;
pub mod serial;
pub mod shell;
pub mod task;
pub mod utils;
pub mod vga_buffer;
pub mod virtio;

use conquer_once::spin::OnceCell;
use spin::Mutex;
use x86_64::instructions::port::Port;

pub use crate::fs::simple_fs::GLOBALFSType;

/// The global handle to the Simple File System (SFS).
/// Initialized once the VirtIO block device is mounted during boot.
pub static GLOBAL_FS: OnceCell<Mutex<GLOBALFSType>> = OnceCell::uninit();

/// Initializes core CPU structures (GDT, IDT) and enables hardware interrupts.
///
/// Note: The APIC and I/O APIC must be initialized separately after the
/// memory heap is established.
pub fn init() {
	gdt::init();
	interrupts::init_idt();
}

/// A thin wrapper around the `hlt` instruction.
/// Puts the CPU to sleep until the next interrupt arrives, saving power.
pub fn hlt_loop() -> ! {
	loop {
		x86_64::instructions::hlt();
	}
}

/// Standardized exit codes for the `isa-debug-exit` QEMU device.
/// These specific codes (0x10, 0x11) avoid clashing with QEMU's default internal exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
	Success = 0x10,
	Failed = 0x11,
}

/// Signals QEMU to shutdown the virtual machine with the provided exit code.
/// This requires the `isa-debug-exit` device to be configured in QEMU's launch args.
pub fn exit_qemu(exit_code: QemuExitCode) {
	unsafe {
		let mut port = Port::new(0xf4); // iobase of the isa-debug-exit device
		port.write(exit_code as u32);
	}
}
