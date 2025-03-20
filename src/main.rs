#![allow(dead_code)]
#![allow(unreachable_code)]
#![no_std]
#![no_main] // disabling all rust level entry points 
// set the test_framework_entry_function to "test_main" and call it from the _start entry point
#![reexport_test_harness_main = "test_main"]  // reexport the generated test-harness main function as "test_main"
// To implement a custom test framework!
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]


// #![feature(asm)] // for inline assembly -- has been a feature since a while in the nightly versions

// we need a panic handler .. the std implements its own .. we need our own .. since we're not
// gonna have the std
//
use core::panic::PanicInfo;
mod vga_buffer;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

static HELLO: &[u8] = b"Hello World";


#[no_mangle] // read about this a bit
pub extern "C" fn _start() -> ! {
    // vga_buffer::print_something();

    // use core::fmt::Write;
    // vga_buffer::WRITER.lock().write_str("hello again!").unwrap();
    // write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.6969).unwrap();
    
    println!("Hello World{}", "!"); 
    // don't have to import the macro since it already lives in the root namespace
    
    // panic!("This is a test panic message!");


    #[cfg(test)]
    test_main(); // call the re-exported test harness when testing

    loop{}
}

// to include nightly features we can use feature flags and use them


#[cfg(test)]
/// takes the tests(functions) as arguments
/// iterates over each function
/// - Fn() is a trait [functions that don't take arguments and don't return anything] and dyn Fn() is a trait object
///
/// - we just iterate over this list of functins ... used for testing
pub fn test_runner(tests: &[&dyn Fn()])
{
    println!("Running {} tests", tests.len());
    for test in tests{
        test(); // call each test function in the list
    }

    // to exit_qemu -- cargo considers all error codes other than 0 as Failures
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion()
{
    println!("trivial assertion .... don't mind me!");
    assert_eq!(1, 1);
    println!("[ok]");
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
pub fn exit_qemu(exit_code: QemuExitCode)
{
    use x86_64::instructions::port::Port;

    unsafe
    {
        let mut port = Port::new(0xf4); // creates a new Port at 0xf4, which is the iobase of the isa-debug-exit device
        port.write(exit_code as u32);
    }
}
