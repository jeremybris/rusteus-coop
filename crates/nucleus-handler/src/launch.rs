//! Process launching and process tracking.

use nucleus_common::error::{NucleusError, NucleusResult};
use std::path::Path;
use std::process::Command;

/// Responsible for executing game binaries with custom arguments, environment, and working directory.
pub struct ProcessLauncher;

/// Splits a command-line string into arguments, honouring double quotes.
///
/// Handlers build `Context.StartArguments` as one string and routinely quote paths that
/// contain spaces (`-config "C:\\My Games\\p1.cfg"`). Quotes group text and are removed;
/// `\\"` yields a literal quote. Everything else, backslashes included, is kept verbatim.
pub fn split_command_line(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
                has_token = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(current);
    }
    args
}

impl ProcessLauncher {
    /// Launches an executable process and returns the spawned process ID.
    pub fn launch<I, K, V>(
        executable: &Path,
        args: &[String],
        working_dir: &Path,
        environment: I,
    ) -> NucleusResult<u32>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<std::ffi::OsStr>,
        V: AsRef<std::ffi::OsStr>,
    {
        let mut cmd = Command::new(executable);
        cmd.args(args);
        cmd.current_dir(working_dir);

        for (k, v) in environment {
            cmd.env(k, v);
        }

        let child = cmd.spawn().map_err(|e| {
            NucleusError::Process(format!("Failed to spawn {:?}: {}", executable, e))
        })?;

        Ok(child.id())
    }

    /// Launches an executable process with optional working directory and returns the spawned process ID.
    pub fn launch_process<I, K, V>(
        executable: &Path,
        args: &[String],
        working_dir: Option<&Path>,
        environment: I,
    ) -> NucleusResult<u32>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<std::ffi::OsStr>,
        V: AsRef<std::ffi::OsStr>,
    {
        let fallback = executable.parent().unwrap_or_else(|| Path::new("."));
        let wd = working_dir.unwrap_or(fallback);
        Self::launch(executable, args, wd, environment)
    }

    /// Terminates a process by its process ID.
    pub fn terminate_process(pid: u32) -> NucleusResult<()> {
        Self::terminate(pid)
    }

    /// Terminates a process by its process ID.
    pub fn terminate(pid: u32) -> NucleusResult<()> {
        #[cfg(windows)]
        {
            let status = Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .output()
                .map_err(|e| NucleusError::Process(e.to_string()))?;

            if !status.status.success() {
                // If it already exited, that's acceptable
                return Ok(());
            }
        }

        #[cfg(not(windows))]
        {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_command_line_keeps_quoted_paths_together() {
        assert_eq!(
            split_command_line(r#"-windowed -config "C:\My Games\p1.cfg" +name Player1"#),
            vec!["-windowed", "-config", r"C:\My Games\p1.cfg", "+name", "Player1"]
        );
        assert_eq!(split_command_line("  "), Vec::<String>::new());
        assert_eq!(split_command_line(r#"-a "" -b"#), vec!["-a", "", "-b"]);
        assert_eq!(
            split_command_line(r#"-name \"Bob\" -dir C:\games\"#),
            vec!["-name", "\"Bob\"", "-dir", r"C:\games\"]
        );
        assert_eq!(split_command_line(r#"-x="a b""#), vec!["-x=a b"]);
    }

    #[test]
    fn test_process_launch_and_terminate() {
        #[cfg(windows)]
        let (prog, args) = (
            "cmd.exe",
            vec!["/c".to_string(), "timeout /t 5".to_string()],
        );
        #[cfg(not(windows))]
        let (prog, args) = ("sleep", vec!["5".to_string()]);

        let cur_dir = std::env::current_dir().unwrap();
        let pid = ProcessLauncher::launch(
            Path::new(prog),
            &args,
            &cur_dir,
            std::iter::empty::<(&str, &str)>(),
        )
        .unwrap();
        assert!(pid > 0);

        ProcessLauncher::terminate(pid).unwrap();
    }
}
