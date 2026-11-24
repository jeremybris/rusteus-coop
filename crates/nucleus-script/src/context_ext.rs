//! Extended `Context` API surface for handler scripts.
//!
//! Implements the members of the legacy `GenericContext` class that handler scripts rely on
//! beyond the core set registered in [`crate::bridge`]: registry editing, zip handling,
//! file discovery, text-file editing with explicit encodings, process control, and the
//! per-player path properties (`NucleusUserRoot`, `DocumentsPlayer`, `ScriptFolder`, ...).
//!
//! Native functions are plain function pointers, so any per-instance state they need is read
//! back from the `Context` object (`this`) they are invoked on.

use crate::handler_context::HandlerContext;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsResult, JsString, JsValue};
use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_common::file_util;
use std::path::{Path, PathBuf};

type NativeFn = fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>;

// ─── Argument helpers ───────────────────────────────────────────────────────

/// Converts any primitive argument into a Rust string (`undefined`/`null` yield `None`).
fn arg_string(args: &[JsValue], idx: usize, ctx: &mut Context) -> Option<String> {
    let v = args.get(idx)?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    if let Some(s) = v.as_string() {
        return Some(s.to_std_string_escaped());
    }
    v.to_string(ctx).ok().map(|s| s.to_std_string_escaped())
}

/// Like [`arg_string`], additionally expanding `%ENV%` variables for path arguments.
fn arg_path(args: &[JsValue], idx: usize, ctx: &mut Context) -> Option<String> {
    arg_string(args, idx, ctx).map(|s| file_util::expand_env_vars(&s))
}

fn arg_bool(args: &[JsValue], idx: usize) -> bool {
    args.get(idx).map(|v| v.to_boolean()).unwrap_or(false)
}

fn arg_i64(args: &[JsValue], idx: usize, ctx: &mut Context) -> Option<i64> {
    let v = args.get(idx)?;
    if let Some(n) = v.as_number() {
        return Some(n as i64);
    }
    let s = arg_string(args, idx, ctx)?;
    let t = s.trim();
    t.parse::<i64>()
        .ok()
        .or_else(|| t.parse::<f64>().ok().map(|f| f as i64))
}

/// Reads a JS array (or a single scalar) argument into a list of strings.
fn value_string_array(v: Option<&JsValue>, ctx: &mut Context) -> Vec<String> {
    let Some(v) = v else {
        return Vec::new();
    };
    if v.is_undefined() || v.is_null() {
        return Vec::new();
    }
    if let Some(obj) = v.as_object() {
        if obj.is_array() {
            let len = obj
                .get(JsString::from("length"), ctx)
                .ok()
                .and_then(|n| n.as_number())
                .unwrap_or(0.0) as usize;
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Ok(item) = obj.get(i, ctx) {
                    if item.is_undefined() || item.is_null() {
                        continue;
                    }
                    if let Ok(s) = item.to_string(ctx) {
                        out.push(s.to_std_string_escaped());
                    }
                }
            }
            return out;
        }
    }
    match v.to_string(ctx) {
        Ok(s) => vec![s.to_std_string_escaped()],
        Err(_) => Vec::new(),
    }
}

/// Reads a byte array argument (JS array of numbers, or the ASCII bytes of a string).
fn value_bytes(v: Option<&JsValue>, ctx: &mut Context) -> Vec<u8> {
    let Some(v) = v else {
        return Vec::new();
    };
    if let Some(s) = v.as_string() {
        return s.to_std_string_escaped().into_bytes();
    }
    if let Some(obj) = v.as_object() {
        let len = obj
            .get(JsString::from("length"), ctx)
            .ok()
            .and_then(|n| n.as_number())
            .unwrap_or(0.0) as usize;
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            if let Ok(item) = obj.get(i, ctx) {
                out.push(item.as_number().unwrap_or(0.0) as i64 as u8);
            }
        }
        return out;
    }
    Vec::new()
}

fn js_str(s: &str) -> JsValue {
    JsValue::from(JsString::from(s))
}

fn js_string_array(items: &[String], ctx: &mut Context) -> JsValue {
    let arr = JsArray::new(ctx);
    for item in items {
        let _ = arr.push(js_str(item), ctx);
    }
    JsValue::from(arr)
}

/// Reads a string property from the `Context` object a native method was invoked on.
fn this_string(this: &JsValue, key: &str, ctx: &mut Context) -> String {
    this.as_object()
        .and_then(|o| o.get(JsString::from(key), ctx).ok())
        .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
        .unwrap_or_default()
}

fn this_number(this: &JsValue, key: &str, ctx: &mut Context) -> f64 {
    this.as_object()
        .and_then(|o| o.get(JsString::from(key), ctx).ok())
        .and_then(|v| v.as_number())
        .unwrap_or(0.0)
}

// ─── Filesystem helpers ─────────────────────────────────────────────────────

/// Case-insensitive wildcard match supporting `*` and `?` (the `Directory.GetFiles` dialect).
pub fn wildcard_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn walk(root: &Path, recursive: bool, include_dirs: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir || include_dirs {
            out.push(path.clone());
        }
        if is_dir && recursive {
            walk(&path, recursive, include_dirs, out);
        }
    }
}

/// Finds files under `root` whose file name matches the wildcard `pattern`.
pub fn find_files(root: &Path, pattern: &str, recursive: bool) -> Vec<String> {
    let mut all = Vec::new();
    walk(root, recursive, false, &mut all);
    let pattern = if pattern.is_empty() { "*" } else { pattern };
    // `*.*` matches extension-less files too in the .NET dialect.
    let pattern = if pattern == "*.*" { "*" } else { pattern };
    all.into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| wildcard_match(pattern, n))
                .unwrap_or(false)
        })
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            if to.exists() {
                let _ = std::fs::remove_file(&to);
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

// ─── Text encoding helpers ──────────────────────────────────────────────────

fn resolve_encoding(label: &str) -> &'static encoding_rs::Encoding {
    let l = label.trim().to_ascii_lowercase();
    let mapped = match l.as_str() {
        "" | "utf8" | "utf-8" | "default" => "utf-8",
        "unicode" | "utf-16" | "utf16" | "utf-16le" => "utf-16le",
        "unicodefffe" | "utf-16be" => "utf-16be",
        "ascii" | "us-ascii" | "ansi" | "latin1" | "iso-8859-1" => "windows-1252",
        other => other,
    };
    encoding_rs::Encoding::for_label(mapped.as_bytes()).unwrap_or(encoding_rs::UTF_8)
}

/// Reads all lines of a text file using the named .NET-style encoding.
pub fn read_lines_encoded(path: &Path, encoder: &str) -> std::io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let enc = resolve_encoding(encoder);
    let (text, _, _) = enc.decode(&bytes);
    Ok(text.lines().map(|l| l.to_string()).collect())
}

/// Writes lines (CRLF terminated, like `File.WriteAllLines`) using the named encoding.
pub fn write_lines_encoded(path: &Path, lines: &[String], encoder: &str) -> std::io::Result<()> {
    let mut text = String::new();
    for l in lines {
        text.push_str(l);
        text.push_str("\r\n");
    }
    let enc = resolve_encoding(encoder);
    let bytes: Vec<u8> = if enc == encoding_rs::UTF_16LE {
        let mut b = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        b
    } else if enc == encoding_rs::UTF_16BE {
        let mut b = vec![0xFE, 0xFF];
        for u in text.encode_utf16() {
            b.extend_from_slice(&u.to_be_bytes());
        }
        b
    } else {
        enc.encode(&text).0.into_owned()
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, bytes)
}

