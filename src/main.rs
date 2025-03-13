#![no_std]
#![no_main]

// we need a panic handler .. the std implements its own .. we need our own .. since we're not
// gonna have the std
//
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // read about this a bit
pub extern "C" fn _start() -> ! {
    // entry point should not return instead do the exit system call
    loop {}
}
