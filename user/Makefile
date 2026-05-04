# Makefile for building Pintos user test programs for RustOS
#
# Usage:
#   make minimal    - Build minimal test (raw syscalls, no library)
#   make args       - Build args test (uses Pintos libc + test framework)
#   make halt       - Build halt test
#   make all        - Build all tests
#   make embed      - Build all and generate embedded_tests.rs
#   make clean      - Remove build artifacts

CC      := /opt/local/bin/i386-elf-gcc
LD      := /opt/local/bin/i386-elf-ld
OBJCOPY := /opt/local/bin/i386-elf-objcopy
OBJDUMP := /opt/local/bin/i386-elf-objdump

PINTOS  := /Users/dean/26Sp-Team-27/project/src
USER_DIR := /Users/dean/rustos/user
BUILD   := $(USER_DIR)/build

# Include paths: lib/user first so #include_next works
INCLUDES := -I$(PINTOS)/lib/user -I$(PINTOS)/lib -I$(PINTOS)

# Compiler flags matching Pintos
CFLAGS := -nostdlib -nostdinc -static -m32 -Os -Wall -Wno-format \
          -ffreestanding -fno-builtin -fno-stack-protector $(INCLUDES)

LDSCRIPT := $(PINTOS)/lib/user/user.lds
LDFLAGS  := -T $(LDSCRIPT) --entry=_start -m elf_i386

# Pintos user library objects (needed for all tests except minimal)
LIB_SRCS := $(PINTOS)/lib/user/entry.c \
            $(PINTOS)/lib/user/syscall.c \
            $(PINTOS)/lib/user/console.c \
            $(PINTOS)/lib/user/debug.c \
            $(PINTOS)/lib/debug.c \
            $(PINTOS)/lib/string.c \
            $(PINTOS)/lib/stdio.c

LIB_OBJS := $(patsubst $(PINTOS)/%.c,$(BUILD)/%.o,$(LIB_SRCS))

# Test framework
TEST_FW_SRC := $(PINTOS)/tests/lib.c
TEST_FW_OBJ := $(BUILD)/tests/lib.o

.PHONY: all minimal args halt embed clean

all: minimal args halt

# === Minimal test (raw syscalls) ===
minimal: $(BUILD)/test_minimal.elf
	@echo "Built: $<"
	@file $<

$(BUILD)/test_minimal.elf: $(BUILD)/test_minimal.o
	@echo "  LD    $@"
	$(LD) $(LDFLAGS) -o $@ $^

$(BUILD)/test_minimal.o: $(USER_DIR)/test_minimal.c | $(BUILD)
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

# === Args test ===
args: $(BUILD)/args.elf
	@echo "Built: $<"
	@file $<

$(BUILD)/args.elf: $(LIB_OBJS) $(TEST_FW_OBJ) $(BUILD)/tests/userprog/args.o
	@echo "  LD    $@"
	$(LD) $(LDFLAGS) -o $@ $^

$(BUILD)/tests/userprog/args.o: $(PINTOS)/tests/userprog/args.c | $(BUILD)/tests/userprog
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

# === Halt test ===
halt: $(BUILD)/halt.elf
	@echo "Built: $<"
	@file $<

$(BUILD)/halt.c:
	@mkdir -p $(dir $@)
	@echo '#include <syscall.h>' > $@
	@echo 'int main(void) { halt(); return 0; }' >> $@

$(BUILD)/halt.elf: $(LIB_OBJS) $(BUILD)/halt.o
	@echo "  LD    $@"
	$(LD) $(LDFLAGS) -o $@ $^

$(BUILD)/halt.o: $(BUILD)/halt.c | $(BUILD)
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

# === Generic rules for library objects ===
$(BUILD)/lib/user/%.o: $(PINTOS)/lib/user/%.c | $(BUILD)/lib/user
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD)/lib/%.o: $(PINTOS)/lib/%.c | $(BUILD)/lib
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD)/tests/lib.o: $(PINTOS)/tests/lib.c | $(BUILD)/tests
	@echo "  CC    $<"
	$(CC) $(CFLAGS) -c -o $@ $<

# === Directories ===
$(BUILD):
	mkdir -p $@

$(BUILD)/lib:
	mkdir -p $@

$(BUILD)/lib/user:
	mkdir -p $@

$(BUILD)/tests:
	mkdir -p $@

$(BUILD)/tests/userprog:
	mkdir -p $@

# === Generate embedded Rust byte array ===
embed: all
	@echo "Generating embedded_tests.rs..."
	@python3 $(USER_DIR)/gen_embed.py

# === Clean ===
clean:
	rm -rf $(BUILD)
	rm -f $(USER_DIR)/embedded_tests.rs
