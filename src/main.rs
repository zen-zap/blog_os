#![allow(dead_code)]
#![allow(unreachable_code)]
#![no_std]
#![no_main] // disabling all rust level entry points 
// set the test_framework_entry_function to "test_main" and call it from the _start entry point
#![reexport_test_harness_main = "test_main"]  // reexport the generated test-harness main function as "test_main"
// To implement a custom test framework!
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)] // moved to lib.rs


use core::panic::PanicInfo;
use blog_os::println;


/// Entry point of the code
#[no_mangle]
pub extern "C" fn _start() -> ! {

    println!("Hello World{}", "!");


    blog_os::init(); // for exception things
    
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3(); // this is a breakpoint exception .. int3 is the asm

    

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    loop{}
}


/// our panic handler in general mode
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
    blog_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion()
{
    assert_eq!(1, 1);
}
