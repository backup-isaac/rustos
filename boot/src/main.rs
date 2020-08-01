#![feature(asm)]
#![feature(llvm_asm)]
#![feature(global_asm)]

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

use xmodem::Xmodem;
use core::time::Duration;
use core::slice;
use pi::uart::MiniUart;
use pi::gpio;
use pi::timer;
use shim::io;

/// Start address of the binary to load and of the bootloader.
const BINARY_START_ADDR: usize = 0x80000;
const BOOTLOADER_START_ADDR: usize = 0x4000000;

/// Pointer to where the loaded binary expects to be laoded.
const BINARY_START: *mut u8 = BINARY_START_ADDR as *mut u8;

/// Free space between the bootloader and the loaded binary's start address.
const MAX_BINARY_SIZE: usize = BOOTLOADER_START_ADDR - BINARY_START_ADDR;

/// Branches to the address `addr` unconditionally.
unsafe fn jump_to(addr: *mut u8) -> ! {
    llvm_asm!("br $0" : : "r"(addr as usize));
    loop {
        llvm_asm!("wfe" :::: "volatile")
    }
}

fn kmain() -> ! {
    let mut uart = MiniUart::new();
    uart.set_read_timeout(Duration::from_millis(750));
    // blink an LED when the bootloader errors for debugging purposes
    let mut led = gpio::Gpio::new(5).into_output();
    loop {
        unsafe {
            let new_kernel = slice::from_raw_parts_mut(BINARY_START, MAX_BINARY_SIZE);
            match Xmodem::receive(&mut uart, new_kernel) {
                Ok(_) => jump_to(BINARY_START),
                Err(e) => {
                    if e.kind() != io::ErrorKind::TimedOut {
                        led.set();
                        timer::spin_sleep(Duration::from_millis(75));
                        led.clear();
                        timer::spin_sleep(Duration::from_millis(75));
                    }
                }
            }
        }
    }
}