/// `ReplacePartialLinesInTextFile`: each entry is `lineNumber|regex|replacement`.
pub fn replace_partial_lines(path: &Path, entries: &[String], encoder: &str) -> bool {
    let Ok(mut lines) = read_lines_encoded(path, encoder) else {
        return false;
    };
    for item in entries {
        let parts: Vec<&str> = item.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(num) = parts[0].trim().parse::<usize>() else {
            continue;
        };
        if num == 0 || num > lines.len() {
            continue;
        }
        if let Ok(re) = regex::Regex::new(parts[1]) {
            // .NET uses `$1` group references, which the regex crate understands as well.
            lines[num - 1] = re.replace_all(&lines[num - 1], parts[2]).into_owned();
        }
    }
    write_lines_encoded(path, &lines, encoder).is_ok()
}

/// `EditTextFile`: every line containing `ref_lines[i]` is replaced by `new_lines[i]`,
/// or removed when the replacement is the literal `Delete`.
pub fn edit_text_file(
    path: &Path,
    ref_lines: &[String],
    new_lines: &[String],
    encoder: &str,
) -> bool {
    let Ok(content) = read_lines_encoded(path, encoder) else {
        return false;
    };
    let mut out: Vec<String> = Vec::with_capacity(content.len());
    for line in content {
        let mut replaced: Option<Option<String>> = None;
        for (i, r) in ref_lines.iter().enumerate() {
            if !r.is_empty() && line.contains(r.as_str()) {
                let new = new_lines.get(i).cloned().unwrap_or_default();
                replaced = Some(if new == "Delete" { None } else { Some(new) });
            }
        }
        match replaced {
            None => out.push(line),
            Some(Some(new)) => out.push(new),
            Some(None) => {}
        }
    }
    write_lines_encoded(path, &out, encoder).is_ok()
}

// ─── Zip helpers ────────────────────────────────────────────────────────────

/// Extracts every entry of `source_zip` into `dest`, overwriting existing files.
pub fn extract_zip(source_zip: &Path, dest: &Path, password: &str) -> bool {
    let Ok(file) = std::fs::File::open(source_zip) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let _ = std::fs::create_dir_all(dest);
    let mut ok = true;
    for i in 0..archive.len() {
        let entry = if password.is_empty() {
            archive.by_index(i)
        } else {
            archive.by_index_decrypt(i, password.as_bytes())
        };
        let Ok(mut entry) = entry else {
            ok = false;
            continue;
        };
        // `enclosed_name` rejects entries that would escape the destination folder.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest.join(rel);
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out_path);
            continue;
        }
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::File::create(&out_path) {
            Ok(mut out) => {
                if std::io::copy(&mut entry, &mut out).is_err() {
                    ok = false;
                }
            }
            Err(_) => ok = false,
        }
    }
    ok
}

fn add_path_to_zip<W: std::io::Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    src: &Path,
    entry_name: &str,
) -> zip::result::ZipResult<()> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if src.is_dir() {
        for entry in std::fs::read_dir(src)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let child = if entry_name.is_empty() {
                name
            } else {
                format!("{}/{}", entry_name.trim_end_matches('/'), name)
            };
            add_path_to_zip(writer, &entry.path(), &child)?;
        }
    } else {
        writer.start_file(entry_name, options)?;
        let mut f = std::fs::File::open(src)?;
        std::io::copy(&mut f, writer)?;
    }
    Ok(())
}

/// Rewrites `source_zip` into `save_path`, dropping `entries_to_remove` and adding
/// `items_to_add` (each `sourcePath|directoryInArchive`). Kept entries are copied raw,
/// so encrypted entries stay encrypted.
pub fn edit_zip(
    source_zip: &Path,
    save_path: &Path,
    items_to_add: &[String],
    entries_to_remove: &[String],
) -> bool {
    let Ok(file) = std::fs::File::open(source_zip) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let tmp_path = save_path.with_extension("nucleus_tmp_zip");
    let Ok(out) = std::fs::File::create(&tmp_path) else {
        return false;
    };
    let mut writer = zip::ZipWriter::new(out);
    let removals: Vec<String> = entries_to_remove
        .iter()
        .map(|e| e.replace('\\', "/"))
        .collect();
    let mut ok = true;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index_raw(i) else {
            ok = false;
            continue;
        };
        let name = entry.name().to_string();
        let root = name.split('/').next().unwrap_or_default();
        let skip = removals
            .iter()
            .any(|r| r == &name || (r.ends_with('/') && r.trim_end_matches('/') == root));
        if skip {
            continue;
        }
        if writer.raw_copy_file(entry).is_err() {
            ok = false;
        }
    }
    for item in items_to_add {
        let Some((src, dir_in_zip)) = item.split_once('|') else {
            continue;
        };
        let src_path = Path::new(src);
        let dir = dir_in_zip.replace('\\', "/");
        let dir = dir.trim_matches('/');
        let base = src_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let entry_name = if dir.is_empty() {
            base
        } else {
            format!("{dir}/{base}")
        };
        if add_path_to_zip(&mut writer, src_path, &entry_name).is_err() {
            ok = false;
        }
    }
    if writer.finish().is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return false;
    }
    drop(archive);
    if save_path.exists() {
        let _ = std::fs::remove_file(save_path);
    }
    std::fs::rename(&tmp_path, save_path).is_ok() && ok
}

// ─── Date helpers ───────────────────────────────────────────────────────────

fn local_system_time(args: &[JsValue], ctx: &mut Context) -> Option<std::time::SystemTime> {
    use chrono::TimeZone;
    let y = arg_i64(args, 1, ctx)? as i32;
    let mo = arg_i64(args, 2, ctx)? as u32;
    let d = arg_i64(args, 3, ctx)? as u32;
    let h = arg_i64(args, 4, ctx).unwrap_or(0) as u32;
    let mi = arg_i64(args, 5, ctx).unwrap_or(0) as u32;
    let s = arg_i64(args, 6, ctx).unwrap_or(0) as u32;
    let dt = chrono::Local
        .with_ymd_and_hms(y, mo, d, h, mi, s)
        .earliest()?;
    Some(std::time::SystemTime::from(dt))
}

// ─── Process helpers ────────────────────────────────────────────────────────

