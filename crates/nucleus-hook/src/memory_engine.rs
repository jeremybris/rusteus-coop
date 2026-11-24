//! In-process memory manipulation engine (Cheat Engine parity).
//!
//! Provides memory scanning (AOB), primitive reading/writing with auto-unprotect,
//! multi-level pointer resolution, and frozen memory value management.

use nucleus_common::error::{NucleusError, NucleusResult};
use std::ffi::c_void;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use crate::privilege::open_target_process;
#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
#[cfg(windows)]
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_GUARD,
    PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE, VirtualProtectEx,
    VirtualQueryEx,
};

/// A frozen memory slot for periodic reinforcement of locked values.
#[derive(Debug, Clone)]
pub struct FrozenMemorySlot {
    /// Unique identifier for this freeze rule.
    pub id: u64,
    /// Target process ID.
    pub pid: u32,
    /// Target memory virtual address.
    pub address: usize,
    /// Raw byte sequence to continuously enforce.
    pub bytes: Vec<u8>,
}

/// Central memory manipulation engine supporting AOB scanning and pointer chaining.
pub struct MemoryEngine;

static NEXT_FREEZE_ID: AtomicU64 = AtomicU64::new(1);
static FROZEN_SLOTS: RwLock<Vec<FrozenMemorySlot>> = RwLock::new(Vec::new());

