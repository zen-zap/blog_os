use core::arch::asm;

pub unsafe fn enter_user_mode(
	code_selector: u16,
	data_selector: u16,
	entry_point: u64,
	stack_pointer: u64,
) -> ! {
	asm!(
		// load the user data segment into the data registers
		"mov ds, cx",
		"mov es, cx",
		"mov fs, cx", // what are these?
		"mov gs, cx",

		// building the iretq stack frame (pushed in reverse order)
		"push rcx", // SS (user data segment)
		"push rdx", // RSP (user stack pointer)

		// pushing RFLAGS, make sure interrupts are enabled (set bit 9)
		"pushf",
		"pop rax",
		"or rax, 0x200",
		"push rax", // RFLAGS
		"push rdi", // CS (user code segment)
		"push rsi", // RIP (user entry point)

		"iretq",

		in("rdi") code_selector,
		in("rsi") entry_point,
		in("rdx") stack_pointer,
		in("cx") data_selector,
		options(noreturn)
	);
}
