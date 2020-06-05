use core::panic::PanicInfo;
use crate::console::kprintln;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    kprintln!("Kernel Panic (-.-):");
    if let Some(msg) = _info.message() {
        kprintln!("{:?}", msg);
    }
    if let Some(loc) = _info.location() {
        kprintln!("  at {}:{}:{}", loc.file(), loc.line(), loc.column());
    }
    loop {}
}
