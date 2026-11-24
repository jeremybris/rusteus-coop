//! Process privilege escalation and handle acquisition helpers.

use nucleus_common::error::{NucleusError, NucleusResult};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
#[cfg(windows)]
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

/// Attempts to enable `SeDebugPrivilege` on the current process token.
#[cfg(windows)]
pub fn enable_debug_privilege() -> NucleusResult<()> {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return Err(NucleusError::Platform(
                "OpenProcessToken failed".to_string(),
            ));
        }

        let mut luid = LUID::default();
        let priv_name = windows::core::w!("SeDebugPrivilege");
        if LookupPrivilegeValueW(None, priv_name, &mut luid).is_err() {
            let _ = CloseHandle(token);
            return Err(NucleusError::Platform(
                "LookupPrivilegeValueW failed".to_string(),
            ));
        }

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let res = AdjustTokenPrivileges(token, false, Some(&mut tp), 0, None, None);
        let _ = CloseHandle(token);

        if res.is_err() {
            return Err(NucleusError::Platform(
                "AdjustTokenPrivileges failed".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn enable_debug_privilege() -> NucleusResult<()> {
    Ok(())
}

/// Opens a target process handle with the necessary rights for DLL injection and memory operations.
#[cfg(windows)]
pub fn open_target_process(pid: u32, write_access: bool) -> NucleusResult<HANDLE> {
    let desired_access = if write_access {
        PROCESS_CREATE_THREAD
            | PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ
    } else {
        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ
    };

    unsafe {
        match OpenProcess(desired_access, false, pid) {
            Ok(handle) => Ok(handle),
            Err(first_err) => {
                // If denied, attempt to elevate SeDebugPrivilege and retry
                let _ = enable_debug_privilege();
                match OpenProcess(desired_access, false, pid) {
                    Ok(handle) => Ok(handle),
                    Err(second_err) => Err(NucleusError::Platform(format!(
                        "OpenProcess failed for PID {}: {} (initial error: {})",
                        pid, second_err, first_err
                    ))),
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn open_target_process(pid: u32, _write_access: bool) -> NucleusResult<usize> {
    let _ = pid;
    Err(NucleusError::Unsupported(
        "Process handle acquisition not supported on non-Windows".to_string(),
    ))
}
