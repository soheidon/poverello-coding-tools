/*
 * CRT-free USERENV startup comparator.
 * It retains a direct GetUserProfileDirectoryW import but never calls it.
 * It performs no profile, file, network, credential, registry, or UI access.
 */
__declspec(dllimport) int __stdcall
GetUserProfileDirectoryW(void *token, unsigned short *path, unsigned long *length);
__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

typedef int(__stdcall *PocGetUserProfileDirectoryW)(
    void *token,
    unsigned short *path,
    unsigned long *length);
static PocGetUserProfileDirectoryW volatile userenv_import_reference =
    GetUserProfileDirectoryW;

__attribute__((noreturn)) void poc_userenv_startup_entry(void) {
    if (userenv_import_reference == 0) {
        ExitProcess(4);
    }
    ExitProcess(0);
}
