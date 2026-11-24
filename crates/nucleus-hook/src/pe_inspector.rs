//! Pure-Rust Portable Executable (PE) header inspector and architecture verification.
//!
//! Provides memory-safe parsing of PE32 (x86) and PE32+ (x64) binaries directly from
//! disk or memory buffers without calling unsafe Windows loader functions.

use nucleus_common::error::{NucleusError, NucleusResult};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Machine architecture specified in the PE header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeMachine {
    /// 32-bit Intel 386 or later (`0x014C`).
    X86,
    /// 64-bit AMD64 / x86-64 (`0x8664`).
    X64,
    /// 64-bit ARM64 (`0xAA64`).
    Arm64,
    /// Unknown or unsupported architecture.
    Unknown(u16),
}

impl From<u16> for PeMachine {
    fn from(val: u16) -> Self {
        match val {
            0x014C => Self::X86,
            0x8664 => Self::X64,
            0xAA64 => Self::Arm64,
            other => Self::Unknown(other),
        }
    }
}

/// Information extracted from a validated Portable Executable (PE) binary.
#[derive(Debug, Clone, PartialEq)]
pub struct PeInfo {
    /// Target machine architecture.
    pub machine: PeMachine,
    /// Whether the binary is a 64-bit PE32+ image.
    pub is_64bit: bool,
    /// Whether the binary has the `IMAGE_FILE_DLL` flag set.
    pub is_dll: bool,
    /// Relative virtual address (RVA) of the entry point (`DllMain` or main).
    pub entry_point_rva: u32,
    /// Preferred base address of the image in virtual memory.
    pub image_base: u64,
    /// Exported symbol names mapped to their relative virtual addresses (RVAs).
    pub exports: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
struct PeSection {
    virtual_address: u32,
    virtual_size: u32,
    raw_data_offset: u32,
    raw_data_size: u32,
}

impl PeSection {
    fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        let size = self.virtual_size.max(self.raw_data_size);
        if rva >= self.virtual_address && rva < self.virtual_address.saturating_add(size) {
            let delta = rva - self.virtual_address;
            Some((self.raw_data_offset as usize).saturating_add(delta as usize))
        } else {
            None
        }
    }
}

/// Pure-Rust Portable Executable (PE) header parser.
pub struct PeInspector;

