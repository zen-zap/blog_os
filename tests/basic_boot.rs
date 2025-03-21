// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]  // to use our custom test framework
#![test_runner(blog_os::test_runner)] // instead of reimplementing it ... take it from somewhere else
#![reexport_test_harness_main="test_main"] // to set the main function of the test

use core::panic::PanicInfo;
use blog_os::println;


/// all integration tests are their own executables and hence have their own entry_points
/// 
/// - all crate attributes are made again
///
/// - panic handler is also made
#[no_mangle]
pub extern "C" fn _start() -> !
{
    test_main();

    loop{}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
    blog_os::test_panic_handler(info)
}

#[test_case]
pub fn test_println()
{
    println!("println! works fine!");
}
