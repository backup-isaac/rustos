#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use core::time::Duration;
use kernel_api::println;
use kernel_api::syscall::{exit, getpid, time};

fn fib(n: u64, deadline: Duration) -> u64 {
    if time() > deadline {
        println!("Fib process {} timed out", getpid());
        exit();
    }
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1, deadline) + fib(n - 2, deadline),
    }
}

fn main() {
    println!("Started...");
    let deadline = time() + Duration::from_secs(10 + getpid());
    let rtn = fib(30, deadline);

    println!("Ended: Result = {}", rtn);
}
