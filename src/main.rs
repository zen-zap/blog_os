#![allow(dead_code)]
#![allow(unreachable_code)]
#![no_std]
#![no_main] // disabling all rust level entry points 
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
    //
    panic!("This is a test panic message!");

    loop{}
}

// to include nightly features we can use feature flags and use them
