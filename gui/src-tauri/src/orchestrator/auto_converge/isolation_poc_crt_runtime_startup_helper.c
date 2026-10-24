/*
 * CRT-free API-set startup comparator.
 * Retains a direct api-ms-win-crt-runtime-l1-1-0.dll!_initterm import but never calls it.
 * Performs no file, network, credential, environment, registry, profile, or UI access.
 */
__declspec(dllimport) void __cdecl _initterm(const void **first, const void **last);
__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

typedef void(__cdecl *PocInitterm)(const void **first, const void **last);
static PocInitterm volatile crt_runtime_import_reference = _initterm;

__attribute__((noreturn)) void poc_crt_runtime_startup_entry(void) {
    if (crt_runtime_import_reference == 0) {
        ExitProcess(4);
    }
    ExitProcess(0);
}
