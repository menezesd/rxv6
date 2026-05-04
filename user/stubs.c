/* Minimal stubs for functions needed by the Pintos user library
   but not relevant for the args test. */

/* debug_backtrace - called by debug_panic (PANIC/ASSERT).
   In the real Pintos it walks the call stack; here we do nothing. */
void debug_backtrace(void) {}

/* random_ulong - used by shuffle() in tests/lib.c.
   The args test never calls shuffle(), so a stub is fine. */
unsigned long random_ulong(void) { return 0; }
