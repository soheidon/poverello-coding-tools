/*
 * CRT-free VCRUNTIME startup comparator.
 * Retains a direct VCRUNTIME140.dll!memcpy import but never calls it.
 * Performs no file, network, credential, environment, registry, profile, or UI access.
 */
__declspec(dllimport) void *__cdecl memcpy(void *destination, const void *source, unsigned long long count);
__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

typedef void *(__cdecl *PocMemcpy)(void *destination, const void *source, unsigned long long count);
static PocMemcpy volatile vcruntime_import_reference = memcpy;

__attribute__((noreturn)) void poc_vcruntime_startup_entry(void) {
    if (vcruntime_import_reference == 0) {
        ExitProcess(4);
    }
    ExitProcess(0);
}
