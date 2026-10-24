/*
 * CRT-free Windows startup diagnostic helper.
 * It performs no work except terminating with exit code 0.
 * Build with MinGW GCC using -nostdlib and an explicit custom entry point.
 */
__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

__attribute__((noreturn)) void poc_startup_entry(void) {
    ExitProcess(0);
}
