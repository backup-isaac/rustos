ROOT := $(shell git rev-parse --show-toplevel)
HOST := $(shell rustc -Vv | grep host | cut -d ' ' -f2)

KERN := boot
ELF := build/$(KERN).elf
TARGET := target/aarch64-unknown-none/release/${KERN}
OBJCPY := cargo objcopy -- --strip-all -O binary
QEMU_FLAGS := 										\
	-nographic											\
	-M raspi3												\
	-serial null -serial mon:stdio	\
	-kernel $(ELF)

.PHONY: all build qemu objdump nm check clean install test

all: build

build:
	@echo "+ Building $(ELF) [xbuild/$@]"
	@cargo xbuild --release
	@mkdir -p build
	@cp -f $(TARGET) $(ELF)

	@echo "+ Building build/$(KERN).bin [objcopy]"
	@cargo objcopy --bin $(KERN) -- --strip-all build/$(KERN).bin

check:
	@cargo xcheck

qemu: build
	@qemu-system-aarch64 $(QEMU_FLAGS) $(QEMU_ARGS)

objdump: build
	cargo objdump --bin $(KERN) -- -disassemble -no-show-raw-insn -print-imm-hex

nm: build
	cargo nm --bin $(KERN)

clean:
	cargo clean
	rm -rf build

install: build
	@echo "+ Installing build/$(KERN).elf [install-kernel]"
	@$(ROOT)/bin/install-kernel build/$(KERN).elf

test:
	cargo test --target=$(HOST)
