#![allow(dead_code, unused, unreachable_code)]
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
use x86_64::structures::paging::{PageTable, Translate};
use blog_os::memory::{self, translate_addr};
use x86_64::VirtAddr;

entry_point!(kernel_main); // defines the real low-level _start for us --- this thing is
                           // type-checked so you can't really modify the signature on a whim

fn kernel_main(boot_info: &'static BootInfo) -> ! {

    println!("hello world{}", "!~");

    blog_os::init(); // for the exception things
                     
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    //let l4_table = unsafe {
    //    active_level_4_table(phys_mem_offset)
    //    // takes the offset and returns the virtual address
    //};
    //
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // let mut frame_allocator = memory::EmptyFrameAllocator;
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    //for (i, entry) in l4_table.iter().enumerate() {
    //    if !entry.is_unused() {
    //        println!("L4 Entry {}: {:?}", i, entry);
    //
    //        // get the physical address from the entry and convert it
    //        let phys = entry.frame().unwrap().start_address();
    //        let virt = phys.as_u64() + boot_info.physical_memory_offset;
    //        let ptr = VirtAddr::new(virt).as_mut_ptr();
    //        let l3_table: &PageTable = unsafe { &*ptr };
    //
    //        // print the non-empty entries of the level 3 table
    //        for (i, entry) in l3_table.iter().enumerate() {
    //            if !entry.is_unused() {
    //                println!("L3 Entry {}:{:?}", i, entry);
    //            }
    //        }
    //    }
    //}

    let addresses = [
        // the identity-mapped VGA buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {

        let virt = VirtAddr::new(address);

        let phys = mapper.translate_addr(virt);

        //let phys = unsafe {
        //    translate_addr(virt, phys_mem_offset)
        //};

        println!("{:?} -> {:?}", virt, phys);
    }

    // map an unused page -- mapping created at address 0
    let page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // write the string `New!` to the screen through the mapping
    let page_ptr; *mut u64 = page.start_address().as_mut_ptr();
    unsafe {
        page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)
    };

    #[cfg(test)]
    test_main();

    println!("It did not crash! --- YOU CAN WRITE SOMETHING HERE");
    blog_os::hlt_loop();
}


///// Entry point of the code
//#[no_mangle]
//pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
//
//    println!("Hello World{}", "!");
//
//
//    blog_os::init(); // for exception things
//
//    // invoke a breakpoint exception
//    // x86_64::instructions::interrupts::int3(); // this is a breakpoint exception .. int3 is the asm
//
//    // triggerring a page fault -- to demonstrate a double fault
//    //unsafe {
//    //    *(0xdeadbeef as *mut u8) = 42;
//    //};
//    //
//    // println!("Handled the breakpoint_exception! .. caused by int3 instruction");
//
//    #[allow(unconditional_recursion)]
//    fn stack_overflow()
//    {
//        stack_overflow();
//    }
//
//    // we just used the address that the page fault handler returned
//    let ptr = 0x2047b9 as *mut u8;
//
//    // read from a code page
//    unsafe {
//        let _x = *ptr;
//    }
//    println!("read worked from address: {:?}", ptr);
//
//    // write to a code page
//    //unsafe {
//    //    *ptr = 42; // try to store 42 at that address?
//    //}
//    //println!("write worked");
//
//    use x86_64::registers::control::Cr3;
//
//    // as we all know that CR3 holds the base level 4 page table -- btw all of the levels have
//    // names .. check them out
//    let (level_4_page_table, _) = Cr3::read();
//    println!("level 4 page table: {:?}", level_4_page_table.start_address());
//
//    // trigger a stack_overflow
//    // stack_overflow();
//
//    // println!("Handled the double_fault!");
//
//    #[cfg(test)]
//    test_main();
//
//    println!("It did not crash!");
//
//    blog_os::hlt_loop(); 
//}


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
