//! Per-instance file operations requested by handler properties:
//! `CopyFoldersTo`, `SymlinkFoldersTo`, `HardlinkFoldersTo`, `DeleteFiles`,
//! `RenameAndOrMoveFiles`, `CopyCustomUtils`, and the `HexEdit*` family.
//!
//! Instance folders are usually built from symlinks or hardlinks into the real game
//! installation. Every operation that rewrites a file therefore first *detaches* it (replaces
//! the link with a private copy) so the user's installed game is never modified.

use std::path::{Path, PathBuf};

/// How `*FoldersTo` entries are materialised in the instance folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderLinkMode {
    Copy,
    Symlink,
    Hardlink,
}

/// Splits an entry with an optional leading 1-based instance number.
///
/// `fields` is the number of fields without the instance prefix. Returns `None` when the
/// entry is malformed or addressed to a different instance.
fn fields_for_instance<'a>(
    entry: &'a str,
    fields: usize,
    player_index: usize,
) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = entry.split('|').collect();
    if parts.len() == fields + 1 {
        let target = parts[0].trim().parse::<usize>().ok()?;
        if target != player_index + 1 {
            return None;
        }
        Some(parts[1..].to_vec())
    } else if parts.len() == fields {
        Some(parts)
    } else {
        None
    }
}

fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn remove_existing(path: &Path) {
    if path.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(path);
    }
}

fn symlink_file(src: &Path, dst: &Path) -> bool {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(src, dst).is_ok()
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (src, dst);
        false
    }
}

/// Materialises `source|destination` folder pairs from the game folder into the instance.
///
/// Links fall back to hardlinks and then to copies, so an unprivileged session still ends
/// up with a usable folder. Returns the number of files placed.
pub fn place_folders(
    game_dir: &Path,
    instance_dir: &Path,
    entries: &[String],
    mode: FolderLinkMode,
) -> usize {
    let mut placed = 0usize;
    for entry in entries {
        let Some((src_rel, dst_rel)) = entry.split_once('|') else {
            tracing::warn!(
                "Ignoring malformed folder entry '{entry}' (expected 'source|destination')"
            );
            continue;
        };
        let src_root = game_dir.join(src_rel.trim());
        let dst_root = instance_dir.join(dst_rel.trim());
        if !src_root.is_dir() {
            tracing::warn!("Folder to place does not exist: {}", src_root.display());
            continue;
        }
        let mut files = Vec::new();
        walk_files(&src_root, &mut files);
        let _ = std::fs::create_dir_all(&dst_root);
        for file in files {
            let Ok(rel) = file.strip_prefix(&src_root) else {
                continue;
            };
            let dst = dst_root.join(rel);
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            remove_existing(&dst);
            let ok = match mode {
                FolderLinkMode::Copy => std::fs::copy(&file, &dst).is_ok(),
                FolderLinkMode::Symlink => {
                    symlink_file(&file, &dst)
                        || std::fs::hard_link(&file, &dst).is_ok()
                        || std::fs::copy(&file, &dst).is_ok()
                }
                FolderLinkMode::Hardlink => {
                    std::fs::hard_link(&file, &dst).is_ok() || std::fs::copy(&file, &dst).is_ok()
                }
            };
            if ok {
                placed += 1;
            } else {
                tracing::warn!("Failed to place {} -> {}", file.display(), dst.display());
            }
        }
    }
    placed
}

/// Applies `Game.DeleteFiles` (`[instance|]relativeFile`). `confirm` is consulted per file
/// unless the handler set `IgnoreDeleteFilesPrompt`.
pub fn delete_files(
    instance_dir: &Path,
    entries: &[String],
    player_index: usize,
    mut confirm: impl FnMut(&Path) -> bool,
) -> usize {
    let mut deleted = 0usize;
    for entry in entries {
        let Some(fields) = fields_for_instance(entry, 1, player_index) else {
            continue;
        };
        let target = instance_dir.join(fields[0].trim());
        if target.symlink_metadata().is_err() || target.is_dir() {
            continue;
        }
        if !confirm(&target) {
            continue;
        }
        if std::fs::remove_file(&target).is_ok() {
            deleted += 1;
        }
    }
    deleted
}