impl PeInspector {
    /// Inspects a PE binary file on disk.
    pub fn inspect_file(path: &Path) -> NucleusResult<PeInfo> {
        let mut file = File::open(path)
            .map_err(|e| NucleusError::Platform(format!("Failed to open DLL {:?}: {}", path, e)))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| NucleusError::Platform(format!("Failed to read DLL {:?}: {}", path, e)))?;
        Self::inspect_bytes(&buffer)
    }

    /// Inspects a PE binary from an in-memory byte slice.
    pub fn inspect_bytes(data: &[u8]) -> NucleusResult<PeInfo> {
        if data.len() < 64 {
            return Err(NucleusError::PeInspection(
                "Buffer too small for DOS header".to_string(),
            ));
        }

        // Verify DOS signature ("MZ" / 0x5A4D)
        if data[0] != b'M' || data[1] != b'Z' {
            return Err(NucleusError::PeInspection(
                "Invalid DOS signature (expected MZ)".to_string(),
            ));
        }

        // Read e_lfanew at offset 0x3C (4 bytes, little-endian)
        let lfanew = u32::from_le_bytes(
            data[0x3C..0x40]
                .try_into()
                .map_err(|_| NucleusError::PeInspection("Failed to read e_lfanew".to_string()))?,
        ) as usize;

        if lfanew + 24 > data.len() {
            return Err(NucleusError::PeInspection(
                "Corrupt PE offset (exceeds file size)".to_string(),
            ));
        }

        // Verify PE signature ("PE\0\0" / 0x00004550)
        let pe_sig = &data[lfanew..lfanew + 4];
        if pe_sig != b"PE\0\0" {
            return Err(NucleusError::PeInspection(
                "Invalid PE signature".to_string(),
            ));
        }

        let file_header_offset = lfanew + 4;
        let machine_raw = u16::from_le_bytes(
            data[file_header_offset..file_header_offset + 2]
                .try_into()
                .unwrap(),
        );
        let num_sections = u16::from_le_bytes(
            data[file_header_offset + 2..file_header_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let opt_header_size = u16::from_le_bytes(
            data[file_header_offset + 16..file_header_offset + 18]
                .try_into()
                .unwrap(),
        ) as usize;
        let characteristics = u16::from_le_bytes(
            data[file_header_offset + 18..file_header_offset + 20]
                .try_into()
                .unwrap(),
        );

        let is_dll = (characteristics & 0x2000) != 0;
        let machine = PeMachine::from(machine_raw);

        let opt_header_offset = file_header_offset + 20;
        if opt_header_offset + opt_header_size > data.len() {
            return Err(NucleusError::PeInspection(
                "Corrupt optional header size".to_string(),
            ));
        }

        let opt_magic = u16::from_le_bytes(
            data[opt_header_offset..opt_header_offset + 2]
                .try_into()
                .unwrap(),
        );

        let is_64bit = match opt_magic {
            0x010B => false, // PE32
            0x020B => true,  // PE32+
            other => {
                return Err(NucleusError::PeInspection(format!(
                    "Unsupported optional header magic: 0x{:04X}",
                    other
                )));
            }
        };

        let entry_point_rva = u32::from_le_bytes(
            data[opt_header_offset + 16..opt_header_offset + 20]
                .try_into()
                .unwrap(),
        );

        let (image_base, export_dir_rva, export_dir_size) = if is_64bit {
            let base = u64::from_le_bytes(
                data[opt_header_offset + 24..opt_header_offset + 32]
                    .try_into()
                    .unwrap(),
            );
            // Export table is Data Directory entry 0 at offset 112
            let (exp_rva, exp_size) = if opt_header_size >= 120 {
                let rva = u32::from_le_bytes(
                    data[opt_header_offset + 112..opt_header_offset + 116]
                        .try_into()
                        .unwrap(),
                );
                let sz = u32::from_le_bytes(
                    data[opt_header_offset + 116..opt_header_offset + 120]
                        .try_into()
                        .unwrap(),
                );
                (rva, sz)
            } else {
                (0, 0)
            };
            (base, exp_rva, exp_size)
        } else {
            let base = u32::from_le_bytes(
                data[opt_header_offset + 28..opt_header_offset + 32]
                    .try_into()
                    .unwrap(),
            ) as u64;
            // Export table is Data Directory entry 0 at offset 96
            let (exp_rva, exp_size) = if opt_header_size >= 104 {
                let rva = u32::from_le_bytes(
                    data[opt_header_offset + 96..opt_header_offset + 100]
                        .try_into()
                        .unwrap(),
                );
                let sz = u32::from_le_bytes(
                    data[opt_header_offset + 100..opt_header_offset + 104]
                        .try_into()
                        .unwrap(),
                );
                (rva, sz)
            } else {
                (0, 0)
            };
            (base, exp_rva, exp_size)
        };

        // Parse section table
        let section_table_offset = opt_header_offset + opt_header_size;
        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let sec_offset = section_table_offset + i * 40;
            if sec_offset + 40 > data.len() {
                break;
            }
            let virtual_size =
                u32::from_le_bytes(data[sec_offset + 8..sec_offset + 12].try_into().unwrap());
            let virtual_address =
                u32::from_le_bytes(data[sec_offset + 12..sec_offset + 16].try_into().unwrap());
            let raw_data_size =
                u32::from_le_bytes(data[sec_offset + 16..sec_offset + 20].try_into().unwrap());
            let raw_data_offset =
                u32::from_le_bytes(data[sec_offset + 20..sec_offset + 24].try_into().unwrap());
            sections.push(PeSection {
                virtual_address,
                virtual_size,
                raw_data_offset,
                raw_data_size,
            });
        }

        let rva_to_offset = |rva: u32| -> Option<usize> {
            for sec in &sections {
                if let Some(off) = sec.rva_to_offset(rva) {
                    return Some(off);
                }
            }
            None
        };

        // Parse exports if export table exists
        let mut exports = HashMap::new();
        if export_dir_rva != 0 && export_dir_size > 0 {
            if let Some(exp_off) = rva_to_offset(export_dir_rva) {
                if exp_off + 40 <= data.len() {
                    let num_functions =
                        u32::from_le_bytes(data[exp_off + 20..exp_off + 24].try_into().unwrap())
                            as usize;
                    let num_names =
                        u32::from_le_bytes(data[exp_off + 24..exp_off + 28].try_into().unwrap())
                            as usize;
                    let addr_functions_rva =
                        u32::from_le_bytes(data[exp_off + 28..exp_off + 32].try_into().unwrap());
                    let addr_names_rva =
                        u32::from_le_bytes(data[exp_off + 32..exp_off + 36].try_into().unwrap());
                    let addr_name_ordinals_rva =
                        u32::from_le_bytes(data[exp_off + 36..exp_off + 40].try_into().unwrap());

                    if let (Some(names_off), Some(ordinals_off), Some(funcs_off)) = (
                        rva_to_offset(addr_names_rva),
                        rva_to_offset(addr_name_ordinals_rva),
                        rva_to_offset(addr_functions_rva),
                    ) {
                        for i in 0..num_names {
                            let name_ptr_off = names_off + i * 4;
                            let ord_ptr_off = ordinals_off + i * 2;
                            if name_ptr_off + 4 > data.len() || ord_ptr_off + 2 > data.len() {
                                break;
                            }
                            let name_rva = u32::from_le_bytes(
                                data[name_ptr_off..name_ptr_off + 4].try_into().unwrap(),
                            );
                            let ordinal = u16::from_le_bytes(
                                data[ord_ptr_off..ord_ptr_off + 2].try_into().unwrap(),
                            ) as usize;

                            if ordinal < num_functions {
                                let func_ptr_off = funcs_off + ordinal * 4;
                                if func_ptr_off + 4 <= data.len() {
                                    let func_rva = u32::from_le_bytes(
                                        data[func_ptr_off..func_ptr_off + 4].try_into().unwrap(),
                                    );
                                    if let Some(str_off) = rva_to_offset(name_rva) {
                                        let mut end = str_off;
                                        while end < data.len() && data[end] != 0 {
                                            end += 1;
                                        }
                                        if let Ok(name_str) =
                                            std::str::from_utf8(&data[str_off..end])
                                        {
                                            exports.insert(name_str.to_string(), func_rva);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(PeInfo {
            machine,
            is_64bit,
            is_dll,
            entry_point_rva,
            image_base,
            exports,
        })
    }

    /// Pre-inspects the DLL and target process to guarantee architecture compatibility.
    pub fn verify_compatibility(dll_path: &Path, target_pid: u32) -> NucleusResult<PeInfo> {
        let pe_info = Self::inspect_file(dll_path)?;

        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{CloseHandle, HANDLE};
            use windows::Win32::System::Threading::{
                IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            };

            unsafe {
                let process: HANDLE = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    target_pid,
                )
                .map_err(|e| {
                    NucleusError::Platform(format!("Failed to open PID {}: {}", target_pid, e))
                })?;

                let mut is_wow64 = windows::Win32::Foundation::BOOL(0);
                let check_res = IsWow64Process(process, &mut is_wow64);
                let _ = CloseHandle(process);

                if check_res.is_err() {
                    return Err(NucleusError::Platform(format!(
                        "IsWow64Process query failed for PID {}",
                        target_pid
                    )));
                }

                let target_is_32bit = is_wow64.as_bool();
                if target_is_32bit && pe_info.is_64bit {
                    return Err(NucleusError::PeInspection(format!(
                        "Architecture mismatch: DLL {:?} is 64-bit AMD64, but target process PID {} is 32-bit (WOW64)",
                        dll_path, target_pid
                    )));
                }

                if !target_is_32bit && !pe_info.is_64bit {
                    return Err(NucleusError::PeInspection(format!(
                        "Architecture mismatch: DLL {:?} is 32-bit i386, but target process PID {} is native 64-bit AMD64",
                        dll_path, target_pid
                    )));
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = target_pid;
        }

        Ok(pe_info)
    }
}
