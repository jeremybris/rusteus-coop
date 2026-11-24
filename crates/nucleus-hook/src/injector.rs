//! Dynamic link library (DLL) injection and ejection mechanics.

use crate::pe_inspector::{PeInspector, PeMachine};
use crate::privilege::open_target_process;
use crate::remote_call::RemoteRpc;
use crate::remote_memory::RemoteMemoryGuard;
use nucleus_common::dll_injection::{DllInjectionConfig, TargetArchitecture};
use nucleus_common::error::{NucleusError, NucleusResult};
use std::path::Path;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE, WAIT_OBJECT_0};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeProcess, GetExitCodeThread, OpenThread, QueueUserAPC,
    THREAD_SET_CONTEXT, WaitForSingleObject,
};

/// Utilities for injecting custom dynamic libraries into remote target processes.
pub struct DllInjector;

impl DllInjector {
    /// Injects a specified DLL file into the address space of a target process.
    pub fn inject_dll(pid: u32, dll_path: &Path) -> NucleusResult<()> {
        let config = DllInjectionConfig::new(dll_path.to_string_lossy().to_string());
        Self::inject_advanced(pid, &config).map(|_| ())
    }

    /// Injects a dynamic library using an advanced configuration specification, verifying
    /// PE architectures, isolating remote crashes, and executing startup exports.
    ///
    /// Returns the remote module base address upon success.
    pub fn inject_advanced(pid: u32, config: &DllInjectionConfig) -> NucleusResult<usize> {
        let dll_path = Path::new(&config.path);
        if !dll_path.exists() {
            return Err(NucleusError::Platform(format!(
                "DLL file does not exist: {:?}",
                dll_path
            )));
        }

        // Verify configuration architecture expectations if explicitly set
        let pe_info = PeInspector::inspect_file(dll_path)?;
        match config.architecture {
            TargetArchitecture::X64 => {
                if pe_info.machine != PeMachine::X64 {
                    return Err(NucleusError::PeInspection(format!(
                        "DLL {:?} is not 64-bit AMD64 as required by configuration (found {:?})",
                        dll_path, pe_info.machine
                    )));
                }
            }
            TargetArchitecture::X86 => {
                if pe_info.machine != PeMachine::X86 {
                    return Err(NucleusError::PeInspection(format!(
                        "DLL {:?} is not 32-bit i386 as required by configuration (found {:?})",
                        dll_path, pe_info.machine
                    )));
                }
            }
            TargetArchitecture::Auto => {}
        }

        // Pre-flight check: Verify DLL architecture matches the target process architecture
        let _ = PeInspector::verify_compatibility(dll_path, pid)?;

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;

            let abs_path =
                std::fs::canonicalize(dll_path).unwrap_or_else(|_| dll_path.to_path_buf());
            let wide_path: Vec<u16> = abs_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let size = wide_path.len() * std::mem::size_of::<u16>();

            let process = open_target_process(pid, true)?;

            let module_base = unsafe {
                // RAII guard automatically cleans up memory if any error occurs
                let mem_guard = match RemoteMemoryGuard::allocate(process, size) {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = CloseHandle(process);
                        return Err(e);
                    }
                };

                let path_bytes = std::slice::from_raw_parts(wide_path.as_ptr() as *const u8, size);
                if let Err(e) = mem_guard.write(path_bytes) {
                    let _ = CloseHandle(process);
                    return Err(e);
                }

                let kernel32 = match GetModuleHandleA(windows::core::s!("kernel32.dll")) {
                    Ok(k) => k,
                    Err(e) => {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Failed to get kernel32 handle: {}",
                            e
                        )));
                    }
                };

                let load_library_w =
                    match GetProcAddress(kernel32, windows::core::s!("LoadLibraryW")) {
                        Some(f) => f,
                        None => {
                            let _ = CloseHandle(process);
                            return Err(NucleusError::Platform(
                                "LoadLibraryW symbol not found in kernel32".to_string(),
                            ));
                        }
                    };

                if config.method == nucleus_common::dll_injection::InjectionMethod::QueueUserApc {
                    let tid = match Self::find_primary_thread_id(pid) {
                        Ok(t) => t,
                        Err(e) => {
                            let _ = CloseHandle(process);
                            return Err(e);
                        }
                    };

                    let thread = match OpenThread(THREAD_SET_CONTEXT, false, tid) {
                        Ok(t) => t,
                        Err(e) => {
                            let _ = CloseHandle(process);
                            return Err(NucleusError::Platform(format!(
                                "Failed to open thread {}: {}",
                                tid, e
                            )));
                        }
                    };

                    let res = QueueUserAPC(
                        Some(std::mem::transmute(load_library_w)),
                        thread,
                        mem_guard.address() as usize,
                    );
                    let _ = CloseHandle(thread);

                    if res == 0 {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "QueueUserAPC failed to queue LoadLibraryW on thread {}",
                            tid
                        )));
                    }

                    // Leak memory guard so DLL path buffer remains valid until the suspended thread resumes
                    let leaked_addr = mem_guard.leak() as usize;
                    let _ = CloseHandle(process);
                    tracing::info!(
                        "Queued QueueUserAPC on thread {} for DLL {:?}",
                        tid,
                        dll_path
                    );
                    return Ok(leaked_addr);
                }

                let thread_res = CreateRemoteThread(
                    process,
                    None,
                    0,
                    Some(std::mem::transmute(load_library_w)),
                    Some(mem_guard.address() as *const std::ffi::c_void),
                    0,
                    None,
                );

                let thread = match thread_res {
                    Ok(th) => th,
                    Err(e) => {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "CreateRemoteThread failed: {}",
                            e
                        )));
                    }
                };

                // Bounded polling loop: wait up to 10s monitoring process liveness
                let timeout_ms = 10_000u64;
                let poll_interval_ms = 100u64;
                let mut elapsed = 0u64;
                let mut exit_code = 0u32;

                loop {
                    let wait_res = WaitForSingleObject(thread, poll_interval_ms as u32);
                    if wait_res == WAIT_OBJECT_0 {
                        let _ = GetExitCodeThread(thread, &mut exit_code);
                        break;
                    }

                    // Detect target process crash inside DllMain
                    let mut proc_exit = 0u32;
                    let _ = GetExitCodeProcess(process, &mut proc_exit);
                    if proc_exit != STILL_ACTIVE.0 as u32 {
                        let _ = CloseHandle(thread);
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Target process PID {} crashed during LoadLibraryW of {:?} (exit code: {:#x})",
                            pid, dll_path, proc_exit
                        )));
                    }

                    elapsed += poll_interval_ms;
                    if elapsed >= timeout_ms {
                        let _ = CloseHandle(thread);
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Timeout waiting for LoadLibraryW of {:?} to complete in PID {}",
                            dll_path, pid
                        )));
                    }
                }

                let _ = CloseHandle(thread);
                let _ = CloseHandle(process);

                // Exit code of LoadLibraryW remote thread is the remote HMODULE base address
                if exit_code == 0 {
                    return Err(NucleusError::DllInjection(format!(
                        "LoadLibraryW returned NULL for {:?} in PID {} (possible missing dependencies or DllMain returned FALSE)",
                        dll_path, pid
                    )));
                }

                exit_code as usize
            };

            // Execute post-injection export if configured
            if let Some(ref export) = config.export_name {
                let _ = RemoteRpc::call_remote_export(
                    pid,
                    dll_path,
                    module_base,
                    export,
                    config.export_argument.as_deref(),
                )?;
            }

            Ok(module_base)
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, config);
            Err(NucleusError::Unsupported(
                "DLL injection only supported on Windows".to_string(),
            ))
        }
    }

    /// Ejects an injected module from the target process address space via `FreeLibrary`.
    pub fn eject_dll(pid: u32, module_handle: usize) -> NucleusResult<()> {
        if module_handle == 0 {
            return Ok(());
        }

        #[cfg(windows)]
        {
            let process = open_target_process(pid, true)?;

            unsafe {
                let kernel32 = match GetModuleHandleA(windows::core::s!("kernel32.dll")) {
                    Ok(k) => k,
                    Err(e) => {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Failed to get kernel32 handle: {}",
                            e
                        )));
                    }
                };

                let free_library = match GetProcAddress(kernel32, windows::core::s!("FreeLibrary"))
                {
                    Some(f) => f,
                    None => {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(
                            "FreeLibrary symbol not found in kernel32".to_string(),
                        ));
                    }
                };

                let thread_res = CreateRemoteThread(
                    process,
                    None,
                    0,
                    Some(std::mem::transmute(free_library)),
                    Some(module_handle as *const std::ffi::c_void),
                    0,
                    None,
                );

                let thread = match thread_res {
                    Ok(th) => th,
                    Err(e) => {
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "CreateRemoteThread for FreeLibrary failed: {}",
                            e
                        )));
                    }
                };

                let _ = WaitForSingleObject(thread, 5000);
                let mut exit_code = 0u32;
                let _ = GetExitCodeThread(thread, &mut exit_code);

                let _ = CloseHandle(thread);
                let _ = CloseHandle(process);

                if exit_code == 0 {
                    tracing::warn!(
                        "FreeLibrary returned 0 for module {:#x} in PID {}",
                        module_handle,
                        pid
                    );
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, module_handle);
        }

        Ok(())
    }

    /// Finds the primary thread ID of a given process using ToolHelp snapshot.
    #[cfg(windows)]
    pub fn find_primary_thread_id(pid: u32) -> NucleusResult<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).map_err(|e| {
                NucleusError::Platform(format!("Failed to create thread snapshot: {}", e))
            })?;
            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            let mut first = true;
            while if first {
                first = false;
                Thread32First(snapshot, &mut entry).is_ok()
            } else {
                Thread32Next(snapshot, &mut entry).is_ok()
            } {
                if entry.th32OwnerProcessID == pid {
                    let tid = entry.th32ThreadID;
                    let _ = CloseHandle(snapshot);
                    return Ok(tid);
                }
            }
            let _ = CloseHandle(snapshot);
            Err(NucleusError::Platform(format!(
                "No thread found for PID {}",
                pid
            )))
        }
    }
}