impl MemoryEngine {
    /// Reads a primitive value (`i32`, `u32`, `f32`, `f64`, etc.) from target process memory.
    pub fn read_value<T: Copy>(pid: u32, address: usize) -> NucleusResult<T> {
        let size = std::mem::size_of::<T>();
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        let buffer = unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, size) };

        Self::read_bytes(pid, address, buffer)?;
        Ok(unsafe { value.assume_init() })
    }

    /// Writes a primitive value to target process memory with auto-unprotect if page is read-only.
    pub fn write_value<T: Copy>(pid: u32, address: usize, value: &T) -> NucleusResult<()> {
        let size = std::mem::size_of::<T>();
        let slice = unsafe { std::slice::from_raw_parts(value as *const T as *const u8, size) };
        Self::write_bytes(pid, address, slice)?;
        Ok(())
    }

    /// Reads raw bytes from target process memory into a provided buffer.
    pub fn read_bytes(pid: u32, address: usize, buffer: &mut [u8]) -> NucleusResult<usize> {
        #[cfg(windows)]
        {
            if buffer.is_empty() {
                return Ok(0);
            }

            let handle = open_target_process(pid, false)?;
            let mut bytes_read: usize = 0;

            let success = unsafe {
                ReadProcessMemory(
                    handle,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len(),
                    Some(&mut bytes_read),
                )
            };

            unsafe {
                let _ = CloseHandle(handle);
            }

            if success.is_err() || bytes_read == 0 {
                let err = std::io::Error::last_os_error();
                return Err(NucleusError::Platform(format!(
                    "ReadProcessMemory failed at 0x{address:X} for PID {pid}: {err}"
                )));
            }

            Ok(bytes_read)
        }
        #[cfg(not(windows))]
        {
            let _ = (pid, address, buffer);
            Err(NucleusError::Platform(
                "Memory reading is only supported on Windows".into(),
            ))
        }
    }

    /// Writes raw bytes to target process memory, automatically removing write protection if needed.
    pub fn write_bytes(pid: u32, address: usize, data: &[u8]) -> NucleusResult<usize> {
        #[cfg(windows)]
        {
            if data.is_empty() {
                return Ok(0);
            }

            let handle = open_target_process(pid, true)?;
            let mut bytes_written: usize = 0;

            let mut success = unsafe {
                WriteProcessMemory(
                    handle,
                    address as *const c_void,
                    data.as_ptr() as *const c_void,
                    data.len(),
                    Some(&mut bytes_written),
                )
            };

            // If initial write failed, attempt VirtualProtect to PAGE_EXECUTE_READWRITE
            if success.is_err() {
                let mut old_protect = PAGE_PROTECTION_FLAGS(0);
                let protect_res = unsafe {
                    VirtualProtectEx(
                        handle,
                        address as *const c_void,
                        data.len(),
                        PAGE_EXECUTE_READWRITE,
                        &mut old_protect,
                    )
                };

                if protect_res.is_ok() {
                    success = unsafe {
                        WriteProcessMemory(
                            handle,
                            address as *const c_void,
                            data.as_ptr() as *const c_void,
                            data.len(),
                            Some(&mut bytes_written),
                        )
                    };

                    // Restore previous protection
                    let mut restore_protect = PAGE_PROTECTION_FLAGS(0);
                    unsafe {
                        let _ = VirtualProtectEx(
                            handle,
                            address as *const c_void,
                            data.len(),
                            old_protect,
                            &mut restore_protect,
                        );
                    }
                }
            }

            unsafe {
                let _ = CloseHandle(handle);
            }

            if success.is_err() {
                let err = std::io::Error::last_os_error();
                return Err(NucleusError::Platform(format!(
                    "WriteProcessMemory failed at 0x{address:X} for PID {pid}: {err}"
                )));
            }

            Ok(bytes_written)
        }
        #[cfg(not(windows))]
        {
            let _ = (pid, address, data);
            Err(NucleusError::Platform(
                "Memory writing is only supported on Windows".into(),
            ))
        }
    }

    /// Scans a specific memory range for a byte pattern with a wildcard mask (`"xxxx"` or `"x?xx"`).
    pub fn aob_scan_range(
        pid: u32,
        start_addr: usize,
        size: usize,
        pattern: &[u8],
        mask: &str,
    ) -> NucleusResult<Vec<usize>> {
        if pattern.is_empty() || pattern.len() != mask.len() || size < pattern.len() {
            return Ok(Vec::new());
        }

        let mask_bytes = mask.as_bytes();
        let chunk_size = 65536; // 64 KB read buffer
        let overlap = pattern.len() - 1;
        let mut matches = Vec::new();

        let mut offset = 0;
        let mut buffer = vec![0u8; chunk_size];

        while offset < size {
            let to_read = (size - offset).min(chunk_size);
            let read_slice = &mut buffer[..to_read];

            if let Ok(bytes_read) = Self::read_bytes(pid, start_addr + offset, read_slice) {
                if bytes_read >= pattern.len() {
                    let search_limit = bytes_read - pattern.len();
                    for i in 0..=search_limit {
                        let mut is_match = true;
                        for j in 0..pattern.len() {
                            if mask_bytes[j] != b'?' && read_slice[i + j] != pattern[j] {
                                is_match = false;
                                break;
                            }
                        }
                        if is_match {
                            matches.push(start_addr + offset + i);
                        }
                    }
                }
                if bytes_read < to_read || offset + to_read >= size {
                    break;
                }
            } else {
                break;
            }

            let advance = to_read.saturating_sub(overlap);
            if advance == 0 {
                break;
            }
            offset += advance;
        }

        Ok(matches)
    }

    /// Scans all committed readable virtual memory regions of a process for a pattern with wildcard mask.
    pub fn aob_scan(pid: u32, pattern: &[u8], mask: &str) -> NucleusResult<Vec<usize>> {
        #[cfg(windows)]
        {
            let handle = open_target_process(pid, false)?;
            let mut matches = Vec::new();
            let mut current_addr: usize = 0;

            loop {
                let mut mem_info = MEMORY_BASIC_INFORMATION::default();
                let result = unsafe {
                    VirtualQueryEx(
                        handle,
                        Some(current_addr as *const c_void),
                        &mut mem_info,
                        std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                    )
                };

                if result == 0 {
                    break;
                }

                let is_committed = mem_info.State == MEM_COMMIT;
                let protect = mem_info.Protect;
                let is_readable = (protect & PAGE_READONLY).0 != 0
                    || (protect & PAGE_READWRITE).0 != 0
                    || (protect & PAGE_EXECUTE_READ).0 != 0
                    || (protect & PAGE_EXECUTE_READWRITE).0 != 0;
                let is_guarded = (protect & PAGE_GUARD).0 != 0 || (protect & PAGE_NOACCESS).0 != 0;

                if is_committed && is_readable && !is_guarded {
                    let region_base = mem_info.BaseAddress as usize;
                    let region_size = mem_info.RegionSize;
                    if let Ok(sub_matches) =
                        Self::aob_scan_range(pid, region_base, region_size, pattern, mask)
                    {
                        matches.extend(sub_matches);
                    }
                }

                match current_addr.checked_add(mem_info.RegionSize) {
                    Some(next) => current_addr = next,
                    None => break,
                }
            }

            unsafe {
                let _ = CloseHandle(handle);
            }
            Ok(matches)
        }
        #[cfg(not(windows))]
        {
            let _ = (pid, pattern, mask);
            Err(NucleusError::Platform(
                "AOB scanning is only supported on Windows".into(),
            ))
        }
    }

    /// Resolves a multi-level pointer chain: `base -> [offset_0] -> [offset_1] -> ... -> final_address`.
    pub fn resolve_pointer_chain(
        pid: u32,
        base: usize,
        offsets: &[usize],
        is_64bit: bool,
    ) -> NucleusResult<usize> {
        if offsets.is_empty() {
            return Ok(base);
        }

        let mut current_addr = base;

        for (i, &offset) in offsets.iter().enumerate() {
            let pointer_target = current_addr.checked_add(offset).ok_or_else(|| {
                NucleusError::Platform(format!(
                    "Pointer arithmetic overflow at offset index {i} (0x{offset:X})"
                ))
            })?;

            // If this is the last offset, return the calculated address directly
            if i == offsets.len() - 1 {
                return Ok(pointer_target);
            }

            // Dereference pointer to next level
            let dereferenced: usize = if is_64bit {
                let ptr64: u64 = Self::read_value(pid, pointer_target)?;
                ptr64 as usize
            } else {
                let ptr32: u32 = Self::read_value(pid, pointer_target)?;
                ptr32 as usize
            };

            if dereferenced == 0 {
                return Err(NucleusError::Platform(format!(
                    "Null pointer encountered while traversing chain at level {i} (address 0x{pointer_target:X})"
                )));
            }

            current_addr = dereferenced;
        }

        Ok(current_addr)
    }

    /// Registers a memory address and value to be kept frozen. Returns unique slot ID.
    pub fn freeze_value(pid: u32, address: usize, bytes: Vec<u8>) -> u64 {
        let id = NEXT_FREEZE_ID.fetch_add(1, Ordering::Relaxed);
        let mut slots = FROZEN_SLOTS.write().unwrap();
        slots.push(FrozenMemorySlot {
            id,
            pid,
            address,
            bytes,
        });
        id
    }

    /// Unfreezes a registered memory slot by ID.
    pub fn unfreeze_value(id: u64) -> bool {
        let mut slots = FROZEN_SLOTS.write().unwrap();
        let initial_len = slots.len();
        slots.retain(|slot| slot.id != id);
        slots.len() != initial_len
    }

    /// Re-applies all active frozen slots across target processes. Called on session heartbeat ticks.
    pub fn apply_frozen_slots() {
        let slots = {
            let guard = FROZEN_SLOTS.read().unwrap();
            guard.clone()
        };

        for slot in slots {
            let _ = Self::write_bytes(slot.pid, slot.address, &slot.bytes);
        }
    }
}
