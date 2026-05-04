#!/bin/bash
# Build the Pintos args test and generate an embedded Rust byte array.
set -ex

CC=/opt/local/bin/i386-elf-gcc
LD=/opt/local/bin/i386-elf-ld
P=/Users/dean/26Sp-Team-27/project/src
RUSTOS=/Users/dean/rustos
B=$RUSTOS/user/build
I="-I$P/lib/user -I$P/lib -I$P"
F="-nostdlib -nostdinc -static -m32 -Os -Wall -Wno-format -ffreestanding -fno-builtin -fno-stack-protector $I"

mkdir -p $B

# Compile Pintos user library
$CC $F -c -o $B/entry.o       $P/lib/user/entry.c
$CC $F -c -o $B/syscall.o     $P/lib/user/syscall.c
$CC $F -c -o $B/console.o     $P/lib/user/console.c
$CC $F -c -o $B/debug.o       $P/lib/user/debug.c
$CC $F -c -o $B/debug_common.o $P/lib/debug.c
$CC $F -c -o $B/string.o      $P/lib/string.c
$CC $F -c -o $B/stdio.o       $P/lib/stdio.c
$CC $F -c -o $B/testlib.o     $P/tests/lib.c
$CC $F -c -o $B/stubs.o       $RUSTOS/user/stubs.c

# Compile args test
$CC $F -c -o $B/args.o        $P/tests/userprog/args.c

# Link into ELF binary
$LD -T $P/lib/user/user.lds --entry=_start -m elf_i386 -o $RUSTOS/user/args.elf \
  $B/entry.o $B/syscall.o $B/console.o $B/debug.o $B/debug_common.o \
  $B/string.o $B/stdio.o $B/testlib.o $B/stubs.o $B/args.o

file $RUSTOS/user/args.elf

# Generate embedded Rust byte array
python3 -c "
data = open('$RUSTOS/user/args.elf', 'rb').read()
print('#[allow(dead_code)]')
print('pub static ARGS_ELF: &[u8] = &[')
for i in range(0, len(data), 12):
    chunk = data[i:i+12]
    line = ', '.join(f'0x{b:02x}' for b in chunk)
    print(f'    {line},')
print('];')
" > $RUSTOS/src/userprog/embedded_args.rs

echo "Generated: $RUSTOS/src/userprog/embedded_args.rs"
echo "Success!"
