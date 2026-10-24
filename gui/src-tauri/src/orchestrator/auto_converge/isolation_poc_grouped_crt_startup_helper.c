/*
 * CRT-free grouped direct-import comparator.
 * Retains direct imports observed in the Rust test PE, but never calls them.
 * Performs no file, network, credential, environment, registry, profile, or UI access.
 */
#define IMPORT(name) __declspec(dllimport) void __cdecl name(void)

IMPORT(__current_exception_context);
IMPORT(memcpy);
IMPORT(memset);
IMPORT(memmove);
IMPORT(memcmp);
IMPORT(_CxxThrowException);
IMPORT(__C_specific_handler);
IMPORT(__current_exception);
IMPORT(__CxxFrameHandler3);

IMPORT(fmod);
IMPORT(pow);
IMPORT(ceil);
IMPORT(__setusermatherr);
IMPORT(floor);

IMPORT(_initterm_e);
IMPORT(_initterm);
IMPORT(exit);
IMPORT(_initialize_narrow_environment);
IMPORT(_configure_narrow_argv);
IMPORT(__p___argv);
IMPORT(_set_app_type);
IMPORT(_seh_filter_exe);
IMPORT(__p___argc);
IMPORT(_c_exit);
IMPORT(_register_thread_local_exe_atexit_callback);
IMPORT(_initialize_onexit_table);
IMPORT(_register_onexit_function);
IMPORT(_crt_atexit);
IMPORT(terminate);
IMPORT(_cexit);
IMPORT(_exit);
IMPORT(_get_initial_narrow_environment);

IMPORT(__p__commode);
IMPORT(_set_fmode);
IMPORT(_configthreadlocale);
IMPORT(_set_new_mode);
IMPORT(free);

__declspec(dllimport) __attribute__((noreturn)) void __stdcall
ExitProcess(unsigned int exit_code);

typedef void(__cdecl *PocImportedFunction)(void);
static PocImportedFunction volatile grouped_import_references[] = {
    __current_exception_context,
    memcpy,
    memset,
    memmove,
    memcmp,
    _CxxThrowException,
    __C_specific_handler,
    __current_exception,
    __CxxFrameHandler3,
    fmod,
    pow,
    ceil,
    __setusermatherr,
    floor,
    _initterm_e,
    _initterm,
    exit,
    _initialize_narrow_environment,
    _configure_narrow_argv,
    __p___argv,
    _set_app_type,
    _seh_filter_exe,
    __p___argc,
    _c_exit,
    _register_thread_local_exe_atexit_callback,
    _initialize_onexit_table,
    _register_onexit_function,
    _crt_atexit,
    terminate,
    _cexit,
    _exit,
    _get_initial_narrow_environment,
    __p__commode,
    _set_fmode,
    _configthreadlocale,
    _set_new_mode,
    free,
};

__attribute__((noreturn)) void poc_grouped_crt_startup_entry(void) {
    unsigned long count = sizeof(grouped_import_references) / sizeof(grouped_import_references[0]);
    for (unsigned long index = 0; index < count; ++index) {
        if (grouped_import_references[index] == 0) {
            ExitProcess(4);
        }
    }
    ExitProcess(0);
}
