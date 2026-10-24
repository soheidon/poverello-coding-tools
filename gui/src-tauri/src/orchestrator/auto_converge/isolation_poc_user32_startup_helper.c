/*
 * CRT-free USER32 startup comparator.
 * It queries the current process Window Station and exits with a fixed status.
 * It performs no file, network, credential, environment, registry, or UI work.
 */
__declspec(dllimport) void *__stdcall GetProcessWindowStation(void);
__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

__attribute__((noreturn)) void poc_user32_startup_entry(void) {
    void *station = GetProcessWindowStation();
    ExitProcess(station != 0 ? 0u : 3u);
}
