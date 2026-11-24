//! Remote procedure call (RPC) execution of exported functions inside target processes.

use crate::pe_inspector::PeInspector;
use crate::privilege::open_target_process;
use crate::remote_memory::RemoteMemoryGuard;
use nucleus_common::error::{NucleusError, NucleusResult};
use std::ffi::c_void;
use std::path::Path;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE, WAIT_OBJECT_0};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeProcess, GetExitCodeThread, WaitForSingleObject,
};

/// Remote procedure execution helper for calling exported functions in injected DLLs.
pub struct RemoteRpc;

impl RemoteRpc {
    /// Calls an exported function from a loaded module in the target process.
    pub fn call_remote_export(
        pid: u32,
        dll_path: &Path,
        module_base: usize,
        export_name: &str,
        argument: Option<&str>,
    ) -> NucleusResult<u32> {
        let pe_info = PeInspector::inspect_file(dll_path)?;
        let rva = pe_info.exports.get(export_name).copied().ok_or_else(|| {
            NucleusError::DllInjection(format!(
                "Export '{}' not found in PE export table of {:?}",
                export_name, dll_path
            ))
        })?;

        let remote_func_addr = (module_base as u64 + rva as u64) as *const c_void;

        #[cfg(windows)]
        {
            let process = open_target_process(pid, true)?;

            let (arg_ptr, _guard) = if let Some(arg_str) = argument {
                let bytes = arg_str.as_bytes();
                let mut data_with_null = Vec::with_capacity(bytes.len() + 1);
                data_with_null.extend_from_slice(bytes);
                data_with_null.push(0);

                let guard = RemoteMemoryGuard::allocate(process, data_with_null.len())?;
                guard.write(&data_with_null)?;
                let ptr = guard.address() as *const c_void;
                (Some(ptr), Some(guard))
            } else {
                (None, None)
            };

            unsafe {
                let thread_res = CreateRemoteThread(
                    process,
                    None,
                    0,
                    Some(std::mem::transmute(remote_func_addr)),
                    arg_ptr,
                    0,
                    None,
                );

                let thread = thread_res.map_err(|e| {
                    let _ = CloseHandle(process);
                    NucleusError::Platform(format!(
                        "CreateRemoteThread failed for export '{}': {}",
                        export_name, e
                    ))
                })?;

                // Bounded polling loop: wait up to 10s checking process liveness
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

                    // Check if target process is still running or crashed
                    let mut proc_exit = 0u32;
                    let _ = GetExitCodeProcess(process, &mut proc_exit);
                    if proc_exit != STILL_ACTIVE.0 as u32 {
                        let _ = CloseHandle(thread);
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Target process PID {} crashed during execution of export '{}' (exit code: {:#x})",
                            pid, export_name, proc_exit
                        )));
                    }

                    elapsed += poll_interval_ms;
                    if elapsed >= timeout_ms {
                        let _ = CloseHandle(thread);
                        let _ = CloseHandle(process);
                        return Err(NucleusError::Platform(format!(
                            "Timeout waiting for export '{}' to complete in PID {}",
                            export_name, pid
                        )));
                    }
                }

                let _ = CloseHandle(thread);
                let _ = CloseHandle(process);
                Ok(exit_code)
            }
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, module_base, export_name, argument);
            Err(NucleusError::Unsupported(
                "Remote export execution only supported on Windows".to_string(),
            ))
        }
    }
}
