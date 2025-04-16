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
use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main); // defines the real low-level _start for us -- this thing is
                           // type-checked so you can't really modify the signature on a whim

fn kernel_main(boot_info: &'static BootInfo) -> ! {


}


/// Entry point of the code
#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {

    println!("Hello World{}", "!");


    blog_os::init(); // for exception things
    
    // invoke a breakpoint exception
    // x86_64::instructions::interrupts::int3(); // this is a breakpoint exception .. int3 is the asm

    // triggerring a page fault -- to demonstrate a double fault
    //unsafe {
    //    *(0xdeadbeef as *mut u8) = 42;
    //};
    //
    // println!("Handled the breakpoint_exception! .. caused by int3 instruction");
   
    #[allow(unconditional_recursion)]
    fn stack_overflow()
    {
        stack_overflow();
    }

    // we just used the address that the page fault handler returned
    let ptr = 0x2047b9 as *mut u8;

    // read from a code page
    unsafe {
        let _x = *ptr;
    }
    println!("read worked from address: {:?}", ptr);

    // write to a code page
    //unsafe {
    //    *ptr = 42; // try to store 42 at that address?
    //}
    //println!("write worked");

    use x86_64::registers::control::Cr3;

    // as we all know that CR3 holds the base level 4 page table -- btw all of the levels have
    // names .. check them out
    let (level_4_page_table, _) = Cr3::read();
    println!("level 4 page table: {:?}", level_4_page_table.start_address());

    // trigger a stack_overflow
    // stack_overflow();

    // println!("Handled the double_fault!");

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    blog_os::hlt_loop(); 
}


/// our panic handler in general mode
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    
    blog_os::hlt_loop();
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
