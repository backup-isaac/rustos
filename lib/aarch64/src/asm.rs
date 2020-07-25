/// Wait for event not to burn CPU.
#[inline(always)]
pub fn wfe() {
    unsafe { llvm_asm!("wfe" :::: "volatile") };
}

/// Wait for interrupt not to burn CPU.
#[inline(always)]
pub fn wfi() {
    unsafe { llvm_asm!("wfi" :::: "volatile") };
}


/// A NOOP that won't be optimized out.
#[inline(always)]
pub fn nop() {
    unsafe { llvm_asm!("nop" :::: "volatile") };
}

/// Transition to a lower level
#[inline(always)]
pub fn eret() {
    unsafe { llvm_asm!("eret" :::: "volatile") };
}

/// Instruction Synchronization Barrier
#[inline(always)]
pub fn isb() {
    unsafe { llvm_asm!("isb" :::: "volatile") };
}

/// Set Event
#[inline(always)]
pub fn sev() {
    unsafe { llvm_asm!("sev" ::::"volatile") };
}

/// Enable (unmask) interrupts
#[inline(always)]
pub unsafe fn sti() {
    llvm_asm!("msr DAIFClr, 0b0010"
         :
         :
         :
         : "volatile");
}

/// Disable (mask) interrupt
#[inline(always)]
pub unsafe fn cli() {
    llvm_asm!("msr DAIFSet, 0b0010"
         :
         :
         :
         : "volatile");
}

/// Break with an immeidate
#[macro_export]
macro_rules! brk {
    ($num:tt) => {
        unsafe { llvm_asm!(concat!("brk ", stringify!($num)) :::: "volatile"); }
    }
}

/// Supervisor call with an immediate
#[macro_export]
macro_rules! svc {
    ($num:tt) => {
        unsafe { llvm_asm!(concat!("svc ", stringify!($num)) :::: "volatile"); }
    }
}
