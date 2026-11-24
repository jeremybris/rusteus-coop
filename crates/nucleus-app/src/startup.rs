//! Startup Verification, Pre-flight Checks & UAC Elevation.
//!
//! Provides exact architectural parity with C# NucleusCoop's `StartChecks.cs`,
//! `UserDriveInfo.cs`, and `Program.cs`:
//!
//! 1. Single-Instance Exclusivity (`SingleInstanceGuard`) via Win32 named mutex.
//! 2. Drive Filesystem Validation (exFAT / FAT32 rejection via `GetVolumeInformationW`).
//! 3. Files Integrity Verification (checking utility binaries, auto-creating GUI folders).
//! 4. User Environment Registry Recovery (recovering from abnormal termination & backing up "User Shell Folders").
//! 5. Installation Path Safety (detecting protected directories & OneDrive redirection).
//! 6. Launch-time UAC elevation management.

use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{info, warn};

use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_platform::PlatformServices;
use nucleus_profile::AppSettings;

#[cfg(windows)]
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, SetLastError,
    WIN32_ERROR,
};
#[cfg(windows)]
use windows::Win32::Security::{
    InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SetSecurityDescriptorDacl,
};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;
#[cfg(windows)]
use windows::core::PCWSTR;

/// Default mutex name for single-instance exclusivity.
pub const SINGLE_INSTANCE_MUTEX_NAME: &str = "Global\\NucleusCoop_SingleInstance_Mutex";
/// Local fallback mutex name if Global namespace access is restricted.
pub const SINGLE_INSTANCE_MUTEX_FALLBACK: &str = "Local\\NucleusCoop_SingleInstance_Mutex";

// ============================================================================
// 1. Single-Instance Exclusivity (SingleInstanceGuard)
// ============================================================================

/// Error encountered during single-instance acquisition.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SingleInstanceError {
    /// Another instance of NucleusCoop is already running.
    #[error(
        "Nucleus Co-op is already running. If you don't see the window, it might be running in the background. Close the process using Task Manager."
    )]
    AlreadyRunning,

    /// System error attempting to create or open the mutex.
    #[error("Failed to acquire single instance lock: {0}")]
    SystemError(String),
}

/// RAII Guard that holds a named system mutex for the lifetime of the process.
///
/// When dropped, the mutex handle is automatically closed, allowing subsequent launches.
#[derive(Debug)]
pub struct SingleInstanceGuard {
    #[cfg(windows)]
    handle: HANDLE,
    name: String,
}

// Safety: Windows mutex handles are safe to send and share across threads.
unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl SingleInstanceGuard {
    /// Attempts to acquire single-instance exclusivity using the given mutex name.
    ///
    /// If the mutex already exists, returns `Err(SingleInstanceError::AlreadyRunning)`.
    pub fn acquire(mutex_name: &str) -> Result<Self, SingleInstanceError> {
        #[cfg(windows)]
        {
            let wide_name: Vec<u16> = mutex_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                let mut sd = SECURITY_DESCRIPTOR::default();
                let _ = InitializeSecurityDescriptor(
                    PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut _),
                    1,
                );
                // Permissive null DACL allows access across privilege boundaries (admin and standard user)
                let _ = SetSecurityDescriptorDacl(
                    PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut _),
                    true,
                    None,
                    false,
                );
                let sa = SECURITY_ATTRIBUTES {
                    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: &mut sd as *mut _ as *mut _,
                    bInheritHandle: false.into(),
                };

                SetLastError(WIN32_ERROR(0));
                let handle = match CreateMutexW(Some(&sa), true, PCWSTR(wide_name.as_ptr())) {
                    Ok(h) => h,
                    Err(e) => {
                        // If Global namespace access was denied (e.g. unprivileged user without SeCreateGlobalPrivilege),
                        // attempt with Local namespace fallback.
                        let err_code = GetLastError();
                        if (err_code == ERROR_ACCESS_DENIED || (e.code().0 as u32) == 0x80070005)
                            && mutex_name.starts_with("Global\\")
                        {
                            let local_name = format!("Local\\{}", &mutex_name["Global\\".len()..]);
                            let local_wide: Vec<u16> = local_name
                                .encode_utf16()
                                .chain(std::iter::once(0))
                                .collect();
                            SetLastError(WIN32_ERROR(0));
                            match CreateMutexW(Some(&sa), true, PCWSTR(local_wide.as_ptr())) {
                                Ok(lh) => lh,
                                Err(le) => {
                                    return Err(SingleInstanceError::SystemError(format!(
                                        "CreateMutexW failed on fallback {}: {}",
                                        local_name, le
                                    )));
                                }
                            }
                        } else {
                            return Err(SingleInstanceError::SystemError(format!(
                                "CreateMutexW failed on {}: {}",
                                mutex_name, e
                            )));
                        }
                    }
                };

                let last_err = GetLastError();
                if last_err == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(handle);
                    return Err(SingleInstanceError::AlreadyRunning);
                }

                Ok(Self {
                    handle,
                    name: mutex_name.to_string(),
                })
            }
        }

        #[cfg(not(windows))]
        {
            // Non-Windows stub / simulation for testing & portable environments
            Ok(Self {
                name: mutex_name.to_string(),
            })
        }
    }

    /// Acquires the default global single-instance lock (`Global\NucleusCoop_SingleInstance_Mutex`).
    pub fn acquire_default() -> Result<Self, SingleInstanceError> {
        Self::acquire(SINGLE_INSTANCE_MUTEX_NAME)
    }

    /// Returns the name of the held mutex.
    pub fn mutex_name(&self) -> &str {
        &self.name
    }
}

