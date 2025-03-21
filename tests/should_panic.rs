// in tests/should_panic.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use blog_os::{QemuExitCode, exit_qemu, serial_println, serial_print};


/// panic handler for should_panic tests
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {

    // any function within this module that reaches the panic handler is a correct function!
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);

    loop{}
}


/// test_runner defined inside should_panic
pub fn test_runner(tests: &[&dyn Fn()])
{
    serial_println!("Running {} tests..", tests.len());

    for test in tests
    {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }

    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn should_fail() 
{
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1); // this will panic leading to the panic handler .. 
}


#[no_mangle]
pub extern "C" fn _start() -> !
{
    test_main();

    loop{}
}
