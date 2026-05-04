#!/bin/bash
# Build script for Pintos user test programs for RustOS
#
# Usage: ./build.sh [test_name ...]
#   test_name: "minimal", "args", "halt", "all" (default: "all")
#
# After building, run: python3 gen_embed.py
# to generate the Rust embedded byte arrays.

set -e

# Configuration
CC=/opt/local/bin/i386-elf-gcc
LD=/opt/local/bin/i386-elf-ld
OBJCOPY=/opt/local/bin/i386-elf-objcopy
OBJDUMP=/opt/local/bin/i386-elf-objdump

PINTOS_SRC=/Users/dean/26Sp-Team-27/project/src
USER_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="${USER_DIR}/build"

# Include paths: lib/user first so #include_next works (user/stdio.h -> lib/stdio.h)
INCLUDES="-I${PINTOS_SRC}/lib/user -I${PINTOS_SRC}/lib -I${PINTOS_SRC}"

# Compiler flags matching Pintos
CFLAGS="-nostdlib -nostdinc -static -m32 -Os -Wall -Wno-format \
  -ffreestanding -fno-builtin -fno-stack-protector \
  ${INCLUDES}"

LDSCRIPT="${PINTOS_SRC}/lib/user/user.lds"
LDFLAGS="-T ${LDSCRIPT} --entry=_start -m elf_i386"

# Create build directory
mkdir -p "${BUILD_DIR}"

# Compile a source file, placing the object under BUILD_DIR mirroring the source path.
# Usage: compile <src_file>
# Prints the object file path.
compile() {
    local src="$1"
    local relpath

    # Determine relative object path
    if [[ "$src" == "${PINTOS_SRC}"/* ]]; then
        relpath="${src#${PINTOS_SRC}/}"
    elif [[ "$src" == "${USER_DIR}"/* ]]; then
        relpath="${src#${USER_DIR}/}"
    else
        relpath="$(basename "$src")"
    fi

    local obj="${BUILD_DIR}/${relpath%.c}.o"
    mkdir -p "$(dirname "$obj")"

    echo "  CC  ${relpath}" >&2
    ${CC} ${CFLAGS} -c -o "${obj}" "${src}"
    echo "${obj}"
}

# Build the Pintos user library objects (shared by args, halt, etc.)
build_userlib() {
    USERLIB_OBJS=""
    for src in \
        "${PINTOS_SRC}/lib/user/entry.c" \
        "${PINTOS_SRC}/lib/user/syscall.c" \
        "${PINTOS_SRC}/lib/user/console.c" \
        "${PINTOS_SRC}/lib/user/debug.c" \
        "${PINTOS_SRC}/lib/debug.c" \
        "${PINTOS_SRC}/lib/string.c" \
        "${PINTOS_SRC}/lib/stdio.c" \
    ; do
        OBJ=$(compile "$src")
        USERLIB_OBJS="${USERLIB_OBJS} ${OBJ}"
    done
}

# Build test framework object
build_testlib() {
    TESTLIB_OBJ=$(compile "${PINTOS_SRC}/tests/lib.c")
}

show_info() {
    local elf="$1"
    echo ""
    echo "--- ELF: $(basename "$elf") ---"
    file "$elf"
    ${OBJDUMP} -f "$elf" 2>/dev/null || true
    ls -la "$elf"
}

# ---- Build targets ----

build_minimal() {
    echo "=== Building: minimal (raw syscalls) ==="
    OBJ=$(compile "${USER_DIR}/test_minimal.c")
    echo "  LD  test_minimal.elf" >&2
    ${LD} ${LDFLAGS} -o "${BUILD_DIR}/test_minimal.elf" "${OBJ}"
    show_info "${BUILD_DIR}/test_minimal.elf"
}

build_args() {
    echo "=== Building: args (Pintos libc + test framework) ==="
    build_userlib
    build_testlib
    OBJ=$(compile "${PINTOS_SRC}/tests/userprog/args.c")
    echo "  LD  args.elf" >&2
    ${LD} ${LDFLAGS} -o "${BUILD_DIR}/args.elf" ${USERLIB_OBJS} ${TESTLIB_OBJ} "${OBJ}"
    show_info "${BUILD_DIR}/args.elf"
}

build_halt() {
    echo "=== Building: halt ==="
    build_userlib

    # Generate halt.c if needed
    cat > "${BUILD_DIR}/halt.c" << 'EOF'
#include <syscall.h>
int main(void) {
    halt();
    return 0;
}
EOF
    OBJ=$(compile "${BUILD_DIR}/halt.c")
    echo "  LD  halt.elf" >&2
    ${LD} ${LDFLAGS} -o "${BUILD_DIR}/halt.elf" ${USERLIB_OBJS} "${OBJ}"
    show_info "${BUILD_DIR}/halt.elf"
}

# ---- Main ----

TARGETS="${@:-all}"

for target in $TARGETS; do
    case "$target" in
        minimal)  build_minimal ;;
        args)     build_args ;;
        halt)     build_halt ;;
        all)
            build_minimal
            echo ""
            build_args
            echo ""
            build_halt
            ;;
        *)
            echo "Unknown target: $target"
            echo "Usage: $0 [minimal|args|halt|all]"
            exit 1
            ;;
    esac
done

echo ""
echo "=== Build complete ==="
echo "ELFs in: ${BUILD_DIR}/"
ls -la "${BUILD_DIR}"/*.elf 2>/dev/null || echo "(no ELF files)"
echo ""
echo "To generate embedded Rust byte array:"
echo "  python3 ${USER_DIR}/gen_embed.py"
