#![no_std]
#![no_main] // disabling all rust level entry points 
// #![feature(asm)] // for inline assembly -- has been a feature since a while in the nightly versions

// we need a panic handler .. the std implements its own .. we need our own .. since we're not
// gonna have the std
//
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

static HELLO: &[u8] = b"Hello World";

#[no_mangle] // read about this a bit
pub extern "C" fn _start() -> ! {
    // entry point should not return instead do the exit system call
    // since the linker looks for a start point 
    //
    let vga_buffer = 0xb8000 as *mut u8;  // the starting position of the VGA buffer is fixed!
                                          
    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe { // raw pointer operations can lead to memory corruptions is used incorrectly!
                 // ----- so use this within the unsafe block!

            *vga_buffer.offset(i as isize * 2) = byte;  // writes the character byte into the buffer
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb; // write the color byte right after the character byte
            
            // the multiplication by 2 is needed cuz each character cell is 2 bytes 

        }
    }

    loop{}
}

// to include nightly features we can use feature flags and use them
