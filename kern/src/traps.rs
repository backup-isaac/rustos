mod frame;
mod syndrome;
mod syscall;

pub mod irq;
pub use self::frame::TrapFrame;

use pi::interrupt::{Controller, Interrupt};

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Kind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Source {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Info {
    source: Source,
    kind: Kind,
}

/// This function is called when an exception occurs. The `info` parameter
/// specifies the source and kind of exception that has occurred. The `esr` is
/// the value of the exception syndrome register. Finally, `tf` is a pointer to
/// the trap frame for the exception.
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame) {
    // crate::console::kprintln!("{:?}, esr {}, {:?}", info, esr, tf);
    // crate::console::kprintln!("esr {}, {:?}", esr, tf);
    // crate::console::kprintln!("{}", unsafe { aarch64::current_el() });
    if info.kind == Kind::Synchronous {
        match Syndrome::from(esr) {
            Syndrome::Brk(_) => {
                crate::shell::shell("brk_handler$ ");
                tf.elr += 4;
            }
            Syndrome::Svc(x) => handle_syscall(x, tf),
            other => {
                crate::console::kprintln!("unhandled exception with syndrome {:?}", other);
                loop {}
            }
        }
    } else if info.kind == Kind::Irq {
        let controller = Controller::new();
        for i in Interrupt::iter() {
            if controller.is_pending(*i) {
                crate::IRQ.invoke(*i, tf);
            }
        }
    }
}
