/* Minimal user program - uses raw int $0x30 syscalls, no library needed.
 * Syscall convention: push args right-to-left, then syscall number, then int $0x30.
 * Return value in %eax.
 *
 * SYS_HALT  = 0
 * SYS_EXIT  = 1
 * SYS_WRITE = 9
 */

static int
sys_write(int fd, const void *buffer, unsigned size)
{
    int retval;
    asm volatile(
        "pushl %[arg2]; pushl %[arg1]; pushl %[arg0]; "
        "pushl %[number]; int $0x30; addl $16, %%esp"
        : "=a"(retval)
        : [number] "i"(9), [arg0] "r"(fd), [arg1] "r"(buffer), [arg2] "r"(size)
        : "memory");
    return retval;
}

static void __attribute__((noreturn))
sys_exit(int status)
{
    asm volatile(
        "pushl %[arg0]; pushl %[number]; int $0x30; addl $8, %%esp"
        : /* no outputs */
        : [number] "i"(1), [arg0] "g"(status)
        : "memory");
    __builtin_unreachable();
}

/* Simple strlen */
static unsigned
my_strlen(const char *s)
{
    unsigned n = 0;
    while (s[n])
        n++;
    return n;
}

void
_start(void)
{
    const char msg[] = "Hello from user mode!\n";
    sys_write(1, msg, my_strlen(msg));
    sys_exit(0);
}
