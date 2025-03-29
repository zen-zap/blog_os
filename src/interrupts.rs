// in src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
// you can check their docs for detailed stuff
use crate::{println, print};
use crate::gdt;


// static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
// the CPU will access this table on every interrupt so it needs to live until we
// load a different IDT  ---- so 'static lifetime ig?
// mut since we need to modify the breakpoint entry in our init() function
// static mut are very prone to data races .. since they are unsafe ...
use lazy_static::lazy_static;

lazy_static!  // this thing does use some unsafe code but that is abstracted for a safe interface
{
    /// The InterruptDescriptorTable struct implements the IndexMut trait, so we can access individual entries through array indexing syntax.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        unsafe{
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);  // set the stack for this in the in the IDT           

            // this was placed inside unsafe since the caller must ensure that the used index is
            // valid and not used for another exception
        }

        // setup the timer interrupt handler for the timer to work .. you know clock cycles and
        // stuff like that 
        // CPU reacts identically to exceptions and external interrupts (the only difference is that some exceptions push an error code)
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);

        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

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
                                                                                                                       //
                                                                                                                       //
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // defaults to the pervious value + 1 = 33 .. so interrupt 33
}

impl InterruptIndex
{
    fn as_u8(self) -> u8
    {
        self as u8
    }

    fn as_usize(self) -> usize 
    {
        usize::from(self.as_u8())
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame)
{
    // print!("Inside the timer_interrupt_handler!");
    // print!(" .itr. ");

    // You also gotta setup an end of interrupt function .. since the PIC expects an explicit EOI
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame)
{
    // print!("some key was presses just now -- key press indicator -- reading the scancode now --
    // reacts to key press and release separately .... so 2 scancodes are printed per single key
    // press");

    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe {  // represents key press/release
        port.read()
    };

    print!(" key pressed scancode ==> {}\n", scancode);

    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()); // motify the end of
                                                                               // this interrupt
    }
}
