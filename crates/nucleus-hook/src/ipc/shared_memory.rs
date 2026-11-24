//! Cross-process shared memory buffer implementation using Windows File Mapping APIs.

use nucleus_common::error::{NucleusError, NucleusResult};
use std::ffi::c_void;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
};
#[cfg(windows)]
use windows::core::PCWSTR;

/// High-performance shared memory segment for telemetry and zero-copy game state exchange.
pub struct SharedMemorySegment {
    #[cfg(windows)]
    handle: HANDLE,
    ptr: *mut u8,
    size: usize,
    name: String,
}

// SAFETY: Synchronization across process boundaries is the caller's responsibility
// or coordinated via atomic variables/events. The pointer is owned exclusively by this struct.
unsafe impl Send for SharedMemorySegment {}
unsafe impl Sync for SharedMemorySegment {}

impl SharedMemorySegment {
    /// Creates a new named shared memory segment with the specified capacity in bytes.
    pub fn create(name: &str, size: usize) -> NucleusResult<Self> {
        #[cfg(windows)]
        {
            if size == 0 {
                return Err(NucleusError::Ipc(
                    "Shared memory size must be greater than zero".into(),
                ));
            }

            let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE,
                    0,
                    size as u32,
                    PCWSTR::from_raw(wide_name.as_ptr()),
                )
            }
            .map_err(|e| {
                NucleusError::Ipc(format!("CreateFileMappingW failed for '{name}': {e}"))
            })?;

            let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size) };

            if view.Value.is_null() {
                let err = std::io::Error::last_os_error();
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(NucleusError::Ipc(format!(
                    "MapViewOfFile failed for '{name}': {err}"
                )));
            }

            Ok(Self {
                handle,
                ptr: view.Value as *mut u8,
                size,
                name: name.to_string(),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (name, size);
            Err(NucleusError::Platform(
                "Shared memory is only supported on Windows".into(),
            ))
        }
    }

    /// Opens an existing named shared memory segment created by another process.
    pub fn open(name: &str) -> NucleusResult<Self> {
        #[cfg(windows)]
        {
            let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = unsafe {
                OpenFileMappingW(
                    FILE_MAP_ALL_ACCESS.0,
                    false,
                    PCWSTR::from_raw(wide_name.as_ptr()),
                )
            }
            .map_err(|e| NucleusError::Ipc(format!("OpenFileMappingW failed for '{name}': {e}")))?;

            // Map full view (0 bytes to map = entire mapped file)
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 0) };

            if view.Value.is_null() {
                let err = std::io::Error::last_os_error();
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(NucleusError::Ipc(format!(
                    "MapViewOfFile failed for '{name}': {err}"
                )));
            }

            // Determine mapped file size via VirtualQuery
            let mut mem_info = windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION::default();
            let queried = unsafe {
                windows::Win32::System::Memory::VirtualQuery(
                    Some(view.Value),
                    &mut mem_info,
                    std::mem::size_of::<windows::Win32::System::Memory::MEMORY_BASIC_INFORMATION>(),
                )
            };

            let size = if queried > 0 {
                mem_info.RegionSize
            } else {
                65536 // Fallback default chunk size
            };

            Ok(Self {
                handle,
                ptr: view.Value as *mut u8,
                size,
                name: name.to_string(),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            Err(NucleusError::Platform(
                "Shared memory is only supported on Windows".into(),
            ))
        }
    }

    /// Returns the capacity of the shared memory region in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the name of the shared memory segment.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a raw pointer to the base of the mapped memory region.
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// Returns a raw mutable pointer to the base of the mapped memory region.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    /// Writes a slice of bytes into the shared memory segment at the given byte offset.
    pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> NucleusResult<()> {
        if offset
            .checked_add(data.len())
            .map_or(true, |end| end > self.size)
        {
            return Err(NucleusError::Ipc(format!(
                "Write out of bounds: offset {} + len {} exceeds shared memory size {}",
                offset,
                data.len(),
                self.size
            )));
        }

        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(offset), data.len());
        }
        Ok(())
    }

    /// Reads a slice of bytes from the shared memory segment at the given byte offset into a buffer.
    pub fn read_bytes(&self, offset: usize, buffer: &mut [u8]) -> NucleusResult<()> {
        if offset
            .checked_add(buffer.len())
            .map_or(true, |end| end > self.size)
        {
            return Err(NucleusError::Ipc(format!(
                "Read out of bounds: offset {} + len {} exceeds shared memory size {}",
                offset,
                buffer.len(),
                self.size
            )));
        }

        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr.add(offset), buffer.as_mut_ptr(), buffer.len());
        }
        Ok(())
    }
}

impl Drop for SharedMemorySegment {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            if !self.ptr.is_null() {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut c_void,
                });
                self.ptr = std::ptr::null_mut();
            }
            if !self.handle.is_invalid() {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
