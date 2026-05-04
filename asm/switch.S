### switch.S - Context switch assembly for RustOS threading.
### Adapted from Pintos threads/switch.S.

.section .text
.globl switch_threads
.type switch_threads, @function

# switch_threads(cur: *mut Thread, next: *mut Thread) -> *mut Thread
#
# Saves the caller's register state on its stack, switches to `next`'s
# saved stack pointer, and restores `next`'s registers.  Returns `cur`
# (the thread we switched away from) in %eax so the new thread can
# run thread_schedule_tail on it.
#
# Stack on entry (cdecl):
#   20(%esp) = cur   (SWITCH_CUR)
#   24(%esp) = next  (SWITCH_NEXT)
# after pushing 4 callee-saved regs (16 bytes).

switch_threads:
    # Save callee-saved registers.
    pushl %ebx
    pushl %ebp
    pushl %esi
    pushl %edi

    # Get offset of `stack` field in Thread struct.
    movl thread_stack_ofs, %edx

    # Save current stack pointer into cur->stack.
    movl 20(%esp), %eax        # eax = cur
    movl %esp, (%eax,%edx,1)   # cur->stack = esp

    # Switch to next's stack pointer.
    movl 24(%esp), %ecx        # ecx = next
    movl (%ecx,%edx,1), %esp   # esp = next->stack

    # Restore callee-saved registers from next's stack.
    popl %edi
    popl %esi
    popl %ebp
    popl %ebx

    # Return cur (still in %eax) to the caller.
    ret

.globl switch_entry
.type switch_entry, @function

# switch_entry()
#
# The first time a new thread is scheduled, switch_threads "returns"
# here (because we set eip = switch_entry in the SwitchThreadsFrame).
# At this point %eax = prev (the thread we switched from).
#
# The stack currently has the SwitchThreadsFrame's `cur` and `next`
# arguments above us. Discard them, then call thread_schedule_tail(prev).

switch_entry:
    addl $8, %esp              # discard cur/next args from SwitchThreadsFrame
    pushl %eax                 # push prev as argument
    call thread_schedule_tail
    addl $4, %esp              # pop argument

    # Now return into kernel_thread (address is in SwitchEntryFrame.eip).
    ret