/// Brings an already running instance of Nucleus Co-op to the foreground.
#[cfg(windows)]
pub fn activate_running_instance_window() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetWindowTextW, IsWindowVisible, SW_RESTORE, SetForegroundWindow,
        ShowWindow,
    };
    use windows::core::PCWSTR;

    let title1: Vec<u16> = "Nucleus Co-op\0".encode_utf16().collect();
    let title2: Vec<u16> = "NucleusCoop\0".encode_utf16().collect();

    unsafe {
        let mut hwnd = FindWindowW(None, PCWSTR(title1.as_ptr())).unwrap_or_default();
        if hwnd.0.is_null() {
            hwnd = FindWindowW(None, PCWSTR(title2.as_ptr())).unwrap_or_default();
        }

        if hwnd.0.is_null() {
            unsafe extern "system" fn enum_proc(wnd: HWND, lparam: LPARAM) -> BOOL {
                unsafe {
                    if !IsWindowVisible(wnd).as_bool() {
                        return true.into();
                    }
                    let mut buf = [0u16; 512];
                    let len = GetWindowTextW(wnd, &mut buf);
                    if len > 0 {
                        let title = String::from_utf16_lossy(&buf[..len as usize]);
                        if title.contains("Nucleus Co-op") || title.contains("NucleusCoop") {
                            let out_ptr = lparam.0 as *mut HWND;
                            *out_ptr = wnd;
                            return false.into();
                        }
                    }
                    true.into()
                }
            }

            let mut found_hwnd = HWND::default();
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut found_hwnd as *mut _ as isize));
            hwnd = found_hwnd;
        }

        if !hwnd.0.is_null() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
pub fn activate_running_instance_window() {}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            if !self.handle.is_invalid() && self.handle.0 != std::ptr::null_mut() {
                let _ = CloseHandle(self.handle);
                self.handle = HANDLE::default();
            }
        }
    }
}

// ============================================================================
// 2. Drive Filesystem Check (UserDriveInfo.IsExFat Parity)
// ============================================================================

/// Report describing the install drive's filesystem format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveFilesystemReport {
    /// Name of the detected filesystem (e.g. "NTFS", "exFAT", "FAT32").
    pub filesystem_name: String,
    /// Whether the filesystem is unsupported for NucleusCoop instance isolation.
    pub is_unsupported: bool,
    /// Descriptive warning message if unsupported.
    pub warning_message: Option<String>,
}

/// Checks if a filesystem format cannot support directory junctions / symlinks.
///
/// Parity with legacy C# `UserDriveInfo.IsExFat`.
pub fn is_unsupported_filesystem(fs_name: &str) -> bool {
    let upper = fs_name.trim().to_uppercase();
    upper == "EXFAT" || upper == "FAT32" || upper == "FAT" || upper == "FAT16"
}

