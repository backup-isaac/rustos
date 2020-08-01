ROOT := $(shell git rev-parse --show-toplevel)
HOST := $(shell rustc -Vv | grep host | cut -d ' ' -f2)
KERN := kernel
TARGET := target/aarch64-unknown-none/release/${KERN}
SDCARD ?= $(ROOT)/ext/fat32-imgs/mock1.fat32.img
BIN := build/$(KERN).bin
# SDCARD ?= $(ROOT)/user/fs.img
TTY_PATH := /dev/ttyUSB0
QEMU_ARGS ?=
QEMU_FLAGS := 										\
	-nographic											\
	-M raspi3												\
	-serial null -serial mon:stdio	\
	-kernel	$(BIN)									\
	-drive 													\
	file=$(SDCARD),format=raw,if=sd \

.PHONY: all build qemu transmit objdump nm check clean install test

all: build

build:
	@echo "+ Building build/$(KERN).elf [xbuild/$@]"
	@cargo xbuild --release
	@mkdir -p build
	@cp -f $(TARGET) build/$(KERN).elf

	@echo "+ Building build/$(KERN).bin [objcopy]"
	@cargo objcopy --bin $(KERN) -- --strip-all $(BIN)

check:
	@cargo xcheck

qemu: build
	@qemu-system-aarch64 $(QEMU_FLAGS) $(QEMU_ARGS)

qemu-gdb: build
	@qemu-system-aarch64 $(QEMU_FLAGS) -s -S

qemu-asm: build
	@qemu-system-aarch64 $(QEMU_FLAGS) -d in_asm

transmit: build
	@echo "+ Transmitting build/$(KERN).bin to $(TTY_PATH)"
	ttywrite -i build/$(KERN).bin $(TTY_PATH)
	screen $(TTY_PATH) 115200

objdump: build
	cargo objdump --bin $(KERN) -- -disassemble -no-show-raw-insn -print-imm-hex

nm: build
	cargo nm --bin $(KERN)

clean:
	cargo clean
	rm -rf build

# install: build
# 	@echo "+ Installing build/$(KERN).elf [install-kernel.py]"
# 	@$(ROOT)/bin/install-kernel.py build/$(KERN).elf

test:
	cargo test --target=$(HOST) -- --test-threads=1 --nocapture
