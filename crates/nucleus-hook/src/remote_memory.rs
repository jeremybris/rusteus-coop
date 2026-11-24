//! RAII guard for memory allocated inside a target process address space.

use nucleus_common::error::{NucleusError, NucleusResult};
use std::ffi::c_void;

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
#[cfg(windows)]
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
};

/// An RAII guard managing a block of memory allocated in a target process.
///
/// Automatically calls `VirtualFreeEx(MEM_RELEASE)` upon drop to guarantee
/// target address space cleanup on any error or panic.
pub struct RemoteMemoryGuard {
    #[cfg(windows)]
    process: HANDLE,
    address: *mut c_void,
    size: usize,
    committed: bool,
}

impl RemoteMemoryGuard {
    /// Allocates `size` bytes of read-write memory in the target process.
    #[cfg(windows)]
    pub fn allocate(process: HANDLE, size: usize) -> NucleusResult<Self> {
        if size == 0 {
            return Err(NucleusError::Platform(
                "Cannot allocate zero bytes in remote process".to_string(),
            ));
        }

        // SAFETY: `process` is a valid open process handle with PROCESS_VM_OPERATION.
        // VirtualAllocEx creates a new committed and reserved block in the remote address space.
        let address = unsafe {
            VirtualAllocEx(
                process,
                None,
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };

        if address.is_null() {
            return Err(NucleusError::Platform(
                "VirtualAllocEx failed to allocate remote memory".to_string(),
            ));
        }

        Ok(Self {
            process,
            address,
            size,
            committed: true,
        })
    }

    #[cfg(not(windows))]
    pub fn allocate(_process: usize, _size: usize) -> NucleusResult<Self> {
        Err(NucleusError::Unsupported(
            "Remote memory allocation not supported on this platform".to_string(),
        ))
    }

    /// Returns the remote base address.
    pub fn address(&self) -> *mut c_void {
        self.address
    }

    /// Returns the size of the allocated memory block.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Writes data into the remote memory block.
    pub fn write(&self, data: &[u8]) -> NucleusResult<()> {
        if data.len() > self.size {
            return Err(NucleusError::Platform(format!(
                "Data length ({}) exceeds remote memory block capacity ({})",
                data.len(),
                self.size
            )));
        }

        #[cfg(windows)]
        unsafe {
            let res = WriteProcessMemory(
                self.process,
                self.address,
                data.as_ptr() as _,
                data.len(),
                None,
            );
            if res.is_err() {
                return Err(NucleusError::Platform(
                    "WriteProcessMemory failed".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Reads data from the remote memory block into a local buffer.
    pub fn read(&self, buffer: &mut [u8]) -> NucleusResult<usize> {
        let read_len = buffer.len().min(self.size);

        #[cfg(windows)]
        unsafe {
            let mut bytes_read = 0;
            let res = ReadProcessMemory(
                self.process,
                self.address,
                buffer.as_mut_ptr() as _,
                read_len,
                Some(&mut bytes_read),
            );
            if res.is_err() {
                return Err(NucleusError::Platform(
                    "ReadProcessMemory failed".to_string(),
                ));
            }
            Ok(bytes_read)
        }

        #[cfg(not(windows))]
        {
            let _ = buffer;
            Ok(0)
        }
    }

    /// Defers deallocation, transferring ownership of the allocated memory to the caller.
    pub fn leak(mut self) -> *mut c_void {
        self.committed = false;
        self.address
    }
}

impl Drop for RemoteMemoryGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if !self.address.is_null() && self.committed {
            // SAFETY: `self.address` was allocated via VirtualAllocEx in `self.process` and
            // has not been previously freed. MEM_RELEASE releases the allocated virtual memory.
            unsafe {
                let _ = VirtualFreeEx(self.process, self.address, 0, MEM_RELEASE);
            }
        }
    }
}