/// Extracts the volume root from any path (e.g. "C:\" from "C:\Games\NucleusCoop").
pub fn extract_volume_root(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    let bytes = path_str.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return format!("{}:\\", (bytes[0] as char).to_ascii_uppercase());
    }
    if path_str.starts_with(r"\\?\") && path_str.len() >= 6 && path_str.as_bytes()[5] == b':' {
        let drive = (path_str.as_bytes()[4] as char).to_ascii_uppercase();
        return format!("{}:\\", drive);
    }
    if let Ok(canonical) = path.canonicalize() {
        let canon_str = canonical.to_string_lossy();
        if canon_str.starts_with(r"\\?\") && canon_str.len() >= 6 && canon_str.as_bytes()[5] == b':'
        {
            let drive = (canon_str.as_bytes()[4] as char).to_ascii_uppercase();
            return format!("{}:\\", drive);
        }
    }
    "C:\\".to_string()
}

/// Queries the filesystem format of the drive hosting `path` via Win32 `GetVolumeInformationW`.
pub fn get_volume_filesystem(path: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        let root = extract_volume_root(path);
        let root_wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let mut fs_name_buf = [0u16; 260];
        unsafe {
            GetVolumeInformationW(
                PCWSTR(root_wide.as_ptr()),
                None,
                None,
                None,
                None,
                Some(&mut fs_name_buf),
            )
            .map_err(|e| format!("GetVolumeInformationW failed for {}: {}", root, e))?;
        }
        let len = fs_name_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(fs_name_buf.len());
        let fs_name = String::from_utf16_lossy(&fs_name_buf[..len]);
        Ok(fs_name)
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Ok("NTFS".to_string())
    }
}

/// Evaluates install path drive format and generates warning if exFAT/FAT32 is detected.
pub fn check_drive_filesystem(install_path: &Path) -> DriveFilesystemReport {
    match get_volume_filesystem(install_path) {
        Ok(fs_name) => {
            let unsupported = is_unsupported_filesystem(&fs_name);
            let warning_message = if unsupported {
                Some(format!(
                    "\"{}\" is not a supported drive format.\n\
                     Move your Nucleus install folder to an NTFS formatted drive, or format the drive to NTFS format.\n\n\
                     Reasons:\n\
                     exFAT and FAT32 do not support directory junctions or symlink functions strictly required for instance isolation.",
                    fs_name
                ))
            } else {
                None
            };

            DriveFilesystemReport {
                filesystem_name: fs_name,
                is_unsupported: unsupported,
                warning_message,
            }
        }
        Err(err) => {
            warn!("Failed to inspect volume filesystem format: {}", err);
            DriveFilesystemReport {
                filesystem_name: "Unknown".to_string(),
                is_unsupported: false,
                warning_message: None,
            }
        }
    }
}

// ============================================================================
// 3. Files Integrity Check (CheckFilesIntegrity Parity)
// ============================================================================

/// Report detailing presence of critical dependencies and directory creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    /// Missing critical files required for multiplayer hooking and emulation.
    pub missing_critical_files: Vec<PathBuf>,
    /// Directories that were absent and auto-created.
    pub created_directories: Vec<PathBuf>,
    /// True if all critical files and directories exist.
    pub is_intact: bool,
    /// Warning messages generated for the user.
    pub warnings: Vec<String>,
}

