use alloc::boxed::Box;
use core::time::Duration;

use crate::console::{CONSOLE, kprintln};
use crate::process::{Process, State};
use crate::traps::TrapFrame;
use crate::SCHEDULER;
use kernel_api::*;
use pi::timer::Timer;

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let timer = Timer::new();
    let start_time = timer.read();
    let end_time = start_time + Duration::from_millis(ms as u64);
    let has_waited_long_enough = Box::new(move |p: &mut Process| {
        if timer.read() >= end_time {
            let elapsed_time = (timer.read() - start_time).as_millis() as u64;
            p.context.x_registers[0] = elapsed_time;
            p.context.x_registers[7] = 1;
            true
        } else {
            false
        }
    });
    SCHEDULER.switch(State::Waiting(has_waited_long_enough), tf);
}

/// Returns current time.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns two
/// parameter:
///  - current time as seconds
///  - fractional part of the current time, in nanoseconds.
pub fn sys_time(tf: &mut TrapFrame) {
    let timer = Timer::new();
    let now = timer.read();
    tf.x_registers[0] = now.as_secs();
    tf.x_registers[1] = now.subsec_nanos() as u64;
    tf.x_registers[7] = 1;
}

/// Kills current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    SCHEDULER.switch(State::Dead, tf);
}

/// Write to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    CONSOLE.lock().write_byte(b);
    tf.x_registers[7] = 1;
}

/// Returns current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.x_registers[0] = tf.tpidr;
    tf.x_registers[7] = 1;
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    match num as usize {
        NR_EXIT => sys_exit(tf),
        NR_GETPID => sys_getpid(tf),
        NR_SLEEP => sys_sleep(tf.x_registers[0] as u32, tf),
        NR_TIME => sys_time(tf),
        NR_WRITE => sys_write(tf.x_registers[0] as u8, tf),
        other => kprintln!("unrecognized syscall {}", other),
    }
}