/// Applies `Game.RenameAndOrMoveFiles` (`[instance|]from|to`, both relative to the instance).
pub fn rename_or_move_files(instance_dir: &Path, entries: &[String], player_index: usize) -> usize {
    let mut moved = 0usize;
    for entry in entries {
        let Some(fields) = fields_for_instance(entry, 2, player_index) else {
            continue;
        };
        let from = instance_dir.join(fields[0].trim());
        let to = instance_dir.join(fields[1].trim());
        if from.symlink_metadata().is_err() {
            continue;
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        remove_existing(&to);
        if std::fs::rename(&from, &to).is_ok() {
            moved += 1;
        } else {
            tracing::warn!("Failed to move {} -> {}", from.display(), to.display());
        }
    }
    moved
}

/// Applies `Game.CopyCustomUtils`: `name[|relativeDestination[|instances]]`, where `name` is
/// a file or folder under `utils\User` and `instances` is a comma-separated list of 0-based
/// instance indices.
pub fn copy_custom_utils(
    utils_dir: &Path,
    instance_dir: &Path,
    entries: &[String],
    player_index: usize,
) -> usize {
    let user_dir = utils_dir.join("User");
    let mut copied = 0usize;
    for entry in entries {
        let parts: Vec<&str> = entry.split('|').collect();
        let name = parts[0].trim();
        if name.is_empty() {
            continue;
        }
        let dest_rel = parts.get(1).map(|s| s.trim()).unwrap_or_default();
        if let Some(instances) = parts.get(2) {
            let wanted: Vec<usize> = instances
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect();
            if !wanted.contains(&player_index) {
                continue;
            }
        }
        let source = user_dir.join(name);
        let dest_dir = if dest_rel.is_empty() {
            instance_dir.to_path_buf()
        } else {
            instance_dir.join(dest_rel)
        };
        if source.is_dir() {
            let mut files = Vec::new();
            walk_files(&source, &mut files);
            for file in files {
                let Ok(rel) = file.strip_prefix(&source) else {
                    continue;
                };
                let dst = dest_dir.join(rel);
                if let Some(parent) = dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                remove_existing(&dst);
                if std::fs::copy(&file, &dst).is_ok() {
                    copied += 1;
                }
            }
        } else if source.is_file() {
            let _ = std::fs::create_dir_all(&dest_dir);
            let dst = dest_dir.join(source.file_name().unwrap_or_default());
            remove_existing(&dst);
            if std::fs::copy(&source, &dst).is_ok() {
                copied += 1;
            }
        } else {
            tracing::warn!("Custom util not found: {}", source.display());
        }
    }
    copied
}

/// Replaces a (possibly linked) file with a private copy and hands its bytes to `patch`.
///
/// The link is removed before the new content is written, so neither a symlink target nor a
/// hardlinked original is ever written through. Returns `false` when the file is missing or
/// `patch` reports that nothing changed.
pub fn patch_detached(path: &Path, patch: impl FnOnce(&mut Vec<u8>) -> bool) -> bool {
    let Ok(mut bytes) = std::fs::read(path) else {
        return false;
    };
    if !patch(&mut bytes) {
        return false;
    }
    let tmp = path.with_extension("nucleus_patch_tmp");
    if std::fs::write(&tmp, &bytes).is_err() {
        return false;
    }
    remove_existing(path);
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
}

/// Overwrites the first occurrence of `find` with `replace` (the original `PatchFile`
/// semantics: later occurrences are left alone).
pub fn replace_first(bytes: &mut [u8], find: &[u8], replace: &[u8]) -> bool {
    if find.is_empty() || find.len() > bytes.len() {
        return false;
    }
    let Some(pos) = bytes.windows(find.len()).position(|w| w == find) else {
        return false;
    };
    let end = (pos + replace.len()).min(bytes.len());
    bytes[pos..end].copy_from_slice(&replace[..end - pos]);
    true
}

/// Applies an ASCII `find|replace` patch to `path`.
pub fn patch_text(path: &Path, find: &str, replace: &str) -> bool {
    let ok = patch_detached(path, |bytes| {
        replace_first(bytes, find.as_bytes(), replace.as_bytes())
    });
    if !ok {
        tracing::warn!(
            "Hex edit found nothing to patch in {} (looking for '{find}')",
            path.display()
        );
    }
    ok
}

fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() || clean.len() % 2 != 0 {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).ok())
        .collect()
}

fn parse_hex_offset(s: &str) -> Option<usize> {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    usize::from_str_radix(t, 16).ok()
}

/// Writes `hex_bytes` at `address` (hex) in `path`, growing the file if needed.
pub fn patch_at_address(path: &Path, address: &str, hex_bytes: &str) -> bool {
    let (Some(offset), Some(data)) = (parse_hex_offset(address), parse_hex_bytes(hex_bytes)) else {
        tracing::warn!("Invalid hex address edit '{address}|{hex_bytes}'");
        return false;
    };
    patch_detached(path, |bytes| {
        let end = offset + data.len();
        if bytes.len() < end {
            bytes.resize(end, 0);
        }
        bytes[offset..end].copy_from_slice(&data);
        true
    })
}

