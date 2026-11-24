//! C# Legacy Parity: `%USERPROFILE%\NucleusCoop\metaInfo.json`
//!
//! Stores playtime, last played timestamp, favorite status, icon path,
//! and game-specific options matching `Nucleus.Gaming.Coop.GameMetaInfo`.

use nucleus_common::error::NucleusResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Individual game entry in `%USERPROFILE%\NucleusCoop\metaInfo.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameMetaInfoEntry {
    /// Accumulated playtime in total seconds stored as a string (e.g. "640") or null.
    #[serde(rename = "TotalPlayTime", default)]
    pub total_play_time: Option<String>,

    /// Full DateTime string of last play session (e.g. "9/20/2025 7:05:10 PM") or null.
    #[serde(rename = "LastPlayedAt", default)]
    pub last_played_at: Option<String>,

    /// Optional custom game icon path.
    #[serde(rename = "IconPath", default)]
    pub icon_path: Option<String>,

    /// Whether game profiles are saved.
    #[serde(rename = "SaveProfile", default)]
    pub save_profile: Option<bool>,

    /// Whether profiles are disabled for this game.
    #[serde(rename = "DisableProfiles", default)]
    pub disable_profiles: Option<bool>,

    /// Whether to keep symlinks/instance folders on exit.
    #[serde(rename = "KeepSymLink", default)]
    pub keep_sym_link: Option<bool>,

    /// Whether this game is marked as favorite.
    #[serde(rename = "Favorite", default)]
    pub favorite: Option<bool>,

    /// Whether this is the first launch of the game.
    #[serde(rename = "FirstLaunch", default)]
    pub first_launch: Option<bool>,

    /// Whether to check for handler updates.
    #[serde(rename = "CheckUpdate", default)]
    pub check_update: Option<bool>,

    /// Use XInput index directly for instances.
    #[serde(rename = "UseApiIndex", default)]
    pub use_api_index: Option<bool>,

    /// Use XInput index for guests.
    #[serde(rename = "UseApiIndexForGuests", default)]
    pub use_api_index_for_guests: Option<bool>,

    /// Assign gamepad by button press in profile.
    #[serde(rename = "ProfileAssignGamepadByButonPress", default)]
    pub profile_assign_gamepad_by_button_press: Option<bool>,

    /// Steam language override.
    #[serde(rename = "SteamLanguage", default)]
    pub steam_language: Option<String>,
}

impl GameMetaInfoEntry {
    /// Returns total playtime in seconds as `u64`.
    pub fn play_time_seconds(&self) -> u64 {
        self.total_play_time
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    }

    /// Sets total playtime in seconds.
    pub fn set_play_time_seconds(&mut self, seconds: u64) {
        self.total_play_time = Some(seconds.to_string());
    }

    /// Formats playtime matching C# `FormatPlayTime`: `00h:00m:00s` or `...` if none.
    pub fn format_play_time(&self) -> String {
        let total = self.play_time_seconds();
        if total == 0 && self.total_play_time.is_none() {
            return "...".to_string();
        }
        let seconds = total % 60;
        let minutes = (total / 60) % 60;
        let hours = total / 3600;
        format!("{:02}h:{:02}m:{:02}s", hours, minutes, seconds)
    }

    /// Formats last played matching C# `GetLastPlayed`: date part only or `...` if none.
    pub fn display_last_played(&self) -> String {
        match self.last_played_at.as_deref() {
            Some(s) if !s.trim().is_empty() => s.split_whitespace().next().unwrap_or(s).to_string(),
            _ => "...".to_string(),
        }
    }
}

/// Represents the container and persistence manager for `%USERPROFILE%\NucleusCoop\metaInfo.json`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetaInfoStore {
    /// Dictionary of game GUID or title -> metadata entry.
    pub entries: HashMap<String, GameMetaInfoEntry>,
    /// Path to the JSON file on disk.
    pub path: PathBuf,
}

impl MetaInfoStore {
    /// Returns the canonical path to `%USERPROFILE%\NucleusCoop\metaInfo.json`.
    pub fn default_path() -> PathBuf {
        // Test binaries (cargo places them in a `deps` folder) must never read or write the
        // user's real play history; each test process gets a private store instead.
        if Self::running_as_test_binary() {
            return std::env::temp_dir()
                .join("nucleus-coop-tests")
                .join(std::process::id().to_string())
                .join("metaInfo.json");
        }
        if let Some(home) = dirs::home_dir() {
            home.join("NucleusCoop").join("metaInfo.json")
        } else {
            PathBuf::from("metaInfo.json")
        }
    }

    /// Whether the current process is a cargo-built test executable.
    fn running_as_test_binary() -> bool {
        std::env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.parent()
                    .and_then(|dir| dir.file_name())
                    .map(|name| name.eq_ignore_ascii_case("deps"))
            })
            .unwrap_or(false)
    }

    /// Loads the store from the default path `%USERPROFILE%\NucleusCoop\metaInfo.json`.
    pub fn load_default() -> Self {
        Self::load_from_path(Self::default_path())
    }

    /// Loads the store from a specific filesystem path.
    pub fn load_from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut store = Self {
            entries: HashMap::new(),
            path: path.clone(),
        };

        if path.exists() {
            let parsed = std::fs::read_to_string(&path).ok().and_then(|content| {
                serde_json::from_str::<HashMap<String, GameMetaInfoEntry>>(&content).ok()
            });
            match parsed {
                Some(entries) => {
                    tracing::info!("Loaded {} entry/entries from {:?}", entries.len(), path);
                    store.entries = entries;
                }
                None => {
                    // The next save would replace an unreadable file with an empty store.
                    // Keep what is there so the play history can be recovered by hand.
                    let keep = path.with_extension("json.unreadable");
                    match std::fs::copy(&path, &keep) {
                        Ok(_) => tracing::warn!(
                            "{:?} could not be read; a copy was kept at {:?}",
                            path,
                            keep
                        ),
                        Err(e) => tracing::warn!("{:?} could not be read ({})", path, e),
                    }
                }
            }
        }

        store
    }

    /// Saves the current metadata entries to disk at `self.path`.
    pub fn save(&self) -> NucleusResult<()> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let json_str = serde_json::to_string_pretty(&self.entries)?;
        // Write to a sibling file and rename it into place, so a crash or a concurrent
        // reader never observes a half-written store.
        let tmp = self
            .path
            .with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, json_str)?;
        if let Err(e) = std::fs::rename(&tmp, &self.path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        tracing::info!(
            "Saved {} metaInfo entries to {:?}",
            self.entries.len(),
            self.path
        );
        Ok(())
    }

    /// Finds a metadata entry matching `key` either by exact match, case-insensitively, or by name.
    pub fn find_entry(&self, key: &str) -> Option<(&String, &GameMetaInfoEntry)> {
        if self.entries.contains_key(key) {
            return self.entries.get_key_value(key);
        }
        let lower = key.to_lowercase();
        self.entries.iter().find(|(k, _)| k.to_lowercase() == lower)
    }

    /// Finds a mutable metadata entry matching `key` or inserts a new default entry for `key`.
    pub fn get_or_insert_mut(&mut self, key: &str) -> &mut GameMetaInfoEntry {
        let matched_key = if self.entries.contains_key(key) {
            Some(key.to_string())
        } else {
            let lower = key.to_lowercase();
            self.entries
                .keys()
                .find(|k| k.to_lowercase() == lower)
                .cloned()
        };

        let insert_key = matched_key.unwrap_or_else(|| key.to_string());
        self.entries
            .entry(insert_key)
            .or_insert_with(|| GameMetaInfoEntry {
                save_profile: Some(true),
                disable_profiles: Some(false),
                keep_sym_link: Some(false),
                favorite: Some(false),
                first_launch: Some(true),
                check_update: Some(true),
                steam_language: Some("App Setting".to_string()),
                ..Default::default()
            })
    }

    /// Records accumulated playtime and current timestamp matching C# formatting.
    pub fn record_playtime(&mut self, key: &str, elapsed_seconds: u64, now_formatted: &str) {
        let entry = self.get_or_insert_mut(key);
        let current_seconds = entry.play_time_seconds();
        entry.set_play_time_seconds(current_seconds + elapsed_seconds);
        entry.last_played_at = Some(now_formatted.to_string());
        entry.first_launch = Some(false);
    }

    /// Toggles or updates the favorite status for a given game.
    pub fn set_favorite(&mut self, key: &str, is_fav: bool) {
        let entry = self.get_or_insert_mut(key);
        entry.favorite = Some(is_fav);
    }

    /// Removes a game entry from the store. Returns true if an entry was removed.
    pub fn remove_game(&mut self, key: &str) -> bool {
        let to_remove: Vec<String> = self
            .entries
            .keys()
            .filter(|k| *k == key || k.eq_ignore_ascii_case(key))
            .cloned()
            .collect();

        let removed = !to_remove.is_empty();
        for k in to_remove {
            self.entries.remove(&k);
        }
        removed
    }
}

/// Returns the current timestamp formatted as "M/D/YYYY h:mm:ss A" matching C# DateTime.Now.ToString().
pub fn now_formatted() -> String {
    let total_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let sec = total_secs % 60;
    let total_mins = total_secs / 60;
    let min = total_mins % 60;
    let total_hours = total_mins / 60;
    let hour_24 = total_hours % 24;
    let total_days = (total_hours / 24) as i64;

    // Howard Hinnant algorithm for civil date from days since 1970-01-01
    let z = total_days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let am_pm = if hour_24 >= 12 { "PM" } else { "AM" };
    let hour = if hour_24 == 0 {
        12
    } else if hour_24 > 12 {
        hour_24 - 12
    } else {
        hour_24
    };

    format!("{}/{}/{} {}:{:02}:{:02} {}", m, d, y, hour, min, sec, am_pm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_info_entry_playtime_formatting() {
        let mut entry = GameMetaInfoEntry::default();
        assert_eq!(entry.format_play_time(), "...");
        assert_eq!(entry.display_last_played(), "...");

        entry.set_play_time_seconds(640);
        assert_eq!(entry.format_play_time(), "00h:10m:40s");

        entry.last_played_at = Some("9/20/2025 7:05:10 PM".to_string());
        assert_eq!(entry.display_last_played(), "9/20/2025");
    }

    #[test]
    fn test_meta_info_store_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_test_metainfo_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("metaInfo.json");

        let mut store = MetaInfoStore::load_from_path(&path);
        store.record_playtime("Slime Rancher 2", 120, "10/23/2026 4:15:00 PM");
        store.set_favorite("Slime Rancher 2", true);
        store.save().unwrap();

        let loaded = MetaInfoStore::load_from_path(&path);
        let entry = loaded
            .find_entry("slime rancher 2")
            .map(|(_, e)| e)
            .unwrap();
        assert_eq!(entry.play_time_seconds(), 120);
        assert_eq!(entry.favorite, Some(true));
        assert_eq!(entry.display_last_played(), "10/23/2026");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_default_path_is_isolated_for_test_binaries() {
        let path = MetaInfoStore::default_path();
        assert!(
            path.starts_with(std::env::temp_dir()),
            "tests must not use the real user store: {path:?}"
        );
        if let Some(home) = dirs::home_dir() {
            assert!(!path.starts_with(home.join("NucleusCoop")));
        }
    }

    #[test]
    fn test_unreadable_store_is_preserved_before_being_replaced() {
        let dir = std::env::temp_dir().join(format!(
            "nucleus_meta_unreadable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("metaInfo.json");
        std::fs::write(&path, "{ \"Half written").unwrap();

        let mut store = MetaInfoStore::load_from_path(&path);
        assert!(store.entries.is_empty());
        let kept = dir.join("metaInfo.json.unreadable");
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "{ \"Half written",
            "the unreadable content must survive"
        );

        store.get_or_insert_mut("game");
        store.save().unwrap();
        let reloaded = MetaInfoStore::load_from_path(&path);
        assert_eq!(reloaded.entries.len(), 1);
        // No temporary files are left behind by the atomic save.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
