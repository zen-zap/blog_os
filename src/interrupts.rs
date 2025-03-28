// in src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
// you can check their docs for detailed stuff
use crate::println;
use crate::gdt;


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

        unsafe{
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);  // set the stack for this in the in the IDT           

            // this was placed inside unsafe since the caller must ensure that the used index is
            // valid and not used for another exception
        }

        idt
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

#[allow(unused_unsafe)]
extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    // diverging function x86-interrupt doesn't permit returning from a double_fault
    // error code for the double fault is always 0 -- so no need to print it ...
    // display the exception stack frame
    // panic!("EXCEPTION: DOUBLE_FAULT\n=== EXCEPTION_STACK_FRAME ===\n{:#?}", stack_frame);
    println!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);

    println!("Getting out of this block .. check double_fault_handler in src/interrupts.rs");

    println!("Normally handling a double fault should not allow further execution but just for showing ...\n\n

        Nah this won't work .. the idt.double_fault..set_handler_fn(double_fault_handler) expects a diverging handler! ");

    loop{}
}



#[test_case]  // doing cargo test naturally runs all these tests .. 
fn test_breakpoint_exception()
{
    x86_64::instructions::interrupts::int3();
}


// there is an abstraction for the PIC in this crate
use pic8259::ChainedPics; // a pair of chained PICs .. check source in doc
use spin;

// the PICs are arranged in a Master-Slave Configuration
pub const PIC_1_OFFSET: u8 = 32;                    // handles IRQs 0-7
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;      // handles IRQs 8-15
// normally this overlaps with the CPU exceptions from 0-31 .. hence PICs are remapped starting from 32

pub static PICS: spin::Mutex<ChainedPics> = spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }); // unsafe since we're setting offsets