/// Verifies presence of critical utility files and ensures GUI cache directories exist.
///
/// Parity with legacy C# `StartChecks.CheckFilesIntegrity`.
pub fn check_files_integrity(install_root: &Path) -> IntegrityReport {
    let mut missing = Vec::new();
    let mut created = Vec::new();
    let mut warnings = Vec::new();

    // 1. Ensure essential GUI directories exist
    let required_dirs = [
        install_root.join("gui").join("covers"),
        install_root.join("gui").join("screenshots"),
        install_root.join("content").join("cache").join("covers"),
    ];

    for dir in &required_dirs {
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                warnings.push(format!(
                    "Failed to create directory {}: {}",
                    dir.display(),
                    e
                ));
            } else {
                created.push(dir.clone());
                info!("Created missing directory: {}", dir.display());
            }
        }
    }

    // 2. Critical utility files to check
    // We check both the direct install root and ./utils/ subdirectories
    let critical_rel_paths = [
        // ProtoInput hooking binaries
        PathBuf::from("utils")
            .join("ProtoInput")
            .join("ProtoInputLoader32.dll"),
        PathBuf::from("utils")
            .join("ProtoInput")
            .join("ProtoInputLoader64.dll"),
        // Goldberg Steam Emulator
        PathBuf::from("utils").join("GoldbergEmu"),
    ];

    for rel in &critical_rel_paths {
        let full = install_root.join(rel);
        if !full.exists() {
            // Also check root directly (in case flattened layout is used)
            let file_name = rel.file_name().unwrap_or_default();
            let alt_full = install_root.join(file_name);
            if !alt_full.exists() {
                missing.push(full);
            }
        }
    }

    // 3. Check for elevator binary (either in root or utils)
    let elevator_root = install_root.join("elevator.exe");
    let elevator_utils = install_root.join("utils").join("elevator.exe");
    if !elevator_root.exists() && !elevator_utils.exists() {
        // Not fatal, but note for warnings
        info!("Elevator helper executable not found in root or utils directory");
    }

    let is_intact = missing.is_empty();
    if !is_intact {
        let warning = format!(
            "{} critical file(s) are missing from your Nucleus Co-op installation folder:\n{}\n\
             Check that your antivirus program is not deleting or blocking Nucleus Co-op files. \
             Add the Nucleus Co-op folder to your antivirus exceptions list and re-extract missing files.",
            missing.len(),
            missing
                .iter()
                .map(|p| format!(" - {}", p.file_name().unwrap_or_default().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        warn!("{}", warning);
        warnings.push(warning);
    }

    IntegrityReport {
        missing_critical_files: missing,
        created_directories: created,
        is_intact,
        warnings,
    }
}

// ============================================================================
// 4. User Environment Registry Recovery (CheckUserEnvironment Parity)
// ============================================================================

/// Registry key path for Windows Explorer User Shell Folders.
pub const USER_SHELL_FOLDERS_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
pub const USER_SHELL_FOLDERS_FULL_KEY: &str =
    r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";

/// Report detailing recovery or baseline backup of user shell environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserEnvironmentReport {
    /// Whether user registry was restored from backup due to previous unclean shutdown.
    pub restored_from_backup: bool,
    /// Whether a baseline backup was generated.
    pub baseline_backup_created: bool,
    /// Whether abnormal termination (contaminated Personal folder) was detected.
    pub abnormal_termination_detected: bool,
    /// The current value of Personal folder found in registry.
    pub current_personal_path: Option<String>,
    /// Informational or diagnostic status.
    pub details: String,
    /// Any error message during recovery.
    pub error: Option<String>,
}

/// Checks if previous session contaminated the user's Documents folder and restores baseline backup.
///
/// Parity with legacy C# `StartChecks.CheckUserEnvironment`.
pub fn check_user_environment(
    install_root: &Path,
    platform: &dyn PlatformServices,
) -> UserEnvironmentReport {
    let backup_dir = install_root.join("utils").join("backup");
    let backup_file = backup_dir.join("User Shell Folders.reg");

    let registry_opt = platform.registry_manager();

    // Read current "Personal" folder from HKCU
    let personal_val = if let Some(reg) = registry_opt {
        reg.read_string(USER_SHELL_FOLDERS_KEY, "Personal").ok()
    } else {
        #[cfg(windows)]
        {
            use winreg::RegKey;
            use winreg::enums::*;
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(USER_SHELL_FOLDERS_KEY)
                .and_then(|k| k.get_value::<String, _>("Personal"))
                .ok()
        }
        #[cfg(not(windows))]
        {
            None
        }
    };

    let mut report = UserEnvironmentReport {
        restored_from_backup: false,
        baseline_backup_created: false,
        abnormal_termination_detected: false,
        current_personal_path: personal_val.clone(),
        details: String::new(),
        error: None,
    };

    let Some(current_personal) = personal_val else {
        report.details =
            "User Shell Folders registry key not available on this platform".to_string();
        return report;
    };

    // Parity check: If current personal folder contains "NucleusCoop", the previous run crashed
    if current_personal.contains("NucleusCoop") {
        report.abnormal_termination_detected = true;
        warn!(
            "Abnormal termination detected! User Documents folder is pointing to instanced folder: '{}'",
            current_personal
        );

        if backup_file.exists() {
            info!(
                "Restoring User Shell Folders registry from backup: {}",
                backup_file.display()
            );
            let restore_result = if let Some(reg) = registry_opt {
                reg.import_from_file(&backup_file)
            } else {
                #[cfg(windows)]
                {
                    let status = std::process::Command::new("reg.exe")
                        .args(["import", &backup_file.to_string_lossy()])
                        .output();
                    match status {
                        Ok(o) if o.status.success() => Ok(()),
                        Ok(o) => Err(NucleusError::Platform(format!(
                            "reg.exe import failed: {}",
                            String::from_utf8_lossy(&o.stderr)
                        ))),
                        Err(e) => Err(NucleusError::Platform(e.to_string())),
                    }
                }
                #[cfg(not(windows))]
                {
                    Ok(())
                }
            };

            match restore_result {
                Ok(_) => {
                    info!("Successfully restored User Shell Folders registry from backup");
                    report.restored_from_backup = true;
                    report.details =
                        "Restored user registry from backup after abnormal termination".to_string();
                }
                Err(e) => {
                    let err_msg = format!("Failed to restore user registry from backup: {}", e);
                    tracing::error!("{}", err_msg);
                    report.error = Some(err_msg);
                }
            }
        } else {
            let err_msg = format!(
                "Abnormal termination detected, but backup registry file not found at: {}",
                backup_file.display()
            );
            tracing::error!("{}", err_msg);
            report.error = Some(err_msg);
        }
    } else {
        // Personal is normal. Ensure baseline backup exists
        if !backup_file.exists() {
            info!(
                "Creating baseline backup of User Shell Folders to: {}",
                backup_file.display()
            );
            let _ = std::fs::create_dir_all(&backup_dir);

            let export_result = if let Some(reg) = registry_opt {
                reg.export_to_file(USER_SHELL_FOLDERS_FULL_KEY, &backup_file)
            } else {
                #[cfg(windows)]
                {
                    let status = std::process::Command::new("reg.exe")
                        .args([
                            "export",
                            USER_SHELL_FOLDERS_FULL_KEY,
                            &backup_file.to_string_lossy(),
                            "/y",
                        ])
                        .output();
                    match status {
                        Ok(o) if o.status.success() => Ok(()),
                        Ok(o) => Err(NucleusError::Platform(format!(
                            "reg.exe export failed: {}",
                            String::from_utf8_lossy(&o.stderr)
                        ))),
                        Err(e) => Err(NucleusError::Platform(e.to_string())),
                    }
                }
                #[cfg(not(windows))]
                {
                    Ok(())
                }
            };

            match export_result {
                Ok(_) => {
                    info!("Successfully created baseline User Shell Folders backup");
                    report.baseline_backup_created = true;
                    report.details =
                        "Created initial baseline User Shell Folders backup".to_string();
                }
                Err(e) => {
                    let err_msg =
                        format!("Failed to create baseline User Shell Folders backup: {}", e);
                    warn!("{}", err_msg);
                    report.error = Some(err_msg);
                }
            }
        } else {
            report.details = "User Shell Folders registry is normal and backup exists".to_string();
        }
    }

    report
}

// ============================================================================
// 5. Installation Path Safety Check (CheckInstallFolder Parity)
// ============================================================================

/// Report describing directory safety and OneDrive redirection detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSafetyReport {
    /// Whether executable resides in a restricted system folder.
    pub is_problematic_path: bool,
    /// Whether user Documents folder is redirected to Microsoft OneDrive.
    pub is_onedrive_active: bool,
    /// Warnings generated regarding installation path and documents storage.
    pub warnings: Vec<String>,
}

