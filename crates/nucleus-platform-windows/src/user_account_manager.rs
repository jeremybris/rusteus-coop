//! Windows implementation of `UserAccountManager` using Windows NetUser APIs.

use std::path::PathBuf;

use nucleus_common::error::NucleusResult;
use nucleus_common::types::{ProcessConfig, ProcessHandle};
use nucleus_platform::UserAccountManager;

/// Windows user account manager implementation.
#[derive(Default)]
pub struct WindowsUserAccountManager;

impl WindowsUserAccountManager {
    /// Creates a new `WindowsUserAccountManager`.
    pub fn new() -> Self {
        Self
    }
}

use std::os::windows::process::CommandExt;

impl UserAccountManager for WindowsUserAccountManager {
    fn create_user(&self, username: &str, password: &str) -> NucleusResult<()> {
        let status = std::process::Command::new("net")
            .args(["user", username, password, "/add"])
            .creation_flags(0x08000000)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(nucleus_common::error::NucleusError::Platform(format!(
                "Failed to create user '{}'",
                username
            )))
        }
    }

    fn delete_user(&self, username: &str) -> NucleusResult<()> {
        let status = std::process::Command::new("net")
            .args(["user", username, "/delete"])
            .creation_flags(0x08000000)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(nucleus_common::error::NucleusError::Platform(format!(
                "Failed to delete user '{}'",
                username
            )))
        }
    }

    fn user_exists(&self, username: &str) -> NucleusResult<bool> {
        let status = std::process::Command::new("net")
            .args(["user", username])
            .creation_flags(0x08000000)
            .status()?;
        Ok(status.success())
    }

    fn run_as_user(
        &self,
        username: &str,
        password: &str,
        config: &ProcessConfig,
    ) -> NucleusResult<ProcessHandle> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            CREATE_UNICODE_ENVIRONMENT, CreateProcessWithLogonW, LOGON_WITH_PROFILE,
            PROCESS_INFORMATION, STARTUPINFOW,
        };
        use windows::core::{PCWSTR, PWSTR};

        let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
        let exe = config.exe_path.to_string_lossy().to_string();
        let mut command_line = wide(&build_command_line(&exe, &config.args));
        let cwd = config
            .working_dir
            .clone()
            .or_else(|| config.exe_path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();
        let (user_w, domain_w, pass_w, exe_w, cwd_w) = (
            wide(username),
            wide("."),
            wide(password),
            wide(&exe),
            wide(&cwd.to_string_lossy()),
        );
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        // The environment is left null so the process gets the target account's own
        // profile environment (USERPROFILE, APPDATA, ...), which is the point of the feature.
        // SAFETY: every pointer refers to a null-terminated buffer that outlives the call.
        unsafe {
            CreateProcessWithLogonW(
                PCWSTR(user_w.as_ptr()),
                PCWSTR(domain_w.as_ptr()),
                PCWSTR(pass_w.as_ptr()),
                LOGON_WITH_PROFILE,
                PCWSTR(exe_w.as_ptr()),
                PWSTR(command_line.as_mut_ptr()),
                CREATE_UNICODE_ENVIRONMENT,
                None,
                PCWSTR(cwd_w.as_ptr()),
                &startup,
                &mut info,
            )
        }
        .map_err(|e| {
            nucleus_common::error::NucleusError::Platform(format!(
                "Could not start '{exe}' as Windows user '{username}': {e}"
            ))
        })?;
        // SAFETY: both handles were just returned by a successful process creation.
        unsafe {
            let _ = CloseHandle(info.hThread);
            let _ = CloseHandle(info.hProcess);
        }
        Ok(ProcessHandle::new(info.dwProcessId, username))
    }

    fn get_user_profile_path(&self, username: &str) -> NucleusResult<PathBuf> {
        let base = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        Ok(PathBuf::from(format!("{}\\Users\\{}", base, username)))
    }
}

/// Builds a Windows command line: the quoted executable followed by its arguments, each
/// quoted when it contains whitespace or quotes (embedded quotes are backslash-escaped).
pub fn build_command_line(exe: &str, args: &[String]) -> String {
    let mut line = format!("\"{exe}\"");
    for arg in args {
        line.push(' ');
        if arg.is_empty() || arg.contains([' ', '\t', '"']) {
            line.push('"');
            line.push_str(&arg.replace('"', "\\\""));
            line.push('"');
        } else {
            line.push_str(arg);
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_only_what_needs_it() {
        let args = vec![
            "-windowed".to_string(),
            r"C:\My Games\p1.cfg".to_string(),
            String::new(),
            "say \"hi\"".to_string(),
        ];
        assert_eq!(
            build_command_line(r"C:\Games\game.exe", &args),
            r#""C:\Games\game.exe" -windowed "C:\My Games\p1.cfg" "" "say \"hi\"""#
        );
    }

    #[test]
    fn run_as_user_reports_a_clear_error_for_unknown_accounts() {
        let mgr = WindowsUserAccountManager::new();
        let mut cfg = ProcessConfig::default();
        cfg.exe_path = std::env::current_exe().unwrap();
        let err = match mgr.run_as_user("nucleus_no_such_user_zz", "wrong-password", &cfg) {
            Ok(_) => panic!("logon must fail for a non-existent account"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("nucleus_no_such_user_zz"), "{err}");
    }
}
