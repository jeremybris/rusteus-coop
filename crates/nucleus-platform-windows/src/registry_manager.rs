use std::path::Path;
use winreg::RegKey;
use winreg::enums::*;

use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_common::types::{RegistryBackup, RegistryValueBackup};
use nucleus_platform::RegistryManager;

/// Windows system registry manager implementation.
#[derive(Default)]
pub struct WindowsRegistryManager;

impl WindowsRegistryManager {
    /// Creates a new `WindowsRegistryManager`.
    pub fn new() -> Self {
        Self
    }

    /// Canonical name of a hive, as accepted back by [`Self::parse_path`].
    fn hive_name(root: winreg::HKEY) -> &'static str {
        match root {
            HKEY_LOCAL_MACHINE => "HKEY_LOCAL_MACHINE",
            HKEY_CLASSES_ROOT => "HKEY_CLASSES_ROOT",
            HKEY_USERS => "HKEY_USERS",
            HKEY_CURRENT_CONFIG => "HKEY_CURRENT_CONFIG",
            _ => "HKEY_CURRENT_USER",
        }
    }

    /// Splits a registry path into its hive and sub-key.
    ///
    /// A path without a hive prefix (`Software\\Vendor\\App`) is relative to
    /// `HKEY_CURRENT_USER` and is kept whole; only a recognised hive name is stripped.
    fn parse_path(full_path: &str) -> (winreg::HKEY, &str) {
        let trimmed = full_path.trim_start_matches('\\');
        let (root, sub) = trimmed.split_once('\\').unwrap_or((trimmed, ""));
        match root.to_uppercase().as_str() {
            "HKEY_LOCAL_MACHINE" | "HKLM" => (HKEY_LOCAL_MACHINE, sub),
            "HKEY_CLASSES_ROOT" | "HKCR" => (HKEY_CLASSES_ROOT, sub),
            "HKEY_USERS" | "HKU" => (HKEY_USERS, sub),
            "HKEY_CURRENT_CONFIG" | "HKCC" => (HKEY_CURRENT_CONFIG, sub),
            "HKEY_CURRENT_USER" | "HKCU" => (HKEY_CURRENT_USER, sub),
            _ => (HKEY_CURRENT_USER, trimmed),
        }
    }
}

impl RegistryManager for WindowsRegistryManager {
    fn read_string(&self, key: &str, value_name: &str) -> NucleusResult<String> {
        let (root, sub) = Self::parse_path(key);
        let reg = RegKey::predef(root)
            .open_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        let val: String = reg
            .get_value(value_name)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(val)
    }

    fn write_string(&self, key: &str, value_name: &str, data: &str) -> NucleusResult<()> {
        let (root, sub) = Self::parse_path(key);
        let (reg, _) = RegKey::predef(root)
            .create_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        reg.set_value(value_name, &data)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(())
    }

    fn write_expand_string(&self, key: &str, value_name: &str, data: &str) -> NucleusResult<()> {
        let (root, sub) = Self::parse_path(key);
        let (reg, _) = RegKey::predef(root)
            .create_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        let mut utf16: Vec<u16> = data.encode_utf16().collect();
        utf16.push(0);
        let bytes =
            unsafe { std::slice::from_raw_parts(utf16.as_ptr() as *const u8, utf16.len() * 2) };
        let reg_val = winreg::RegValue {
            vtype: REG_EXPAND_SZ,
            bytes: bytes.to_vec(),
        };
        reg.set_raw_value(value_name, &reg_val)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(())
    }

    fn read_dword(&self, key: &str, value_name: &str) -> NucleusResult<u32> {
        let (root, sub) = Self::parse_path(key);
        let reg = RegKey::predef(root)
            .open_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        let val: u32 = reg
            .get_value(value_name)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(val)
    }

    fn write_dword(&self, key: &str, value_name: &str, data: u32) -> NucleusResult<()> {
        let (root, sub) = Self::parse_path(key);
        let (reg, _) = RegKey::predef(root)
            .create_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        reg.set_value(value_name, &data)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(())
    }

    fn delete_value(&self, key: &str, value_name: &str) -> NucleusResult<()> {
        let (root, sub) = Self::parse_path(key);
        let reg = RegKey::predef(root)
            .open_subkey_with_flags(sub, KEY_SET_VALUE)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        reg.delete_value(value_name)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(())
    }

