//! Windows concrete implementation of `DllInjectionManager` and `ProcessMemoryManager`.

use nucleus_common::dll_injection::DllInjectionConfig;
use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_platform::{DllInjectionManager, ProcessMemoryManager};
use std::ffi::c_void;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
#[cfg(windows)]
use windows::Win32::System::Memory::{PAGE_PROTECTION_FLAGS, VirtualProtectEx};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

/// Windows process memory manipulation manager.
#[derive(Default)]
pub struct WindowsProcessMemoryManager;

impl WindowsProcessMemoryManager {
    /// Creates a new `WindowsProcessMemoryManager`.
    pub fn new() -> Self {
        Self
    }
}

impl ProcessMemoryManager for WindowsProcessMemoryManager {
    fn read_memory(&self, pid: u32, address: usize, buffer: &mut [u8]) -> NucleusResult<usize> {
        #[cfg(windows)]
        unsafe {
            let process: HANDLE =
                OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid).map_err(
                    |e| {
                        NucleusError::Platform(format!(
                            "OpenProcess for read_memory failed on PID {}: {}",
                            pid, e
                        ))
                    },
                )?;

            let mut bytes_read = 0;
            let res = ReadProcessMemory(
                process,
                address as *const c_void,
                buffer.as_mut_ptr() as _,
                buffer.len(),
                Some(&mut bytes_read),
            );
            let _ = CloseHandle(process);

            if res.is_err() {
                return Err(NucleusError::Platform(format!(
                    "ReadProcessMemory failed at address {:#x} in PID {}",
                    address, pid
                )));
            }
            Ok(bytes_read)
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, address, buffer);
            Err(NucleusError::Unsupported(
                "Memory reading only supported on Windows".to_string(),
            ))
        }
    }

    fn write_memory(&self, pid: u32, address: usize, data: &[u8]) -> NucleusResult<usize> {
        #[cfg(windows)]
        unsafe {
            let process: HANDLE = OpenProcess(PROCESS_VM_WRITE | PROCESS_VM_OPERATION, false, pid)
                .map_err(|e| {
                    NucleusError::Platform(format!(
                        "OpenProcess for write_memory failed on PID {}: {}",
                        pid, e
                    ))
                })?;

            let mut bytes_written = 0;
            let res = WriteProcessMemory(
                process,
                address as *mut c_void,
                data.as_ptr() as _,
                data.len(),
                Some(&mut bytes_written),
            );
            let _ = CloseHandle(process);

            if res.is_err() {
                return Err(NucleusError::Platform(format!(
                    "WriteProcessMemory failed at address {:#x} in PID {}",
                    address, pid
                )));
            }
            Ok(bytes_written)
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, address, data);
            Err(NucleusError::Unsupported(
                "Memory writing only supported on Windows".to_string(),
            ))
        }
    }

    fn virtual_protect(
        &self,
        pid: u32,
        address: usize,
        size: usize,
        new_protect: u32,
    ) -> NucleusResult<u32> {
        #[cfg(windows)]
        unsafe {
            let process: HANDLE = OpenProcess(PROCESS_VM_OPERATION, false, pid).map_err(|e| {
                NucleusError::Platform(format!(
                    "OpenProcess for virtual_protect failed on PID {}: {}",
                    pid, e
                ))
            })?;

            let mut old_protect = PAGE_PROTECTION_FLAGS(0);
            let res = VirtualProtectEx(
                process,
                address as *const c_void,
                size,
                PAGE_PROTECTION_FLAGS(new_protect),
                &mut old_protect,
            );
            let _ = CloseHandle(process);

            if res.is_err() {
                return Err(NucleusError::Platform(format!(
                    "VirtualProtectEx failed at address {:#x} in PID {}",
                    address, pid
                )));
            }
            Ok(old_protect.0)
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, address, size, new_protect);
            Err(NucleusError::Unsupported(
                "VirtualProtect only supported on Windows".to_string(),
            ))
        }
    }
}

/// Windows dynamic library injection manager.
#[derive(Default)]
pub struct WindowsDllInjectionManager;

impl WindowsDllInjectionManager {
    /// Creates a new `WindowsDllInjectionManager`.
    pub fn new() -> Self {
        Self
    }
}

impl DllInjectionManager for WindowsDllInjectionManager {
    fn inject_dll(&self, pid: u32, config: &DllInjectionConfig) -> NucleusResult<usize> {
        let _ = (pid, config);
        // Concrete injection logic lives in nucleus-hook (which links higher in the dependency hierarchy)
        // Platform service returns error or delegates when called from platform level
        Err(NucleusError::Platform(
            "Use nucleus-hook::DllInjector for full DLL injection pipeline".to_string(),
        ))
    }

    fn call_export(
        &self,
        pid: u32,
        module_handle: usize,
        export: &str,
        arg: Option<&str>,
    ) -> NucleusResult<u32> {
        let _ = (pid, module_handle, export, arg);
        Err(NucleusError::Platform(
            "Use nucleus-hook::RemoteRpc for remote export execution".to_string(),
        ))
    }

    fn eject_dll(&self, pid: u32, module_handle: usize) -> NucleusResult<()> {
        let _ = (pid, module_handle);
        Err(NucleusError::Platform(
            "Use nucleus-hook::DllInjector::eject_dll for ejection".to_string(),
        ))
    }
}