/// The `HexEdit*` handler properties for one instance.
#[derive(Debug, Default, Clone, Copy)]
pub struct HexEdits<'a> {
    /// `Game.HexEditExe`: one `find|replace` entry per player.
    pub exe: &'a [String],
    /// `Game.HexEditAllExes`: `find|replace` entries applied to every instance.
    pub all_exes: &'a [String],
    /// `Game.HexEditExeAddress`: `[instance|]address|bytes`.
    pub exe_address: &'a [String],
    /// `Game.HexEditFile`: one `file|find|replace` entry per player.
    pub file: &'a [String],
    /// `Game.HexEditAllFiles`: `file|find|replace` entries applied to every instance.
    pub all_files: &'a [String],
    /// `Game.HexEditFileAddress`: `[instance|]file|address|bytes`.
    pub file_address: &'a [String],
}

impl HexEdits<'_> {
    pub fn is_empty(&self) -> bool {
        self.exe.is_empty()
            && self.all_exes.is_empty()
            && self.exe_address.is_empty()
            && self.file.is_empty()
            && self.all_files.is_empty()
            && self.file_address.is_empty()
    }

    /// Applies every edit for `player_index`; returns how many patches were written.
    pub fn apply(&self, exe_path: &Path, instance_dir: &Path, player_index: usize) -> usize {
        let mut applied = 0usize;

        for entry in self.exe_address {
            if let Some(f) = fields_for_instance(entry, 2, player_index) {
                applied += patch_at_address(exe_path, f[0], f[1]) as usize;
            }
        }
        for entry in self.file_address {
            if let Some(f) = fields_for_instance(entry, 3, player_index) {
                let target = instance_dir.join(f[0].trim());
                applied += patch_at_address(&target, f[1], f[2]) as usize;
            }
        }
        for entry in self.all_exes {
            if let Some((find, replace)) = entry.split_once('|') {
                applied += patch_text(exe_path, find, replace) as usize;
            }
        }
        if let Some(entry) = self.exe.get(player_index) {
            // An empty entry means "leave this player's executable alone".
            if let Some((find, replace)) = entry.split_once('|') {
                applied += patch_text(exe_path, find, replace) as usize;
            }
        }
        for entry in self.all_files {
            let parts: Vec<&str> = entry.splitn(3, '|').collect();
            if parts.len() == 3 {
                applied +=
                    patch_text(&instance_dir.join(parts[0].trim()), parts[1], parts[2]) as usize;
            }
        }
        if let Some(entry) = self.file.get(player_index) {
            let parts: Vec<&str> = entry.splitn(3, '|').collect();
            if parts.len() == 3 {
                applied +=
                    patch_text(&instance_dir.join(parts[0].trim()), parts[1], parts[2]) as usize;
            }
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nucleus_file_ops_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn instance_prefixed_entries_only_apply_to_their_instance() {
        assert_eq!(fields_for_instance("2|a.txt", 1, 1), Some(vec!["a.txt"]));
        assert_eq!(fields_for_instance("2|a.txt", 1, 0), None);
        assert_eq!(fields_for_instance("a.txt", 1, 3), Some(vec!["a.txt"]));
        assert_eq!(fields_for_instance("x|a|b|c", 1, 0), None);
    }

    #[test]
    fn folders_are_placed_in_every_mode() {
        let dir = temp_dir("folders");
        let game = dir.join("game");
        let inst = dir.join("inst");
        std::fs::create_dir_all(game.join("cfg").join("sub")).unwrap();
        std::fs::write(game.join("cfg").join("a.ini"), "a").unwrap();
        std::fs::write(game.join("cfg").join("sub").join("b.ini"), "b").unwrap();
        for (mode, name) in [
            (FolderLinkMode::Copy, "copy"),
            (FolderLinkMode::Symlink, "sym"),
            (FolderLinkMode::Hardlink, "hard"),
        ] {
            let placed = place_folders(&game, &inst, &[format!("cfg|{name}")], mode);
            assert_eq!(placed, 2, "mode {mode:?}");
            assert_eq!(
                std::fs::read_to_string(inst.join(name).join("sub").join("b.ini")).unwrap(),
                "b"
            );
        }
        assert_eq!(
            place_folders(
                &game,
                &inst,
                &s(&["missing|x", "malformed"]),
                FolderLinkMode::Copy
            ),
            0
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn delete_and_rename_respect_instance_and_confirmation() {
        let dir = temp_dir("delren");
        for f in ["intro.bik", "keep.bik", "old.dll", "p2.cfg"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        let entries = s(&["intro.bik", "2|keep.bik", "missing.bik"]);
        assert_eq!(delete_files(&dir, &entries, 0, |_| true), 1);
        assert!(!dir.join("intro.bik").exists());
        assert!(dir.join("keep.bik").exists());
        assert_eq!(
            delete_files(&dir, &entries, 1, |_| false),
            0,
            "declined prompt keeps file"
        );
        assert!(dir.join("keep.bik").exists());

        let renames = s(&["old.dll|bin\\new.dll", "2|p2.cfg|p2_moved.cfg"]);
        assert_eq!(rename_or_move_files(&dir, &renames, 0), 1);
        assert!(dir.join("bin").join("new.dll").exists());
        assert!(dir.join("p2.cfg").exists());
        assert_eq!(rename_or_move_files(&dir, &renames, 1), 1);
        assert!(dir.join("p2_moved.cfg").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_utils_copy_files_and_folders_for_selected_instances() {
        let dir = temp_dir("utils");
        let utils = dir.join("utils");
        let inst = dir.join("inst");
        std::fs::create_dir_all(utils.join("User").join("pack").join("deep")).unwrap();
        std::fs::create_dir_all(&inst).unwrap();
        std::fs::write(utils.join("User").join("fix.dll"), "dll").unwrap();
        std::fs::write(
            utils.join("User").join("pack").join("deep").join("x.txt"),
            "x",
        )
        .unwrap();
        let entries = s(&["fix.dll|bin", "pack|mods|1", "absent.dll"]);
        assert_eq!(copy_custom_utils(&utils, &inst, &entries, 0), 1);
        assert!(inst.join("bin").join("fix.dll").exists());
        assert!(
            !inst.join("mods").exists(),
            "folder entry is limited to instance 1"
        );
        assert_eq!(copy_custom_utils(&utils, &inst, &entries, 1), 2);
        assert!(inst.join("mods").join("deep").join("x.txt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hex_edits_never_write_through_links_to_the_original_game() {
        let dir = temp_dir("hex");
        let game = dir.join("game");
        let inst = dir.join("inst");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::create_dir_all(&inst).unwrap();
        let original = b"HEADER steam_appid=480 name=AAAA name=AAAA".to_vec();
        std::fs::write(game.join("game.exe"), &original).unwrap();
        std::fs::write(game.join("data.bin"), [0u8; 8]).unwrap();
        // Hardlinks share storage with the original, exactly like a hardlinked instance.
        std::fs::hard_link(game.join("game.exe"), inst.join("game.exe")).unwrap();
        std::fs::hard_link(game.join("data.bin"), inst.join("data.bin")).unwrap();

        let exe = s(&["name=AAAA|name=P1__", "name=AAAA|name=P2__"]);
        let all = s(&["steam_appid|nucle_appid"]);
        let addr = s(&["2|0|4E55"]);
        let file_addr = s(&["data.bin|2|FFEE"]);
        let edits = HexEdits {
            exe: &exe,
            all_exes: &all,
            exe_address: &addr,
            file_address: &file_addr,
            ..Default::default()
        };
        assert!(!edits.is_empty());
        // Player 2 (index 1): address edit applies, and it gets *its own* HexEditExe entry.
        assert_eq!(edits.apply(&inst.join("game.exe"), &inst, 1), 4);
        let patched = std::fs::read(inst.join("game.exe")).unwrap();
        assert_eq!(&patched[..2], b"NU");
        let text = String::from_utf8_lossy(&patched).to_string();
        assert!(text.contains("nucle_appid=480"));
        assert!(
            text.contains("name=P2__ name=AAAA"),
            "only the first match is patched: {text}"
        );
        assert_eq!(
            std::fs::read(inst.join("data.bin")).unwrap()[2..4],
            [0xFF, 0xEE]
        );

        // The installed game is untouched.
        assert_eq!(std::fs::read(game.join("game.exe")).unwrap(), original);
        assert_eq!(std::fs::read(game.join("data.bin")).unwrap(), [0u8; 8]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn address_and_byte_parsing_rejects_garbage() {
        assert_eq!(parse_hex_offset("0x1F"), Some(31));
        assert_eq!(parse_hex_offset("1f"), Some(31));
        assert_eq!(parse_hex_offset("zz"), None);
        assert_eq!(parse_hex_bytes("90 90"), Some(vec![0x90, 0x90]));
        assert_eq!(parse_hex_bytes("909"), None);
        assert_eq!(parse_hex_bytes(""), None);
    }
}