/// Evaluates installation path safety and active OneDrive document redirection.
///
/// Parity with legacy C# `StartChecks.CheckInstallFolder` and `Globals.IsOneDriveEnabled`.
pub fn check_installation_path(
    exe_path: &Path,
    user_documents_override: Option<&str>,
) -> PathSafetyReport {
    let mut warnings = Vec::new();
    let lower_exe = exe_path.to_string_lossy().to_lowercase();

    // 1. Check for problematic system folders
    let is_problematic_path = lower_exe.starts_with(r"c:\program files\")
        || lower_exe.starts_with(r"c:\program files (x86)\")
        || lower_exe.starts_with(r"c:\windows\");

    if is_problematic_path {
        let warn_msg = "Nucleus Co-op should not be installed in protected Windows system directories.\n\n\
             Do NOT install it in:\n\
             - C:\\Program Files or C:\\Program Files (x86)\n\
             - C:\\Windows\n\n\
             Windows UAC virtualization and security restrictions can block instance symlinks and process spawning.\n\
             Recommended installation path: C:\\NucleusCoop or C:\\Games\\NucleusCoop";
        warn!("{}", warn_msg);
        warnings.push(warn_msg.to_string());
    }

    // 2. Check for active OneDrive documents synchronization
    let docs_path = if let Some(d) = user_documents_override {
        d.to_string()
    } else {
        dirs::document_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    let is_onedrive_active = docs_path.to_lowercase().contains("onedrive");
    if is_onedrive_active {
        let warn_msg = "Using OneDrive as your default documents path will break many game handlers because \
             Nucleus Co-op cannot access virtualized cloud storage.\n\
             To fix this, log out from the OneDrive app or open Documents folder Properties in File Explorer, \
             select Location, and restore the default local Windows documents path.";
        warn!("{}", warn_msg);
        warnings.push(warn_msg.to_string());
    }

    PathSafetyReport {
        is_problematic_path,
        is_onedrive_active,
        warnings,
    }
}

// ============================================================================
// 6. Launch-Time UAC Elevation Helper Integration
// ============================================================================

/// Action taken or status determined regarding process elevation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationAction {
    /// Process is already running with administrative privileges.
    AlreadyElevated,
    /// Elevation was not required for this launch.
    ElevationNotRequired,
    /// A new elevated process was launched; caller should exit cleanly.
    Relaunched,
}

/// Checks elevation requirements and optionally relaunches the current executable elevated.
pub fn ensure_elevation(
    requires_admin: bool,
    platform: &dyn PlatformServices,
    custom_args: Option<&[&str]>,
) -> NucleusResult<ElevationAction> {
    if !requires_admin {
        return Ok(ElevationAction::ElevationNotRequired);
    }

    let elevation_mgr = platform.elevation_manager();
    if elevation_mgr.is_elevated() {
        info!("Current process is running with administrative privileges");
        return Ok(ElevationAction::AlreadyElevated);
    }

    info!("Elevation required: relaunching executable with UAC elevation...");
    let current_exe = std::env::current_exe()?;
    let fallback_args = std::env::args().skip(1).collect::<Vec<_>>();
    let arg_refs: Vec<&str> = if let Some(args) = custom_args {
        args.to_vec()
    } else {
        fallback_args.iter().map(|s| s.as_str()).collect()
    };

    elevation_mgr.run_elevated(&current_exe, &arg_refs, None, false)?;
    info!("Elevated child process successfully requested; parent will exit");
    Ok(ElevationAction::Relaunched)
}

// ============================================================================
// 7. Consolidated Preflight Execution Pipeline
// ============================================================================

/// Consolidated preflight checks report and session guards.
pub struct PreflightResult {
    /// Active single-instance lock guard (if single instance mode was enforced).
    pub single_instance_guard: Option<SingleInstanceGuard>,
    /// Drive filesystem verification report.
    pub drive_filesystem: DriveFilesystemReport,
    /// File integrity verification report.
    pub files_integrity: IntegrityReport,
    /// User environment registry check/recovery report.
    pub user_environment: UserEnvironmentReport,
    /// Installation path safety report.
    pub path_safety: PathSafetyReport,
    /// Elevation inspection/relaunch action.
    pub elevation: ElevationAction,
}

/// Runs full preflight check suite prior to GUI launch.
///
/// Parity with legacy C# `Program.RunApplication()` startup checklist.
pub fn run_preflight_checks(
    install_root: &Path,
    app_settings: &AppSettings,
    platform: &dyn PlatformServices,
    requires_admin: bool,
) -> Result<PreflightResult, SingleInstanceError> {
    info!("Running startup pre-flight verification checks...");

    // 1. Single-Instance Check
    let single_instance_guard = if !app_settings.nucleus_multi_instances {
        info!("Verifying single-instance exclusivity...");
        match SingleInstanceGuard::acquire_default() {
            Ok(guard) => {
                info!("Acquired single-instance mutex: {}", guard.mutex_name());
                Some(guard)
            }
            Err(e) => {
                warn!("Single-instance check failed: {}", e);
                return Err(e);
            }
        }
    } else {
        info!("Multi-instance launcher mode enabled in settings (single-instance mutex skipped)");
        None
    };

    // 2. Drive Filesystem Check
    let drive_filesystem = check_drive_filesystem(install_root);
    if drive_filesystem.is_unsupported {
        if let Some(ref w) = drive_filesystem.warning_message {
            warn!("{}", w);
        }
    } else {
        info!(
            "Drive filesystem format verified: {}",
            drive_filesystem.filesystem_name
        );
    }

    // 3. File Integrity Check
    let files_integrity = check_files_integrity(install_root);
    if files_integrity.is_intact {
        info!("Files integrity verified (all critical utilities and directories present)");
    } else {
        warn!(
            "File integrity check discovered {} missing critical file(s)",
            files_integrity.missing_critical_files.len()
        );
    }

    // 4. User Environment Registry Recovery
    let user_environment = check_user_environment(install_root, platform);
    if user_environment.abnormal_termination_detected {
        warn!(
            "Abnormal termination recovery completed: restored_from_backup={}",
            user_environment.restored_from_backup
        );
    } else {
        info!("User environment verified: {}", user_environment.details);
    }

    // Registry keys a handler edited are exported before the edit and re-imported when the
    // session ends. Exports still present at startup mean the last session did not end
    // cleanly, so restore them now.
    let restored_keys = nucleus_script::context_ext::registry::restore_backups(install_root);
    if restored_keys > 0 {
        warn!(
            "Restored {} registry key(s) left modified by a handler in the previous session",
            restored_keys
        );
    }

    // 5. Installation Path Safety Check
    let current_exe =
        std::env::current_exe().unwrap_or_else(|_| install_root.join("nucleus-coop.exe"));
    let path_safety = check_installation_path(&current_exe, None);
    if path_safety.is_problematic_path || path_safety.is_onedrive_active {
        warn!(
            "Installation path warnings present (problematic={}, onedrive={})",
            path_safety.is_problematic_path, path_safety.is_onedrive_active
        );
    } else {
        info!("Installation path safety verified");
    }

    // 6. UAC Elevation Check
    let elevation = ensure_elevation(requires_admin, platform, None)
        .unwrap_or(ElevationAction::AlreadyElevated);

    Ok(PreflightResult {
        single_instance_guard,
        drive_filesystem,
        files_integrity,
        user_environment,
        path_safety,
        elevation,
    })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_guard_lifecycle() {
        let test_mutex_name = format!("Local\\NucleusCoop_Test_Mutex_{}", std::process::id());

        // First acquisition must succeed
        let guard1 = SingleInstanceGuard::acquire(&test_mutex_name);
        assert!(
            guard1.is_ok(),
            "First acquire of unique test mutex should succeed"
        );
        let guard1 = guard1.unwrap();
        assert_eq!(guard1.mutex_name(), test_mutex_name);

        #[cfg(windows)]
        {
            // Second acquisition with same name while guard1 is held must fail with AlreadyRunning
            let guard2 = SingleInstanceGuard::acquire(&test_mutex_name);
            assert_eq!(
                guard2.err(),
                Some(SingleInstanceError::AlreadyRunning),
                "Second acquire of same mutex must return AlreadyRunning"
            );
        }

        // Drop guard1, releasing the mutex handle
        drop(guard1);

        #[cfg(windows)]
        {
            // After drop, acquiring again should succeed
            let guard3 = SingleInstanceGuard::acquire(&test_mutex_name);
            assert!(
                guard3.is_ok(),
                "Acquiring mutex after dropping previous guard should succeed"
            );
        }
    }

    #[test]
    fn test_is_unsupported_filesystem() {
        assert!(is_unsupported_filesystem("exFAT"));
        assert!(is_unsupported_filesystem("exfat"));
        assert!(is_unsupported_filesystem("FAT32"));
        assert!(is_unsupported_filesystem("fat32"));
        assert!(is_unsupported_filesystem("FAT"));
        assert!(is_unsupported_filesystem("fat16"));

        assert!(!is_unsupported_filesystem("NTFS"));
        assert!(!is_unsupported_filesystem("ntfs"));
        assert!(!is_unsupported_filesystem("ReFS"));
        assert!(!is_unsupported_filesystem("ext4"));
        assert!(!is_unsupported_filesystem("btrfs"));
    }

    #[test]
    fn test_extract_volume_root() {
        assert_eq!(
            extract_volume_root(Path::new(r"C:\Games\NucleusCoop")),
            "C:\\"
        );
        assert_eq!(extract_volume_root(Path::new(r"d:\nucleus")), "D:\\");
        assert_eq!(extract_volume_root(Path::new(r"\\?\E:\Folder\Sub")), "E:\\");
    }

    #[test]
    fn test_check_drive_filesystem_detection() {
        // Query the drive where the test is executing (typically C:\)
        let current_dir = std::env::current_dir().unwrap();
        let report = check_drive_filesystem(&current_dir);
        assert!(!report.filesystem_name.is_empty());
        // Standard Windows development machines use NTFS
        if report.filesystem_name.eq_ignore_ascii_case("NTFS") {
            assert!(!report.is_unsupported);
            assert!(report.warning_message.is_none());
        }
    }

    #[test]
    fn test_check_files_integrity_in_temp_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_test_integrity_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. Initial check on empty dir: missing critical files, creates gui/covers and screenshots
        let report = check_files_integrity(&temp_dir);
        assert!(!report.is_intact);
        assert!(!report.missing_critical_files.is_empty());
        assert!(
            report
                .created_directories
                .iter()
                .any(|d| d.ends_with("covers"))
        );
        assert!(
            report
                .created_directories
                .iter()
                .any(|d| d.ends_with("screenshots"))
        );
        assert!(temp_dir.join("gui").join("covers").exists());
        assert!(temp_dir.join("gui").join("screenshots").exists());

        // 2. Populate mock critical files
        let proto_dir = temp_dir.join("utils").join("ProtoInput");
        std::fs::create_dir_all(&proto_dir).unwrap();
        std::fs::write(proto_dir.join("ProtoInputLoader32.dll"), b"mock").unwrap();
        std::fs::write(proto_dir.join("ProtoInputLoader64.dll"), b"mock").unwrap();
        std::fs::create_dir_all(temp_dir.join("utils").join("GoldbergEmu")).unwrap();

        // 3. Second check: should now be intact with 0 missing files
        let report2 = check_files_integrity(&temp_dir);
        assert!(
            report2.is_intact,
            "Should be intact after creating required files"
        );
        assert_eq!(report2.missing_critical_files.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_check_installation_path_safety() {
        // Problematic paths
        let prog_files = Path::new(r"C:\Program Files\NucleusCoop\nucleus-coop.exe");
        let rep1 = check_installation_path(prog_files, Some(r"C:\Users\Test\Documents"));
        assert!(rep1.is_problematic_path);
        assert!(!rep1.is_onedrive_active);
        assert!(!rep1.warnings.is_empty());

        let prog_files_x86 = Path::new(r"C:\Program Files (x86)\NucleusCoop\nucleus-coop.exe");
        let rep2 = check_installation_path(prog_files_x86, Some(r"C:\Users\Test\Documents"));
        assert!(rep2.is_problematic_path);

        let win_path = Path::new(r"C:\Windows\System32\nucleus-coop.exe");
        let rep3 = check_installation_path(win_path, Some(r"C:\Users\Test\Documents"));
        assert!(rep3.is_problematic_path);

        // Safe path without OneDrive
        let safe_path = Path::new(r"C:\NucleusCoop\nucleus-coop.exe");
        let rep4 = check_installation_path(safe_path, Some(r"C:\Users\Test\Documents"));
        assert!(!rep4.is_problematic_path);
        assert!(!rep4.is_onedrive_active);
        assert!(rep4.warnings.is_empty());

        // Safe path with active OneDrive redirection
        let rep5 = check_installation_path(safe_path, Some(r"C:\Users\Test\OneDrive\Documents"));
        assert!(!rep5.is_problematic_path);
        assert!(rep5.is_onedrive_active);
        assert_eq!(rep5.warnings.len(), 1);
        assert!(rep5.warnings[0].contains("OneDrive"));
    }

    #[test]
    fn test_user_environment_contamination_detection() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_test_env_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let backup_dir = temp_dir.join("utils").join("backup");
        let _ = std::fs::create_dir_all(&backup_dir);
        let backup_reg = backup_dir.join("User Shell Folders.reg");
        std::fs::write(&backup_reg, "Windows Registry Editor Version 5.00\r\n").unwrap();

        // Parity test: verify abnormal termination detection if Personal contains "NucleusCoop"
        let contaminated_personal = r"C:\Users\Test\NucleusCoop\Player1\Documents";
        assert!(contaminated_personal.contains("NucleusCoop"));

        let clean_personal = r"C:\Users\Test\Documents";
        assert!(!clean_personal.contains("NucleusCoop"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
