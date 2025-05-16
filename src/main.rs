#![allow(dead_code, unused, unreachable_code)]
#![no_std]
#![no_main]
// disabling all rust level entry points
// set the test_framework_entry_function to "test_main" and call it from the _start entry point
#![reexport_test_harness_main = "test_main"]
// reexport the generated test-harness main function as "test_main"
// To implement a custom test framework!
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)] // moved to lib.rs

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use blog_os::interrupts::InterruptIndex::Keyboard;
use blog_os::memory::{self, BootInfoFrameAllocator, translate_addr};
use blog_os::task::{Task, executor::Executor, keyboard, simple_executor::SimpleExecutor};
use blog_os::{allocator, println};
use bootloader::{BootInfo, entry_point};
use core::arch::asm;
use core::panic::PanicInfo;
use x86_64::VirtAddr;
use x86_64::registers::control::Cr2;
use x86_64::structures::paging::{Page, PageTable, Translate};

extern crate alloc;

entry_point!(kernel_main); // defines the real low-level _start for us --- this thing is
// type-checked so you can't really modify the signature on a whim

fn kernel_main(boot_info: &'static BootInfo) -> ! {
	println!("Hello zen-zap{}", "!");
	blog_os::init(); // for the exception things

	let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

	//let l4_table = unsafe {
	//    active_level_4_table(phys_mem_offset)
	//    // takes the offset and returns the virtual address
	//};
	//
	let mut mapper = unsafe { memory::init(phys_mem_offset) };
	// let mut frame_allocator = memory::EmptyFrameAllocator;
	let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

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

	//let addresses = [
	//	// the identity-mapped VGA buffer page
	//	0xb8000,
	//	// some code page
	//	0x201008,
	//	// some stack page
	//	0x0100_0020_1a10,
	//	// virtual address mapped to physical address 0
	//	boot_info.physical_memory_offset,
	//];
	//
	//for &address in &addresses {
	//	let virt = VirtAddr::new(address);
	//
	//	let phys = mapper.translate_addr(virt);
	//
	//	//let phys = unsafe {
	//	//    translate_addr(virt, phys_mem_offset)
	//	//};
	//
	//	println!("{:?} -> {:?}", virt, phys);
	//}
	//
	//// map an unused page -- mapping created at address 0
	//let page = Page::containing_address(VirtAddr::new(0));
	//memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);
	//
	//// write the string `New!` to the screen through the mapping
	//let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
	//unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };

	allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed!");

	// let heap_value = Box::new(41);
	// println!("heap value at {:p}", heap_value);
	//
	// let mut vec = Vec::new(); // dynamic size
	// for i in 0..500 {
	// 	vec.push(i);
	// }
	//
	// println!("vec at {:p}", vec.as_slice());
	//
	// // reference counted vector
	// let reference_counted = Rc::new(vec![1, 2, 3]);
	// let cloned_reference = reference_counted.clone();
	// println!("current reference count is {}", Rc::strong_count(&cloned_reference));
	// core::mem::drop(reference_counted);
	// println!("reference count is {} now", Rc::strong_count(&cloned_reference));

	let mut executor = Executor::new();

	executor.spawn(Task::new(example_task()));
	executor.spawn(Task::new(keyboard::print_keypresses()));
	executor.run();

	#[cfg(test)]
	test_main();

	println!("It did not crash!");
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
//    // triggering a page fault -- to demonstrate a double fault
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
//    // THIS IS STUFF TO DEMONSTRATE PAGE FAULTS
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
	println!("KERNEL PANIC: {}\n", info);

	// reading RIP [current instruction pointer]
	let rip: u64;
	unsafe {
		asm!(
			"lea {rip}, [rip]", // load the effective address of the next instruction
			rip = out(reg) rip,
			options(nomem, nostack, preserves_flags),
		);
	}

	println!("RIP: {:#018x}", rip);

	// stack backtrace
	println!("\nStack Backtrace:");
	let mut rbp: u64;
	unsafe {
		asm!(
			"mov {rbp}, rbp",
			rbp = out(reg) rbp,
			options(nomem, preserves_flags),
		)
	}

	let mut stack_trace_count = 0;

	while rbp != 0 && stack_trace_count < 20 {
		// return address is saved at [RBP + 8]
		let ret = unsafe { *((rbp + 8) as *const u64) };
		println!("  {:#018x}", ret);
		// the previous frame's RBP is at [RBP]
		rbp = unsafe { *(rbp as *const u64) };

		stack_trace_count += 1;
	}

	// let rsp: u64;
	// unsafe {
	// 	asm!(
	// 		"mov {rsp}, rsp",
	// 		rsp = out(reg) rsp,
	// 		options(nomem, preserves_flags),
	// 	);
	// }
	// println!("RSP: {:#x}", rsp);

	// halt it forever,
	blog_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	blog_os::test_panic_handler(info)
}

#[test_case]
fn one_one_assertion() {
	assert_eq!(1, 1);
}

/// Returns 69
async fn async_number() -> u32 {
	69
}

/// Waits on async_number() as prints the result
async fn example_task() {
	let number = async_number().await;
	println!("async number: {}", number);
}

// Now, to experience so better results than this and actually see the advantage of having a Waker,
// let's see something new
