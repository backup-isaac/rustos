# rustos

64-bit (AArch64) OS for Raspberry Pi written in Rust. Extension of the eponymous project from Georgia Tech's CS 3210 during Spring 2020. See https://github.com/sslab-gatech/cs3210-rustos-public for the original project

## Dependencies
Requires nightly compiler. So far it's only been used with `nightly-2019-07-01`

## Building and running
The repository originally provided by CS 3210 hardcoded a bunch of things for building with Ubuntu and also had binaries such as `aarch64-objdump` and `qemu-system-aarch64` checked into it. So a sane build process that isn't tied to linux and doesn't fill the repo with about 100 MB of binaries is a work in progress

## What's in it
- UART bootloader for kernel
- Read only FAT32 file system
  - this means the bootloader isn't persistent yet
- Round robin process scheduler
- Can load userspace processes from a file
- Virtual memory for processes and kernel
- Terminal interface over UART
- A few system calls
