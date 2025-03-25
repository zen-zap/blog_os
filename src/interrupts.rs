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

#[test_case]  // doing cargo test naturally runs all these tests .. 
fn test_breakpoint_exception()
{
    x86_64::instructions::interrupts::int3();
}
