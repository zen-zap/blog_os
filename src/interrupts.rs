// in src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;


// static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
// the CPU will access this table on every interrupt so it needs to live until we
// load a different IDT  ---- so 'static lifetime ig?
// mut since we need to modify the breakpoint entry in our init() function
// static mut are very prone to data races .. since they are unsafe ...
use lazy_static::lazy_static;

lazy_static!  // this thing does use some unsafe code but that is abstracted for a safe interface
{
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt  // this block returns the IDT
    };
}


pub fn init_idt()
{
    IDT.load();  // lidt - Load Interrupt Descriptor Table

}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n {:#?}", stack_frame);
}


extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    // diverging function x86-interrupt doesn't permit returning from a double_fault
    // error code for the double fault is always 0 -- so no need to print it ...
    // display the exception stack frame
    panic!("EXCEPTION: DOUBLE_FAULT\n=== EXCEPTION_STACK_FRAME === \n{:#?}", stack_frame);
}



#[test_case]  // doing cargo test naturally runs all these tests .. 
fn test_breakpoint_exception()
{
    x86_64::instructions::interrupts::int3();
}
