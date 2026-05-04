// Interrupt stub linkage.
//
// Includes the assembly stubs via `global_asm!` and declares the
// `intr_stubs` symbol (an array of 256 function pointers) that the
// IDT setup code uses to fill in handler offsets.

core::arch::global_asm!(include_str!("../../asm/intr_stubs.S"), options(att_syntax));

extern "C" {
    /// Array of 256 interrupt stub entry points, defined in intr_stubs.S.
    /// Each entry is a function pointer to intr<NN>_stub.
    pub static intr_stubs: [unsafe extern "C" fn(); 256];
}