fn hidden_command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Splits one `tasklist /FO CSV` row into its quoted fields.
fn parse_csv_row(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Kills every process whose image name (or, with `by_window`, main window title)
/// contains `needle`, case-insensitively. Returns the number of processes signalled.
pub fn kill_processes_matching(needle: &str, by_window: bool) -> usize {
    if needle.trim().is_empty() {
        return 0;
    }
    let needle = needle.to_lowercase();
    let own_pid = std::process::id();
    let mut killed = 0usize;
    if cfg!(windows) {
        let mut cmd = hidden_command("tasklist");
        cmd.args(["/FO", "CSV", "/NH"]);
        if by_window {
            cmd.arg("/V");
        }
        let Ok(output) = cmd.output() else {
            return 0;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let cols = parse_csv_row(line);
            if cols.len() < 2 {
                continue;
            }
            let Ok(pid) = cols[1].trim().parse::<u32>() else {
                continue;
            };
            if pid == own_pid || pid == 0 {
                continue;
            }
            let hay = if by_window {
                cols.last().cloned().unwrap_or_default()
            } else {
                // Process.ProcessName has no extension.
                cols[0].trim_end_matches(".exe").to_string()
            };
            let hay = hay.to_lowercase();
            if by_window && (hay == "n/a" || hay.is_empty()) {
                continue;
            }
            if hay.contains(&needle) {
                let status = hidden_command("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .output();
                if status.map(|o| o.status.success()).unwrap_or(false) {
                    killed += 1;
                }
            }
        }
    } else if !by_window {
        let status = std::process::Command::new("pkill")
            .args(["-i", "-f", &needle])
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            killed = 1;
        }
    }
    killed
}

/// Starts `path` detached with its own folder as working directory, optionally elevated.
pub fn start_process(path: &Path, args: &str, as_admin: bool, working_dir: Option<&Path>) -> bool {
    if !path.exists() {
        return false;
    }
    let cwd = working_dir
        .map(|p| p.to_path_buf())
        .or_else(|| path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    if cfg!(windows) && as_admin {
        // ShellExecute "runas" through PowerShell keeps this crate free of extra Win32 bindings.
        let esc = |s: &str| s.replace('\'', "''");
        let mut script = format!(
            "Start-Process -FilePath '{}' -WorkingDirectory '{}' -Verb RunAs",
            esc(&path.to_string_lossy()),
            esc(&cwd.to_string_lossy())
        );
        if !args.trim().is_empty() {
            script.push_str(&format!(" -ArgumentList '{}'", esc(args)));
        }
        return hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .spawn()
            .is_ok();
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut cmd = if cfg!(windows) && (ext == "bat" || ext == "cmd") {
        let mut c = hidden_command("cmd");
        c.arg("/C").arg(path);
        c
    } else {
        std::process::Command::new(path)
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if !args.trim().is_empty() {
            cmd.raw_arg(args);
        }
    }
    #[cfg(not(windows))]
    {
        cmd.args(args.split_whitespace());
    }
    if !cwd.as_os_str().is_empty() {
        cmd.current_dir(cwd);
    }
    cmd.spawn().is_ok()
}

// ─── Registry ───────────────────────────────────────────────────────────────

/// Registry backup/edit support used by `Context.EditRegKey` and friends.
pub mod registry {
    use std::path::{Path, PathBuf};

    /// Folder holding `.reg` exports of keys edited during a session.
    pub fn backup_dir(nucleus_folder: &Path) -> PathBuf {
        nucleus_folder.join("utils").join("backup")
    }

    /// Exports `full_key` to `utils\backup\<leaf>.reg` once, before its first modification.
    pub fn backup_key(nucleus_folder: &Path, base_key: &str, sub_key: &str) {
        let leaf = sub_key.rsplit('\\').next().unwrap_or(sub_key);
        if leaf.is_empty() {
            return;
        }
        let dir = backup_dir(nucleus_folder);
        let reg_path = dir.join(format!("{leaf}.reg"));
        if reg_path.exists() {
            return;
        }
        let _ = std::fs::create_dir_all(&dir);
        if cfg!(windows) {
            let full = format!("{base_key}\\{sub_key}");
            let _ = super::hidden_command("reg.exe")
                .arg("export")
                .arg(&full)
                .arg(&reg_path)
                .arg("/y")
                .output();
        }
    }

    /// Re-imports every `.reg` export made by handler registry edits and removes the exports.
    ///
    /// `User Shell Folders.reg` is owned by the Documents-redirection guard and left alone.
    /// Returns the number of exports restored.
    pub fn restore_backups(nucleus_folder: &Path) -> usize {
        let dir = backup_dir(nucleus_folder);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return 0;
        };
        let mut restored = 0usize;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_reg = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("reg"))
                .unwrap_or(false);
            if !is_reg {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.eq_ignore_ascii_case("User Shell Folders.reg") {
                continue;
            }
            if cfg!(windows) {
                let ok = super::hidden_command("reg.exe")
                    .arg("import")
                    .arg(&path)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if !ok {
                    tracing::warn!("Failed to restore registry backup {}", path.display());
                    continue;
                }
            }
            let _ = std::fs::remove_file(&path);
            restored += 1;
        }
        restored
    }

    /// Value written by `EditRegKey`.
    #[derive(Debug, Clone, PartialEq)]
    pub enum RegData {
        String(String),
        ExpandString(String),
        Binary(Vec<u8>),
        DWord(u32),
        QWord(u64),
        MultiString(Vec<String>),
    }

    #[cfg(windows)]
    fn hive(base_key: &str) -> Option<winreg::RegKey> {
        use winreg::enums::*;
        let h = match base_key {
            "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
            "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
            "HKEY_USERS" => HKEY_USERS,
            _ => return None,
        };
        Some(winreg::RegKey::predef(h))
    }

    #[cfg(windows)]
    fn utf16_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(|u| u.to_le_bytes())
            .collect()
    }

    /// Returns whether `base\sub_key` exists.
    #[cfg(windows)]
    pub fn key_exists(base_key: &str, sub_key: &str) -> bool {
        hive(base_key)
            .map(|h| h.open_subkey(sub_key).is_ok())
            .unwrap_or(false)
    }

    /// Sets a value, creating the key when missing.
    #[cfg(windows)]
    pub fn set_value(base_key: &str, sub_key: &str, name: &str, data: &RegData) -> bool {
        use winreg::RegValue;
        use winreg::enums::*;
        let Some(h) = hive(base_key) else {
            return false;
        };
        let Ok((key, _)) = h.create_subkey(sub_key) else {
            return false;
        };
        let raw = match data {
            RegData::String(s) => RegValue {
                bytes: utf16_bytes(s),
                vtype: REG_SZ,
            },
            RegData::ExpandString(s) => RegValue {
                bytes: utf16_bytes(s),
                vtype: REG_EXPAND_SZ,
            },
            RegData::Binary(b) => RegValue {
                bytes: b.clone(),
                vtype: REG_BINARY,
            },
            RegData::DWord(n) => RegValue {
                bytes: n.to_le_bytes().to_vec(),
                vtype: REG_DWORD,
            },
            RegData::QWord(n) => RegValue {
                bytes: n.to_le_bytes().to_vec(),
                vtype: REG_QWORD,
            },
            RegData::MultiString(items) => {
                let mut bytes = Vec::new();
                for item in items {
                    bytes.extend(utf16_bytes(item));
                }
                bytes.extend([0u8, 0u8]);
                RegValue {
                    bytes,
                    vtype: REG_MULTI_SZ,
                }
            }
        };
        key.set_raw_value(name, &raw).is_ok()
    }

    /// Reads a value as its string representation (`None` when the key or value is missing).
    #[cfg(windows)]
    pub fn read_value(base_key: &str, sub_key: &str, name: &str) -> Option<String> {
        use winreg::enums::*;
        let key = hive(base_key)?.open_subkey(sub_key).ok()?;
        let raw = key.get_raw_value(name).ok()?;
        let decode_utf16 = |bytes: &[u8]| -> String {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        };
        Some(match raw.vtype {
            REG_SZ | REG_EXPAND_SZ => decode_utf16(&raw.bytes).trim_end_matches('\0').to_string(),
            REG_MULTI_SZ => decode_utf16(&raw.bytes)
                .split('\0')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            REG_DWORD if raw.bytes.len() >= 4 => {
                u32::from_le_bytes([raw.bytes[0], raw.bytes[1], raw.bytes[2], raw.bytes[3]])
                    .to_string()
            }
            REG_QWORD if raw.bytes.len() >= 8 => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&raw.bytes[..8]);
                u64::from_le_bytes(b).to_string()
            }
            _ => raw
                .bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
        })
    }

    #[cfg(windows)]
    pub fn create_sub_key(base_key: &str, sub_key: &str, child: &str) -> bool {
        hive(base_key)
            .and_then(|h| {
                h.open_subkey_with_flags(sub_key, winreg::enums::KEY_ALL_ACCESS)
                    .ok()
            })
            .map(|k| k.create_subkey(child).is_ok())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    pub fn delete_sub_key(base_key: &str, sub_key: &str, child: &str) -> bool {
        hive(base_key)
            .and_then(|h| {
                h.open_subkey_with_flags(sub_key, winreg::enums::KEY_ALL_ACCESS)
                    .ok()
            })
            .map(|k| k.delete_subkey_all(child).is_ok())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    pub fn delete_values(base_key: &str, sub_key: &str, names: &[String]) -> bool {
        let Some(key) = hive(base_key).and_then(|h| {
            h.open_subkey_with_flags(sub_key, winreg::enums::KEY_ALL_ACCESS)
                .ok()
        }) else {
            return false;
        };
        let mut ok = true;
        for n in names {
            if key.delete_value(n).is_err() {
                ok = false;
            }
        }
        ok
    }

    #[cfg(not(windows))]
    pub fn key_exists(_: &str, _: &str) -> bool {
        false
    }
    #[cfg(not(windows))]
    pub fn set_value(_: &str, _: &str, _: &str, _: &RegData) -> bool {
        false
    }
    #[cfg(not(windows))]
    pub fn read_value(_: &str, _: &str, _: &str) -> Option<String> {
        None
    }
    #[cfg(not(windows))]
    pub fn create_sub_key(_: &str, _: &str, _: &str) -> bool {
        false
    }
    #[cfg(not(windows))]
    pub fn delete_sub_key(_: &str, _: &str, _: &str) -> bool {
        false
    }
    #[cfg(not(windows))]
    pub fn delete_values(_: &str, _: &str, _: &[String]) -> bool {
        false
    }
}

fn valid_hive(base: &str) -> bool {
    matches!(
        base,
        "HKEY_LOCAL_MACHINE" | "HKEY_CURRENT_USER" | "HKEY_USERS"
    )
}

/// Converts the JS value passed to `EditRegKey` into typed registry data.
fn reg_data_from_js(
    value: &JsValue,
    reg_type: i32,
    ctx: &mut Context,
) -> Option<registry::RegData> {
    use registry::RegData;
    if value.is_undefined() || value.is_null() {
        return None;
    }
    let as_text = |ctx: &mut Context| -> String {
        value
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default()
    };
    let as_u64 = |ctx: &mut Context| -> u64 {
        if let Some(n) = value.as_number() {
            return n as i64 as u64;
        }
        let t = as_text(ctx);
        let t = t.trim();
        if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u64::from_str_radix(hex, 16).unwrap_or(0)
        } else {
            t.parse::<i64>().map(|v| v as u64).unwrap_or(0)
        }
    };
    Some(match reg_type {
        4 => RegData::DWord(as_u64(ctx) as u32),
        11 => RegData::QWord(as_u64(ctx)),
        3 => RegData::Binary(as_text(ctx).into_bytes()),
        2 => RegData::ExpandString(as_text(ctx)),
        7 => RegData::MultiString(value_string_array(Some(value), ctx)),
        _ => RegData::String(as_text(ctx)),
    })
}

