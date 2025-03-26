// src/gdt.rs
//
// creates a dedicated stack for handling double faults

use x86_64::VirtAddr; // represents a virtual address in the memory
use x86_64::structures::tss::TastStateSegment;
use lazy_static::lazy_static;

/// indicates which entry in the IST array will be used as a dedicated stack for handling double faults
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static!
{
    /// A TSS is a data structure used by x86_64 CPUs to store information about a task’s state. <br>
    /// One of its key roles is to hold an Interrupt Stack Table (IST), which is an array of stack pointers. <br>
    /// These pointers are used to switch to known-good stacks when handling critical exceptions—like double faults.
    static ref TSS: TastStateSegment {

        let mut tss = TastStateSegment::new();

        // we assign a stack pointer here to the defined index
        // assigns the top of the DOUBLE FAULT STACK to the appropriate IST entry in the TSS
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {

            /// define the STACK SIZE
            const STACK_SIZE: usize = 4096 * 5; // defines a stack of 5 pages

            /// define the STACK
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            // stacks on x86 grow downwards .. i.e. from higher addresses to lower addresses

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start * STACK_SIZE; // initial stack pointer ... top of the stack

            stack_end
        };

        tss
    }
}


use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};

lazy_static!
{
    static ref GDT: GlobalDescriptorTable 
    {
        let mut gdt = GlobalDescriptorTable::new();

        gdt.add_entry(Descriptor::kernle_code_segment());

        gdt.add_entry(Descriptor::tss_segment(&TSS));

        gdt
    }
}
