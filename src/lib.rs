// the library is a separate compilation unit so we need to specify the #![no_std] again
#![allow(unused, dead_code)]
#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![feature(associated_type_defaults)]

pub mod allocator;
// pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod scanc;
pub mod serial;
pub mod task;
pub mod vga_buffer;
pub mod virtio;

extern crate alloc;

use core::panic::PanicInfo;

/// trait for `test` functions
pub trait Testable {
	/// to run the function implementing this trait
	fn run(&self) -> (); // Fn() trait
}

impl<T> Testable for T
where
	T: Fn(),
{
	/// helps print the name and an [ok] message if the test runs succcessfully
	fn run(&self) -> () {
		serial_print!("{}....\t", core::any::type_name::<T>()); // any::type_name is directly
		// implemented by the compiler
		// for functions their type is their name                  // and returns a string
		// description of every type
		self();
		serial_println!("[ok]");
	}
}

// #[cfg(test)] not added so that it is available to all executables and itegration tests -- it is
// also public
/// takes the tests(functions) as arguments
/// iterates over each function
/// - `Fn()` is a trait [functions that don't take arguments and don't return anything] and dyn Fn() is a trait object
///
/// - we just iterate over this list of functins ... used for testing
/// - takes a reference to slice of references to trait objects
pub fn test_runner(tests: &[&dyn Testable]) {
	serial_println!("Running {} tests", tests.len());
	for test in tests {
		test.run(); // call each test function in the list
	}

	// to exit_qemu -- cargo considers all error codes other than 0 as Failures
	exit_qemu(QemuExitCode::Success);
}

/// our panic handler in test mode -- no need to gate it here .... the actual function is gated in
/// main.rs using #[cfg(test)]
pub fn test_panic_handler(info: &PanicInfo) -> ! {
	serial_println!("[failed] \n");
	serial_println!("Error: {} \n", info);
	exit_qemu(QemuExitCode::Failed);

	serial_println!("QemuExitCode::Failed didn't work");

	hlt_loop();
}

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

use bootloader::{BootInfo, entry_point};

#[cfg(test)]
entry_point!(test_kernel_main);

/// actual entry point?
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
	init(); // for breakpoints
	test_main();
	hlt_loop();
}

///// Entry point for `cargo test`
//#[cfg(test)]
//#[no_mangle] // read about this a bit
//pub extern "C" fn _start() -> ! {
//
//    init(); // for the breakpoint checking -- completely separate from main.rs ... gotta make a new
//            // IDT for testing too uk ..
//
//    #[cfg(test)]
//    test_main(); // call the re-exported test harness when testing
//
//    hlt_loop();
//}

/// panic handler for the library in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	test_panic_handler(info)
}

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