fn edit_reg_key(this: &JsValue, args: &[JsValue], ctx: &mut Context, backup: bool) -> JsValue {
    let (Some(base), Some(skey), Some(name)) = (
        arg_string(args, 0, ctx),
        arg_string(args, 1, ctx),
        arg_string(args, 2, ctx),
    ) else {
        return JsValue::from(false);
    };
    if !valid_hive(&base) {
        return JsValue::from(false);
    }
    let reg_type = arg_i64(args, 4, ctx).unwrap_or(1) as i32;
    let Some(data) = args.get(3).and_then(|v| reg_data_from_js(v, reg_type, ctx)) else {
        return JsValue::from(false);
    };
    if backup && registry::key_exists(&base, &skey) {
        let nucleus_folder = this_string(this, "NucleusFolder", ctx);
        registry::backup_key(Path::new(&nucleus_folder), &base, &skey);
    }
    JsValue::from(registry::set_value(&base, &skey, &name, &data))
}

// ─── Installation ───────────────────────────────────────────────────────────

fn folder_string(p: &Path, trailing: bool) -> String {
    let mut s = p.to_string_lossy().to_string();
    if trailing && !s.ends_with('\\') && !s.ends_with('/') {
        s.push(std::path::MAIN_SEPARATOR);
    }
    s
}