    fn delete_key(&self, key: &str) -> NucleusResult<()> {
        let (root, sub) = Self::parse_path(key);
        let reg = RegKey::predef(root);
        reg.delete_subkey_all(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        Ok(())
    }

    fn backup_key(&self, key: &str) -> NucleusResult<RegistryBackup> {
        let (root, sub) = Self::parse_path(key);
        let reg = RegKey::predef(root)
            .open_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;

        let mut values = Vec::new();
        for val_res in reg.enum_values() {
            if let Ok((name, reg_val)) = val_res {
                values.push(RegistryValueBackup {
                    name,
                    value_type: format!("{:?}", reg_val.vtype),
                    data: reg_val.bytes,
                });
            }
        }

        Ok(RegistryBackup {
            root_key: Self::hive_name(root).to_string(),
            sub_key: sub.to_string(),
            values,
        })
    }

    fn restore_key(&self, backup: &RegistryBackup) -> NucleusResult<()> {
        let full_path = format!("{}\\{}", backup.root_key, backup.sub_key);
        let (root, sub) = Self::parse_path(&full_path);
        let (reg, _) = RegKey::predef(root)
            .create_subkey(sub)
            .map_err(|e| NucleusError::Platform(e.to_string()))?;

        for val in &backup.values {
            let vtype = match val.value_type.as_str() {
                "REG_SZ" => REG_SZ,
                "REG_EXPAND_SZ" => REG_EXPAND_SZ,
                "REG_DWORD" => REG_DWORD,
                "REG_DWORD_BIG_ENDIAN" => REG_DWORD_BIG_ENDIAN,
                "REG_MULTI_SZ" => REG_MULTI_SZ,
                "REG_QWORD" => REG_QWORD,
                "REG_NONE" => REG_NONE,
                _ => REG_BINARY,
            };
            let reg_value = winreg::RegValue {
                vtype,
                bytes: val.data.clone(),
            };
            let _ = reg.set_raw_value(&val.name, &reg_value);
        }

        Ok(())
    }

    fn export_to_file(&self, key: &str, file_path: &Path) -> NucleusResult<()> {
        if let Some(parent) = file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (root, sub) = Self::parse_path(key);
        let root_str = match root {
            HKEY_LOCAL_MACHINE => "HKLM",
            HKEY_CLASSES_ROOT => "HKCR",
            HKEY_USERS => "HKU",
            HKEY_CURRENT_CONFIG => "HKCC",
            _ => "HKCU",
        };
        let full_key = if sub.is_empty() {
            root_str.to_string()
        } else {
            format!("{}\\{}", root_str, sub)
        };

        let output = std::process::Command::new("reg.exe")
            .args(["export", &full_key, &file_path.to_string_lossy(), "/y"])
            .output()
            .map_err(|e| {
                NucleusError::Platform(format!("Failed to execute reg.exe export: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NucleusError::Platform(format!(
                "reg.exe export failed: {}",
                stderr.trim()
            )));
        }
        Ok(())
    }

    fn import_from_file(&self, file_path: &Path) -> NucleusResult<()> {
        if !file_path.exists() {
            return Err(NucleusError::Platform(format!(
                "Registry file does not exist: {:?}",
                file_path
            )));
        }
        let output = std::process::Command::new("reg.exe")
            .args(["import", &file_path.to_string_lossy()])
            .output()
            .map_err(|e| {
                NucleusError::Platform(format!("Failed to execute reg.exe import: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NucleusError::Platform(format!(
                "reg.exe import failed: {}",
                stderr.trim()
            )));
        }
        Ok(())
    }

    fn key_exists(&self, key: &str) -> bool {
        let (root, sub) = Self::parse_path(key);
        RegKey::predef(root).open_subkey(sub).is_ok()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_registry_expand_string_and_restore_type_preservation() {
        let mgr = WindowsRegistryManager::new();
        let test_key = format!(r"HKCU\Software\NucleusTest_{}", std::process::id());

        // Clean up before starting
        let _ = mgr.delete_key(&test_key);

        // Write normal string
        mgr.write_string(&test_key, "TestStr", "RegularStringValue")
            .unwrap();
        assert_eq!(
            mgr.read_string(&test_key, "TestStr").unwrap(),
            "RegularStringValue"
        );

        // Write DWORD
        mgr.write_dword(&test_key, "TestDword", 42).unwrap();
        assert_eq!(mgr.read_dword(&test_key, "TestDword").unwrap(), 42);

        // Write REG_EXPAND_SZ
        mgr.write_expand_string(&test_key, "TestExpand", "%USERPROFILE%\\Documents")
            .unwrap();
        assert_eq!(
            mgr.read_string(&test_key, "TestExpand").unwrap(),
            "%USERPROFILE%\\Documents"
        );

        // Perform in-memory backup
        let backup = mgr.backup_key(&test_key).unwrap();
        assert_eq!(backup.values.len(), 3);
        assert_eq!(backup.root_key, "HKEY_CURRENT_USER");

        let expand_entry = backup
            .values
            .iter()
            .find(|v| v.name == "TestExpand")
            .unwrap();
        assert_eq!(expand_entry.value_type, "REG_EXPAND_SZ");

        let dword_entry = backup
            .values
            .iter()
            .find(|v| v.name == "TestDword")
            .unwrap();
        assert_eq!(dword_entry.value_type, "REG_DWORD");

        let str_entry = backup.values.iter().find(|v| v.name == "TestStr").unwrap();
        assert_eq!(str_entry.value_type, "REG_SZ");

        // Modify values
        mgr.write_string(&test_key, "TestStr", "Modified").unwrap();
        mgr.write_dword(&test_key, "TestDword", 999).unwrap();

        // Restore key from backup
        mgr.restore_key(&backup).unwrap();

        // Verify values AND types were preserved
        assert_eq!(
            mgr.read_string(&test_key, "TestStr").unwrap(),
            "RegularStringValue"
        );
        assert_eq!(mgr.read_dword(&test_key, "TestDword").unwrap(), 42);
        assert_eq!(
            mgr.read_string(&test_key, "TestExpand").unwrap(),
            "%USERPROFILE%\\Documents"
        );

        // Clean up
        let _ = mgr.delete_key(&test_key);
    }

    #[test]
    fn test_registry_export_and_import() {
        let mgr = WindowsRegistryManager::new();
        let test_key = format!(r"HKCU\Software\NucleusRegExportTest_{}", std::process::id());
        let _ = mgr.delete_key(&test_key);

        mgr.write_string(&test_key, "ExportKey", "ExportVal123")
            .unwrap();

        let temp_reg =
            std::env::temp_dir().join(format!("nucleus_reg_test_{}.reg", std::process::id()));
        mgr.export_to_file(&test_key, &temp_reg).unwrap();
        assert!(temp_reg.exists(), ".reg backup file should exist on disk");

        // Delete key
        mgr.delete_key(&test_key).unwrap();
        assert!(!mgr.key_exists(&test_key));

        // Import back from file
        mgr.import_from_file(&temp_reg).unwrap();
        assert!(mgr.key_exists(&test_key));
        assert_eq!(
            mgr.read_string(&test_key, "ExportKey").unwrap(),
            "ExportVal123"
        );

        // Cleanup
        let _ = mgr.delete_key(&test_key);
        let _ = std::fs::remove_file(&temp_reg);
    }

    #[test]
    fn test_parse_path_keeps_hive_relative_paths_whole() {
        let (hive, sub) = WindowsRegistryManager::parse_path(r"Software\Microsoft\Windows");
        assert_eq!(hive, HKEY_CURRENT_USER);
        assert_eq!(sub, r"Software\Microsoft\Windows");

        let (hive, sub) = WindowsRegistryManager::parse_path(r"HKEY_CURRENT_USER\Software\X");
        assert_eq!(hive, HKEY_CURRENT_USER);
        assert_eq!(sub, r"Software\X");

        let (hive, sub) = WindowsRegistryManager::parse_path(r"HKLM\SOFTWARE\X");
        assert_eq!(hive, HKEY_LOCAL_MACHINE);
        assert_eq!(sub, r"SOFTWARE\X");

        let (hive, sub) = WindowsRegistryManager::parse_path("Software");
        assert_eq!(hive, HKEY_CURRENT_USER);
        assert_eq!(sub, "Software");
    }

    #[test]
    fn test_read_string_resolves_hive_relative_user_shell_folders() {
        // This expandable value exists for every Windows account; reading it through a
        // hive-relative path is what the startup Documents-folder recovery depends on.
        let mgr = WindowsRegistryManager::new();
        let personal = mgr
            .read_string(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders",
                "Personal",
            )
            .expect("Personal shell folder must be readable");
        assert!(!personal.is_empty());
    }
}