/// Registers `Nucleus.RegType` so handlers can write `Nucleus.RegType.DWord`.
pub fn register_reg_type(context: &mut Context) -> NucleusResult<()> {
    let global = context.global_object();
    let nucleus = match global.get(JsString::from("Nucleus"), context) {
        Ok(v) => v.as_object().cloned(),
        Err(_) => None,
    };
    let Some(nucleus) = nucleus else {
        return Ok(());
    };
    let reg = JsObject::default();
    for (name, val) in [
        ("None", -1),
        ("Unknown", 0),
        ("String", 1),
        ("ExpandString", 2),
        ("Binary", 3),
        ("DWord", 4),
        ("MultiString", 7),
        ("QWord", 11),
    ] {
        reg.set(JsString::from(name), JsValue::from(val), false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
    }
    nucleus
        .set(
            JsString::from("RegType"),
            JsValue::from(reg),
            false,
            context,
        )
        .map_err(|e| NucleusError::Script(e.to_string()))?;
    Ok(())
}

/// Installs the extended properties and methods onto the per-instance `Context` object.
pub fn install(
    context: &mut Context,
    ctx_obj: &JsObject,
    handler_ctx: &HandlerContext,
) -> NucleusResult<()> {
    macro_rules! set {
        ($key:expr, $val:expr) => {
            ctx_obj
                .set(JsString::from($key), $val, false, context)
                .map_err(|e| NucleusError::Script(e.to_string()))?;
        };
    }
    macro_rules! set_fn {
        ($key:expr, $func:expr) => {{
            let f: NativeFn = $func;
            let v = crate::bridge::JsBridge::create_native_function(context, f);
            set!($key, v);
        }};
    }

    // ── Paths ──
    let globals = nucleus_common::globals::Globals::try_get();
    let nucleus_folder: PathBuf = match (&handler_ctx.nucleus_folder, globals) {
        (Some(f), _) if !f.is_empty() => PathBuf::from(f),
        (_, Some(g)) => g.install_root.clone(),
        _ => std::env::current_dir().unwrap_or_default(),
    };
    let env_root = globals
        .map(|g| g.user_environment_root.clone())
        .or_else(dirs::home_dir)
        .unwrap_or_default();
    let docs_root = globals
        .map(|g| g.user_documents_root.clone())
        .or_else(dirs::document_dir)
        .unwrap_or_else(|| env_root.join("Documents"));
    let docs_parent = docs_root
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| env_root.clone());
    let nickname = handler_ctx.nickname.as_str();

    set!(
        "NucleusFolder",
        js_str(&folder_string(&nucleus_folder, false))
    );
    set!(
        "UtilFolder",
        js_str(&folder_string(&nucleus_folder.join("utils"), false))
    );
    set!(
        "NucleusEnvironmentRoot",
        js_str(&folder_string(&env_root, false))
    );
    set!(
        "NucleusDocumentsRoot",
        js_str(&folder_string(&docs_root, false))
    );
    let user_root = if handler_ctx.using_nucleus_accounts {
        env_root
            .parent()
            .map(|p| p.join(format!("nucleusplayer{}", handler_ctx.player_index + 1)))
            .unwrap_or_else(|| env_root.clone())
    } else {
        env_root.clone()
    };
    set!("NucleusUserRoot", js_str(&folder_string(&user_root, false)));

    let documents_root = docs_parent.join("NucleusCoop");
    let documents_player = documents_root.join(nickname).join("Documents");
    let (environment_root, environment_player) = if handler_ctx.user_profile_converted_to_documents
    {
        (documents_root.clone(), documents_player.clone())
    } else {
        (
            env_root.join("NucleusCoop"),
            env_root.join("NucleusCoop").join(nickname),
        )
    };
    set!(
        "DocumentsRoot",
        js_str(&folder_string(&documents_root, true))
    );
    set!(
        "DocumentsPlayer",
        js_str(&folder_string(&documents_player, true))
    );
    set!(
        "EnvironmentRoot",
        js_str(&folder_string(&environment_root, true))
    );
    set!(
        "EnvironmentPlayer",
        js_str(&folder_string(&environment_player, true))
    );
    set!(
        "PlayerGameFilesBackupFolder",
        js_str(&folder_string(
            &env_root
                .join("NucleusCoop")
                .join("_Game Files Backup_")
                .join(&handler_ctx.handler_guid)
                .join(nickname),
            false
        ))
    );
    set!(
        "UserProfileConvertedToDocuments",
        JsValue::from(handler_ctx.user_profile_converted_to_documents)
    );
    for (key, val) in [
        (
            "UserProfileConfigPath",
            &handler_ctx.user_profile_config_path,
        ),
        ("UserProfileSavePath", &handler_ctx.user_profile_save_path),
        ("DocumentsConfigPath", &handler_ctx.documents_config_path),
        ("DocumentsSavePath", &handler_ctx.documents_save_path),
    ] {
        set!(key, js_str(val.as_deref().unwrap_or_default()));
    }

    let handlers_folder = handler_ctx
        .handlers_folder
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| globals.map(|g| g.handlers_path.to_string_lossy().to_string()))
        .unwrap_or_else(|| folder_string(&nucleus_folder.join("handlers"), false));
    set!("HandlersFolder", js_str(&handlers_folder));
    set!(
        "ScriptFolder",
        js_str(handler_ctx.script_folder.as_deref().unwrap_or_default())
    );
    set!(
        "LauncherFolder",
        js_str(handler_ctx.launcher_folder.as_deref().unwrap_or_default())
    );

    // ── Player / session facts ──
    set!("Arch", js_str(&handler_ctx.arch));
    set!(
        "PlayerSteamID",
        js_str(&handler_ctx.player_steam_id.clone().unwrap_or_default())
    );
    let steam_ids = js_string_array(&handler_ctx.player_steam_ids, context);
    set!("PlayerSteamIDs", steam_ids);
    let user_name = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    set!("UserName", js_str(user_name.trim()));
    set!(
        "HasKeyboardPlayer",
        JsValue::from(handler_ctx.has_keyboard_player)
    );
    set!(
        "bHasKeyboardPlayer",
        JsValue::from(handler_ctx.has_keyboard_player)
    );
    set!(
        "NumControllers",
        JsValue::from(handler_ctx.num_controllers as i32)
    );
    set!(
        "NumKeyboards",
        JsValue::from(handler_ctx.num_keyboards as i32)
    );
    let gamepad_guid = handler_ctx
        .gamepad_guid
        .clone()
        .unwrap_or_else(|| "00000000-0000-0000-0000-000000000000".to_string());
    set!("GamepadGuid", js_str(&gamepad_guid));
    set!(
        "x360ceGamepadGuid",
        js_str(&format!("IG_{}", gamepad_guid.replace('-', "")))
    );
    set!(
        "RawHID",
        js_str(handler_ctx.raw_hid.as_deref().unwrap_or_default())
    );
    let (ow, oh) = (
        handler_ctx.orig_width.unwrap_or(handler_ctx.monitor_width),
        handler_ctx
            .orig_height
            .unwrap_or(handler_ctx.monitor_height),
    );
    let mut orig = HandlerContext::new(0, String::new());
    orig.width = ow;
    orig.height = oh;
    set!("OrigAspectRatio", js_str(&orig.aspect_ratio()));
    set!(
        "OrigAspectRatioDecimal",
        JsValue::from(orig.aspect_ratio_decimal())
    );
    set!("IsFullscreen", JsValue::from(handler_ctx.is_fullscreen));
    for (key, val) in [
        ("SteamLang", &handler_ctx.steam_lang),
        ("EpicLang", &handler_ctx.epic_lang),
        ("GogLang", &handler_ctx.gog_lang),
    ] {
        set!(key, js_str(val.as_deref().unwrap_or("english")));
    }
    set!("KeepEditedRegKeys", JsValue::from(false));
    set!("Error", JsValue::null());
    let symlink_files = js_string_array(&handler_ctx.symlink_files, context);
    set!("SymlinkFiles", symlink_files);
    for (i, c) in handler_ctx.proto_controllers.iter().enumerate() {
        set!(
            format!("_ProtoController{}", i + 1).as_str(),
            JsValue::from(*c)
        );
    }
    set!("_HideDesktopRequested", JsValue::from(false));
    set!("_HideTaskbarRequested", JsValue::from(false));

    // ── Conversions ──
    set_fn!("ConvertToString", |_this, args, ctx| {
        Ok(js_str(&arg_string(args, 0, ctx).unwrap_or_default()))
    });
    set_fn!("ToUpperCase", |_this, args, ctx| {
        Ok(js_str(
            &arg_string(args, 0, ctx).unwrap_or_default().to_uppercase(),
        ))
    });
    set_fn!("ToLowerCase", |_this, args, ctx| {
        Ok(js_str(
            &arg_string(args, 0, ctx).unwrap_or_default().to_lowercase(),
        ))
    });
    set_fn!("RandomString", |_this, args, ctx| {
        let size = arg_i64(args, 0, ctx).unwrap_or(0).clamp(0, 4096) as usize;
        let lower = arg_bool(args, 1);
        let base = if lower { b'a' } else { b'A' };
        // xorshift seeded from the clock: handlers only need per-launch uniqueness here.
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
        let mut out = String::with_capacity(size);
        for _ in 0..size {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            out.push((base + (seed % 26) as u8) as char);
        }
        Ok(js_str(&out))
    });
    set_fn!("GetFileName", |_this, args, ctx| {
        let full = arg_string(args, 0, ctx).unwrap_or_default();
        Ok(js_str(full.rsplit('\\').next().unwrap_or_default()))
    });
    set_fn!("Wait", |_this, args, ctx| {
        let ms = arg_i64(args, 0, ctx).unwrap_or(0).clamp(0, 600_000) as u64;
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(JsValue::undefined())
    });
    set_fn!("NewSaveInfo", |_this, args, ctx| {
        let obj = JsObject::default();
        for (i, key) in ["Section", "Key", "Value"].iter().enumerate() {
            let v = arg_string(args, i, ctx).unwrap_or_default();
            obj.set(JsString::from(*key), js_str(&v), false, ctx)?;
        }
        Ok(JsValue::from(obj))
    });

    // ── File discovery ──
    set_fn!("FindFiles", |_this, args, ctx| {
        let root = arg_path(args, 0, ctx).unwrap_or_default();
        let pattern = arg_string(args, 1, ctx).unwrap_or_else(|| "*".to_string());
        let found = find_files(Path::new(&root), &pattern, arg_bool(args, 2));
        Ok(js_string_array(&found, ctx))
    });
    set_fn!("FindFilePartialName", |_this, args, ctx| {
        let root = arg_path(args, 0, ctx).unwrap_or_default();
        let names = value_string_array(args.get(1), ctx);
        let mut all = Vec::new();
        walk(Path::new(&root), true, true, &mut all);
        let mut found = Vec::new();
        for p in all {
            let s = p.to_string_lossy().to_string();
            for n in &names {
                if !n.is_empty() && s.contains(n.as_str()) {
                    found.push(s.clone());
                }
            }
        }
        Ok(js_string_array(&found, ctx))
    });
    set_fn!("SearchSubFolder", |_this, args, ctx| {
        let (Some(parent), Some(pattern)) = (arg_path(args, 0, ctx), arg_string(args, 1, ctx))
        else {
            return Ok(JsValue::null());
        };
        if pattern.is_empty() {
            return Ok(JsValue::null());
        }
        let Ok(entries) = std::fs::read_dir(&parent) else {
            return Ok(JsValue::null());
        };
        let mut dirs: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path().to_string_lossy().to_string())
            .filter(|s| s.contains(pattern.as_str()))
            .collect();
        dirs.sort();
        Ok(dirs.first().map(|s| js_str(s)).unwrap_or(JsValue::null()))
    });
    set_fn!("SearchSubFolders", |_this, args, ctx| {
        let (Some(parent), Some(pattern)) = (arg_path(args, 0, ctx), arg_string(args, 1, ctx))
        else {
            return Ok(JsValue::null());
        };
        if pattern.is_empty() {
            return Ok(JsValue::null());
        }
        let Ok(entries) = std::fs::read_dir(&parent) else {
            return Ok(JsValue::null());
        };
        let mut dirs: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path().to_string_lossy().to_string())
            .filter(|s| s.contains(pattern.as_str()))
            .collect();
        dirs.sort();
        Ok(js_string_array(&dirs, ctx))
    });

    // ── File operations ──
    set_fn!("CopyFolder", |_this, args, ctx| {
        let (Some(src), Some(dst)) = (arg_path(args, 0, ctx), arg_path(args, 1, ctx)) else {
            return Ok(JsValue::from(false));
        };
        Ok(JsValue::from(
            copy_dir_all(Path::new(&src), Path::new(&dst)).is_ok(),
        ))
    });
    set_fn!("CopyScriptFolder", |this, args, ctx| {
        let src = this_string(this, "ScriptFolder", ctx);
        let Some(dst) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::from(false));
        };
        if src.is_empty() || !Path::new(&src).is_dir() {
            return Ok(JsValue::from(false));
        }
        Ok(JsValue::from(
            copy_dir_all(Path::new(&src), Path::new(&dst)).is_ok(),
        ))
    });
    set_fn!("MoveFolder", |this, args, ctx| {
        let (Some(src_rel), Some(dst_rel)) = (arg_string(args, 0, ctx), arg_string(args, 1, ctx))
        else {
            return Ok(JsValue::from(false));
        };
        // Relative names resolve against the instanced game folder.
        let root = this_string(this, "RootFolder", ctx);
        let resolve = |p: &str| -> PathBuf {
            let path = Path::new(p);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                Path::new(&root).join(p)
            }
        };
        let (src, dst) = (resolve(&src_rel), resolve(&dst_rel));
        if !src.is_dir() {
            return Ok(JsValue::from(false));
        }
        if !dst.exists() {
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::rename(&src, &dst).is_ok() {
                return Ok(JsValue::from(true));
            }
        }
        let ok = copy_dir_all(&src, &dst).is_ok();
        if ok {
            let _ = std::fs::remove_dir_all(&src);
        }
        Ok(JsValue::from(ok))
    });
    set_fn!("BackupFile", |this, args, ctx| {
        let overwrite = arg_bool(args, 1);
        let is_array = args
            .first()
            .and_then(|v| v.as_object())
            .map(|o| o.is_array())
            .unwrap_or(false);
        // The single-path overload only runs for the first player (legacy behaviour).
        if !is_array && this_number(this, "PlayerID", ctx) as i64 > 0 {
            return Ok(JsValue::from(true));
        }
        for file in value_string_array(args.first(), ctx) {
            let file = file_util::expand_env_vars(&file);
            let path = Path::new(&file);
            if !path.is_file() {
                tracing::warn!("Unable to backup {file}, file does not exist");
                continue;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let backup = path.with_file_name(format!("{stem}_NUCLEUS_BACKUP{ext}"));
            if backup.exists() && !overwrite {
                continue;
            }
            let _ = std::fs::copy(path, &backup);
        }
        Ok(JsValue::from(true))
    });
    set_fn!("ProceedSymlink", |this, _args, ctx| {
        let orig = this_string(this, "OrigRootFolder", ctx);
        let root = this_string(this, "RootFolder", ctx);
        let files = this
            .as_object()
            .and_then(|o| o.get(JsString::from("SymlinkFiles"), ctx).ok());
        for rel in value_string_array(files.as_ref(), ctx) {
            let src = Path::new(&orig).join(&rel);
            let dst = Path::new(&root).join(&rel);
            link_or_copy_file(&src, &dst);
        }
        Ok(JsValue::from(true))
    });
    set_fn!("HexEdit", |_this, args, ctx| {
        use std::io::{Seek, SeekFrom, Write};
        let (Some(file), Some(address)) = (arg_path(args, 0, ctx), arg_string(args, 1, ctx)) else {
            return Ok(JsValue::from(false));
        };
        let bytes = value_bytes(args.get(2), ctx);
        let addr = address
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        let Ok(offset) = u64::from_str_radix(addr, 16) else {
            return Ok(JsValue::from(false));
        };
        let result = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file)
            .and_then(|mut f| {
                f.seek(SeekFrom::Start(offset))?;
                f.write_all(&bytes)
            });
        Ok(JsValue::from(result.is_ok()))
    });
    set_fn!("ModifiedDate", |_this, args, ctx| {
        let Some(file) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::undefined());
        };
        if let Some(t) = local_system_time(args, ctx) {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&file) {
                let _ = f.set_modified(t);
            }
        }
        Ok(JsValue::undefined())
    });
    set_fn!("CreatedDate", |_this, args, ctx| {
        let Some(file) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::undefined());
        };
        if let Some(t) = local_system_time(args, ctx) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::FileTimesExt;
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&file) {
                    let _ = f.set_times(std::fs::FileTimes::new().set_created(t));
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (&file, t);
            }
        }
        Ok(JsValue::undefined())
    });

    // ── Text files ──
    set_fn!("ReplacePartialLinesInTextFile", |_this, args, ctx| {
        let Some(path) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::from(false));
        };
        let entries = value_string_array(args.get(1), ctx);
        let encoder = arg_string(args, 2, ctx).unwrap_or_default();
        Ok(JsValue::from(replace_partial_lines(
            Path::new(&path),
            &entries,
            &encoder,
        )))
    });
    set_fn!("EditTextFile", |_this, args, ctx| {
        let Some(path) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::from(false));
        };
        let refs = value_string_array(args.get(1), ctx);
        let news = value_string_array(args.get(2), ctx);
        let encoder = arg_string(args, 3, ctx).unwrap_or_default();
        Ok(JsValue::from(edit_text_file(
            Path::new(&path),
            &refs,
            &news,
            &encoder,
        )))
    });

    // ── Zip ──
    set_fn!("ExtractZip", |_this, args, ctx| {
        let (Some(src), Some(dst)) = (arg_path(args, 0, ctx), arg_path(args, 1, ctx)) else {
            return Ok(JsValue::from(false));
        };
        let password = arg_string(args, 2, ctx).unwrap_or_default();
        Ok(JsValue::from(extract_zip(
            Path::new(&src),
            Path::new(&dst),
            &password,
        )))
    });
    set_fn!("EditZipFile", |_this, args, ctx| {
        let (Some(src), Some(save)) = (arg_path(args, 0, ctx), arg_path(args, 2, ctx)) else {
            return Ok(JsValue::from(false));
        };
        let add = value_string_array(args.get(3), ctx);
        let remove = value_string_array(args.get(4), ctx);
        Ok(JsValue::from(edit_zip(
            Path::new(&src),
            Path::new(&save),
            &add,
            &remove,
        )))
    });

    // ── Processes ──
    set_fn!("StartProcess", |_this, args, ctx| {
        let Some(path) = arg_path(args, 0, ctx) else {
            return Ok(JsValue::from(false));
        };
        let proc_args = arg_string(args, 1, ctx).unwrap_or_default();
        start_process(Path::new(&path), &proc_args, arg_bool(args, 2), None);
        Ok(JsValue::from(true))
    });
    set_fn!("RunAdditionalFiles", |this, args, ctx| {
        let files = value_string_array(args.first(), ctx);
        let change_working_dir = arg_bool(args, 1);
        // Overloads: (files, changeDir, seconds[, asAdmin, prompt]) and
        // (files, changeDir, customText, seconds, showFilePath, asAdmin, prompt, confirm).
        let long_form = args.get(2).map(|v| v.is_string()).unwrap_or(false);
        let (pause_idx, admin_idx) = if long_form { (3, 5) } else { (2, 3) };
        let pause = arg_i64(args, pause_idx, ctx).unwrap_or(0).clamp(0, 600) as u64;
        let as_admin = arg_bool(args, admin_idx);
        let player = this_number(this, "PlayerID", ctx) as i64;
        let nucleus_folder = this_string(this, "NucleusFolder", ctx);
        let additional = Path::new(&nucleus_folder)
            .join("content")
            .join("AdditionalFiles");
        let count = files.len();
        for (i, entry) in files.into_iter().enumerate() {
            let file = match entry.split_once('|') {
                Some((who, f)) => {
                    if !who.eq_ignore_ascii_case("all")
                        && who.trim().parse::<i64>().ok() != Some(player + 1)
                    {
                        continue;
                    }
                    f.to_string()
                }
                None => {
                    if player > 0 {
                        continue;
                    }
                    entry
                }
            };
            let file = file_util::expand_env_vars(&file);
            let cwd = if change_working_dir {
                let _ = std::fs::create_dir_all(&additional);
                Some(additional.as_path())
            } else {
                None
            };
            start_process(Path::new(&file), "", as_admin, cwd);
            if i + 1 < count && pause > 0 {
                std::thread::sleep(std::time::Duration::from_secs(pause));
            }
        }
        Ok(JsValue::undefined())
    });
    set_fn!("KillProcessesMatchingProcessName", |_this, args, ctx| {
        let name = arg_string(args, 0, ctx).unwrap_or_default();
        kill_processes_matching(&name, false);
        Ok(JsValue::from(true))
    });
    set_fn!("KillProcessesMatchingWindowName", |_this, args, ctx| {
        let name = arg_string(args, 0, ctx).unwrap_or_default();
        kill_processes_matching(&name, true);
        Ok(JsValue::from(true))
    });

    // ── Registry ──
    set_fn!("EditRegKey", |this, args, ctx| {
        Ok(edit_reg_key(this, args, ctx, true))
    });
    set_fn!("EditRegKeyNoBackup", |this, args, ctx| {
        Ok(edit_reg_key(this, args, ctx, false))
    });
    set_fn!("ReadRegKey", |_this, args, ctx| {
        let (Some(base), Some(skey), Some(name)) = (
            arg_string(args, 0, ctx),
            arg_string(args, 1, ctx),
            arg_string(args, 2, ctx),
        ) else {
            return Ok(js_str(""));
        };
        if !valid_hive(&base) {
            return Ok(js_str(""));
        }
        Ok(registry::read_value(&base, &skey, &name)
            .map(|s| js_str(&s))
            .unwrap_or(JsValue::null()))
    });
    set_fn!("CreateRegKey", |_this, args, ctx| {
        let (Some(base), Some(skey), Some(child)) = (
            arg_string(args, 0, ctx),
            arg_string(args, 1, ctx),
            arg_string(args, 2, ctx),
        ) else {
            return Ok(JsValue::from(false));
        };
        if base == "HKEY_LOCAL_MACHINE" || base == "HKEY_CURRENT_USER" {
            registry::create_sub_key(&base, &skey, &child);
        }
        Ok(JsValue::from(true))
    });
    set_fn!("DeleteRegKey", |this, args, ctx| {
        let (Some(base), Some(skey), Some(child)) = (
            arg_string(args, 0, ctx),
            arg_string(args, 1, ctx),
            arg_string(args, 2, ctx),
        ) else {
            return Ok(JsValue::undefined());
        };
        if valid_hive(&base) && registry::key_exists(&base, &skey) {
            let nucleus_folder = this_string(this, "NucleusFolder", ctx);
            registry::backup_key(Path::new(&nucleus_folder), &base, &skey);
            registry::delete_sub_key(&base, &skey, &child);
        }
        Ok(JsValue::undefined())
    });
    set_fn!("DeleteRegKeyValues", |this, args, ctx| {
        let (Some(base), Some(skey)) = (arg_string(args, 0, ctx), arg_string(args, 1, ctx)) else {
            return Ok(JsValue::undefined());
        };
        let names = value_string_array(args.get(2), ctx);
        if valid_hive(&base) && registry::key_exists(&base, &skey) {
            let nucleus_folder = this_string(this, "NucleusFolder", ctx);
            registry::backup_key(Path::new(&nucleus_folder), &base, &skey);
            registry::delete_values(&base, &skey, &names);
        }
        Ok(JsValue::undefined())
    });

    // ── Session requests reflected back to the launcher ──
    set_fn!("SetProtoInputMultipleControllers", |this, args, ctx| {
        if let Some(obj) = this.as_object() {
            for i in 0..4 {
                let v = arg_i64(args, i, ctx).unwrap_or(0) as i32;
                obj.set(
                    JsString::from(format!("_ProtoController{}", i + 1).as_str()),
                    JsValue::from(v),
                    false,
                    ctx,
                )?;
            }
        }
        Ok(JsValue::undefined())
    });
    set_fn!("HideDesktop", |this, args, ctx| {
        if let Some(obj) = this.as_object() {
            if this_number(this, "PlayerID", ctx) as i64 == 0 {
                obj.set(
                    JsString::from("_HideDesktopRequested"),
                    JsValue::from(true),
                    false,
                    ctx,
                )?;
                if arg_bool(args, 0) {
                    obj.set(
                        JsString::from("_HideTaskbarRequested"),
                        JsValue::from(true),
                        false,
                        ctx,
                    )?;
                }
            }
        }
        Ok(JsValue::undefined())
    });
    for name in ["HideTaskBar", "HideTaskbar"] {
        set_fn!(name, |this, _args, ctx| {
            if let Some(obj) = this.as_object() {
                if this_number(this, "PlayerID", ctx) as i64 == 0 {
                    obj.set(
                        JsString::from("_HideTaskbarRequested"),
                        JsValue::from(true),
                        false,
                        ctx,
                    )?;
                }
            }
            Ok(JsValue::undefined())
        });
    }

    Ok(())
}

/// Creates a file symlink, falling back to a hardlink and finally a plain copy.
pub fn link_or_copy_file(src: &Path, dst: &Path) -> bool {
    if !src.is_file() {
        return false;
    }
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if dst.exists() || dst.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(dst);
    }
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(src, dst).is_ok();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(src, dst).is_ok();
    #[cfg(not(any(windows, unix)))]
    let linked = false;
    linked || std::fs::hard_link(src, dst).is_ok() || std::fs::copy(src, dst).is_ok()
}

/// Reads back the launcher-facing requests a handler made through `Context` during `Game.Play`.
pub fn reflect_requests(
    context: &mut Context,
    ctx_obj: &JsObject,
    handler_ctx: &mut HandlerContext,
) {
    for i in 0..4 {
        let key = format!("_ProtoController{}", i + 1);
        if let Ok(v) = ctx_obj.get(JsString::from(key.as_str()), context) {
            if let Some(n) = v.as_number() {
                handler_ctx.proto_controllers[i] = n as i32;
            }
        }
    }
    let mut flag = |key: &str| -> bool {
        ctx_obj
            .get(JsString::from(key), context)
            .ok()
            .map(|v| v.to_boolean())
            .unwrap_or(false)
    };
    if flag("_HideDesktopRequested") {
        handler_ctx.hide_desktop_requested = true;
    }
    if flag("_HideTaskbarRequested") {
        handler_ctx.hide_taskbar_requested = true;
    }
    if flag("KeepEditedRegKeys") {
        handler_ctx.keep_edited_reg_keys = true;
    }
    for (key, slot) in [
        (
            "UserProfileConfigPath",
            &mut handler_ctx.user_profile_config_path,
        ),
        (
            "UserProfileSavePath",
            &mut handler_ctx.user_profile_save_path,
        ),
        (
            "DocumentsConfigPath",
            &mut handler_ctx.documents_config_path,
        ),
        ("DocumentsSavePath", &mut handler_ctx.documents_save_path),
    ] {
        if let Ok(v) = ctx_obj.get(JsString::from(key), context) {
            if let Some(s) = v.as_string() {
                let s = s.to_std_string_escaped();
                if !s.is_empty() {
                    *slot = Some(s);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nucleus_ctx_ext_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn wildcard_matching_follows_dotnet_dialect() {
        assert!(wildcard_match("*.ini", "Settings.INI"));
        assert!(wildcard_match("save?.dat", "save1.dat"));
        assert!(!wildcard_match("save?.dat", "save12.dat"));
        assert!(wildcard_match("*", "anything"));
        assert!(!wildcard_match("*.cfg", "game.ini"));
    }

    #[test]
    fn find_files_respects_recursion_flag() {
        let dir = temp_dir("find");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.ini"), "x").unwrap();
        std::fs::write(dir.join("sub").join("b.ini"), "x").unwrap();
        std::fs::write(dir.join("sub").join("c.txt"), "x").unwrap();
        assert_eq!(find_files(&dir, "*.ini", false).len(), 1);
        assert_eq!(find_files(&dir, "*.ini", true).len(), 2);
        assert_eq!(find_files(&dir, "*.*", true).len(), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn replace_partial_lines_uses_regex_per_line() {
        let dir = temp_dir("partial");
        let file = dir.join("cfg.txt");
        std::fs::write(&file, "width=1920\r\nheight=1080\r\n").unwrap();
        assert!(replace_partial_lines(
            &file,
            &["1|\\d+|960".to_string(), "9|x|y".to_string()],
            ""
        ));
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text, "width=960\r\nheight=1080\r\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn edit_text_file_replaces_and_deletes_matching_lines() {
        let dir = temp_dir("edit");
        let file = dir.join("cfg.txt");
        std::fs::write(&file, "fullscreen=1\nvsync=1\nname=abc\n").unwrap();
        assert!(edit_text_file(
            &file,
            &["fullscreen".to_string(), "vsync".to_string()],
            &["fullscreen=0".to_string(), "Delete".to_string()],
            "utf-8"
        ));
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text, "fullscreen=0\r\nname=abc\r\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn utf16_round_trip_keeps_bom_and_content() {
        let dir = temp_dir("utf16");
        let file = dir.join("u.txt");
        write_lines_encoded(
            &file,
            &["héllo".to_string(), "wörld".to_string()],
            "unicode",
        )
        .unwrap();
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        assert_eq!(
            read_lines_encoded(&file, "unicode").unwrap(),
            vec!["héllo".to_string(), "wörld".to_string()]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn zip_edit_and_extract_round_trip() {
        use std::io::Write;
        let dir = temp_dir("zip");
        let src_zip = dir.join("in.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&src_zip).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("keep.txt", o).unwrap();
            w.write_all(b"keep").unwrap();
            w.start_file("drop.txt", o).unwrap();
            w.write_all(b"drop").unwrap();
            w.start_file("dropdir/inner.txt", o).unwrap();
            w.write_all(b"inner").unwrap();
            w.finish().unwrap();
        }
        let extra = dir.join("extra.cfg");
        std::fs::write(&extra, "extra").unwrap();
        let out_zip = dir.join("out.zip");
        assert!(edit_zip(
            &src_zip,
            &out_zip,
            &[format!("{}|config", extra.display())],
            &["drop.txt".to_string(), "dropdir\\".to_string()],
        ));
        let dest = dir.join("out");
        assert!(extract_zip(&out_zip, &dest, ""));
        assert_eq!(
            std::fs::read_to_string(dest.join("keep.txt")).unwrap(),
            "keep"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("config").join("extra.cfg")).unwrap(),
            "extra"
        );
        assert!(!dest.join("drop.txt").exists());
        assert!(!dest.join("dropdir").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn csv_rows_keep_commas_inside_quotes() {
        let cols = parse_csv_row("\"game.exe\",\"1234\",\"Console\",\"1\",\"10,240 K\"");
        assert_eq!(cols[0], "game.exe");
        assert_eq!(cols[1], "1234");
        assert_eq!(cols[4], "10,240 K");
    }

    #[cfg(windows)]
    #[test]
    fn registry_values_round_trip_under_hkcu() {
        use registry::*;
        let sub = format!("Software\\NucleusCoopTests\\ctx_ext_{}", std::process::id());
        assert!(set_value(
            "HKEY_CURRENT_USER",
            &sub,
            "Name",
            &RegData::String("Player1".into())
        ));
        assert!(set_value(
            "HKEY_CURRENT_USER",
            &sub,
            "Width",
            &RegData::DWord(960)
        ));
        assert_eq!(
            read_value("HKEY_CURRENT_USER", &sub, "Name").as_deref(),
            Some("Player1")
        );
        assert_eq!(
            read_value("HKEY_CURRENT_USER", &sub, "Width").as_deref(),
            Some("960")
        );
        assert!(delete_values(
            "HKEY_CURRENT_USER",
            &sub,
            &["Width".to_string()]
        ));
        assert_eq!(read_value("HKEY_CURRENT_USER", &sub, "Width"), None);
        let leaf = format!("ctx_ext_{}", std::process::id());
        assert!(delete_sub_key(
            "HKEY_CURRENT_USER",
            "Software\\NucleusCoopTests",
            &leaf
        ));
        assert!(!key_exists("HKEY_CURRENT_USER", &sub));
        let _ = delete_sub_key("HKEY_CURRENT_USER", "Software", "NucleusCoopTests");
    }
}
