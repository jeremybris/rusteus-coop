//! Central GameManager coordinator orchestrating game library, scanner, and active sessions.

use crate::scanner::{DiskScanner, FoundGame};
use crate::session::GameSession;
pub use crate::session::{ActiveSessionSlotSnapshot, ActiveSessionSnapshot};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct HubMetaSidecar {
    #[serde(default)]
    handler_id: Option<String>,
    #[serde(default)]
    package_id: Option<String>,
    #[serde(default)]
    installed_version: Option<i32>,
}

fn read_hubmeta_sidecar_for_js(js_path: &Path) -> Option<HubMetaSidecar> {
    let direct_sidecar = js_path.with_extension("hubmeta.json");
    if direct_sidecar.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&direct_sidecar) {
            if let Ok(meta) = serde_json::from_str::<HubMetaSidecar>(&raw) {
                return Some(meta);
            }
        }
    }
    if let (Some(parent), Some(stem)) = (js_path.parent(), js_path.file_stem()) {
        let companion_sidecar = parent.join(stem).join(".hubmeta.json");
        if companion_sidecar.is_file() {
            if let Ok(raw) = std::fs::read_to_string(&companion_sidecar) {
                if let Ok(meta) = serde_json::from_str::<HubMetaSidecar>(&raw) {
                    return Some(meta);
                }
            }
        }
    }
    None
}
use nucleus_common::error::NucleusResult;
use nucleus_hub::HubClient;
use nucleus_platform::PlatformServices;
use nucleus_profile::GameProfile;
use nucleus_script::HandlerConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Resolves the canonical staging path for a game session matching C# NucleusCoop:
/// 1. `<custom_nucleus_data_dir>/<GUID>` if configured.
/// 2. `<InstallRoot>/content/<GUID>` alongside executable or working dir (created if missing).
/// 3. `%APPDATA%/Nucleus Coop/<GUID>` (via `dirs::data_dir()`) as permission fallback.
/// Resolves a configured path against the installation folder when it is relative.
///
/// Settings store portable paths such as `handlers`. Those must not depend on the working
/// directory the launcher happened to be started from (a shortcut, a terminal, or an elevated
/// relaunch each use a different one).
pub fn resolve_install_relative(path: &Path) -> PathBuf {
    if path.is_absolute() || path.as_os_str().is_empty() {
        return path.to_path_buf();
    }
    match nucleus_common::Globals::try_get() {
        Some(globals) => globals.install_root.join(path),
        None => path.to_path_buf(),
    }
}

pub fn get_app_content_path(guid: &str, custom_nucleus_data_dir: Option<&Path>) -> PathBuf {
    // 1. Custom nucleus data directory from AppSettings
    if let Some(custom) = custom_nucleus_data_dir {
        if !custom.as_os_str().is_empty() {
            return custom.join(guid);
        }
    }

    // 2. Globals install root: <install_root>/content
    if let Some(globals) = nucleus_common::Globals::try_get() {
        let content = globals.install_root.join("content");
        if content.is_dir() || std::fs::create_dir_all(&content).is_ok() {
            return content.join(guid);
        }
    }

    // 3. Alongside the running executable: <exe_dir>/content
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let content = exe_dir.join("content");
            if content.is_dir() || std::fs::create_dir_all(&content).is_ok() {
                return content.join(guid);
            }
        }
    }

    // 4. Current working directory: ./content
    let cwd_content = PathBuf::from("content");
    if cwd_content.is_dir() || std::fs::create_dir_all(&cwd_content).is_ok() {
        return cwd_content.join(guid);
    }

    // 5. Well-known install directory if it exists
    let well_known_content = PathBuf::from(r"C:\Games\NucleusCoop242\content");
    if well_known_content.is_dir() {
        return well_known_content.join(guid);
    }

    // 6. Fallback to %APPDATA%\Nucleus Coop\<GUID> only if local content cannot be created
    if let Some(app_data) = dirs::data_dir() {
        return app_data.join("Nucleus Coop").join(guid);
    }

    // Ultimate fallback if AppData cannot be queried
    dirs::home_dir()
        .map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Nucleus Coop")
                .join(guid)
        })
        .unwrap_or_else(|| std::env::temp_dir().join("Nucleus Coop").join(guid))
}

/// Core manager orchestrating the game catalog, session execution, and platform integration.
pub struct GameManager {
    /// Platform services implementation.
    platform: Arc<dyn PlatformServices>,
    /// Community Hub API client.
    hub: HubClient,
    /// Disk scanner for executable detection.
    scanner: DiskScanner,
    /// Loaded handler configurations indexed by GUID.
    handlers: HashMap<String, HandlerConfig>,
    /// Handler script source code indexed by GUID.
    handler_scripts: HashMap<String, String>,
    /// User configured game profiles indexed by game ID.
    profiles: HashMap<String, GameProfile>,
    /// Currently active split-screen session, if any.
    active_session: Option<GameSession>,
    /// Directory containing JS game handlers.
    handlers_dir: std::path::PathBuf,
    /// Custom nucleus data directory from AppSettings if configured.
    nucleus_data_dir: Option<std::path::PathBuf>,
    /// Persistent metaInfo.json store matching C# %USERPROFILE%\NucleusCoop\metaInfo.json.
    meta_info: nucleus_profile::MetaInfoStore,
    /// Optional prompt handler for interactive multi-instance modal dialogues.
    prompt_handler: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    /// Optional Ludusavi CLI save manager driver.
    pub ludusavi_driver: Option<nucleus_tools::ludusavi::LudusaviDriver>,
    /// Central Extension & Plugin manager.
    pub extension_manager: Option<Arc<parking_lot::Mutex<nucleus_plugin::ExtensionManager>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct UserProfileJson {
    #[serde(rename = "Games", default)]
    pub games: Vec<UserGameEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserGameEntry {
    #[serde(rename = "GameGuid")]
    pub game_guid: String,
    #[serde(rename = "ExePath")]
    pub exe_path: String,
    #[serde(rename = "TotalPlayTimeSeconds", default)]
    pub total_play_time_seconds: u64,
    #[serde(rename = "LastPlayedTimestamp", default)]
    pub last_played_timestamp: Option<u64>,
    #[serde(rename = "Favorite", default)]
    pub favorite: bool,
    #[serde(rename = "PlayerCount", default = "default_entry_player_count")]
    pub player_count: usize,
    #[serde(rename = "Layout", default = "default_entry_layout")]
    pub layout: String,
    #[serde(rename = "CustomArguments", default)]
    pub custom_arguments: Option<String>,
    #[serde(rename = "Options", default)]
    pub options: HashMap<String, String>,
    #[serde(rename = "AssignGamepadsByIndex", default)]
    pub assign_gamepads_by_index: bool,
    #[serde(rename = "AssignGamepadsOnButtonPress", default)]
    pub assign_gamepads_on_button_press: bool,
    #[serde(rename = "KeepInstancesContentFolder", default)]
    pub keep_instances_content_folder: bool,
    #[serde(rename = "DisableProfile", default)]
    pub disable_profile: bool,
    #[serde(rename = "DisableHandlerUpdate", default)]
    pub disable_handler_update: bool,
    #[serde(rename = "CustomGameIcon", default)]
    pub custom_game_icon: Option<String>,
    #[serde(rename = "SteamLanguage", default)]
    pub steam_language: Option<String>,
}

fn default_entry_player_count() -> usize {
    2
}

fn default_entry_layout() -> String {
    "Auto".to_string()
}

impl GameManager {
    /// Creates a new `GameManager` instance using the provided platform services.
    pub fn new(platform: Arc<dyn PlatformServices>) -> NucleusResult<Self> {
        let ludusavi_driver = nucleus_tools::ludusavi::LudusaviDriver::discover(None).ok();

        let plugins_dir = if let Some(globals) = nucleus_common::Globals::try_get() {
            globals.app_data_path.join("plugins")
        } else {
            PathBuf::from("content").join("plugins")
        };
        let addons_dir = if let Some(globals) = nucleus_common::Globals::try_get() {
            globals.app_data_path.join("addons")
        } else {
            PathBuf::from("content").join("addons")
        };
        let perms_file = plugins_dir.join("permissions.json");
        let mut ext_mgr = nucleus_plugin::ExtensionManager::new(
            plugins_dir,
            addons_dir,
            perms_file,
            semver::Version::new(0, 2, 0),
        );
        let _ = ext_mgr.scan_all();
        let extension_manager = Some(Arc::new(parking_lot::Mutex::new(ext_mgr)));

        Ok(Self {
            platform,
            hub: HubClient::default(),
            scanner: DiskScanner::new(),
            handlers: HashMap::new(),
            handler_scripts: HashMap::new(),
            profiles: HashMap::new(),
            active_session: None,
            handlers_dir: std::path::PathBuf::from("handlers"),
            nucleus_data_dir: None,
            meta_info: nucleus_profile::MetaInfoStore::load_default(),
            prompt_handler: None,
            ludusavi_driver,
            extension_manager,
        })
    }

    /// Sets the interactive launch prompt handler.
    pub fn set_prompt_handler(&mut self, handler: Arc<dyn Fn(&str) -> bool + Send + Sync>) {
        self.prompt_handler = Some(handler);
    }

    /// Sets the custom nucleus data directory for session staging.
    pub fn set_nucleus_data_dir(&mut self, dir: impl Into<std::path::PathBuf>) {
        let p = dir.into();
        if !p.as_os_str().is_empty() {
            self.nucleus_data_dir = Some(p);
        } else {
            self.nucleus_data_dir = None;
        }
    }

    /// Returns the custom nucleus data directory if configured.
    pub fn nucleus_data_dir(&self) -> Option<&Path> {
        self.nucleus_data_dir.as_deref()
    }

    /// Sets the directory to scan for JavaScript handler scripts.
    pub fn set_handlers_dir(&mut self, path: impl Into<std::path::PathBuf>) {
        self.handlers_dir = resolve_install_relative(&path.into());
    }

    /// Returns the directory being scanned for JavaScript handler scripts.
    pub fn handlers_dir(&self) -> &Path {
        &self.handlers_dir
    }

    /// Reloads all handler configurations from `handlers_dir`.
    pub fn reload_handlers(&mut self) -> NucleusResult<()> {
        self.handlers.clear();
        self.handler_scripts.clear();
        self.initialize()
    }

    /// Registers a handler configuration and optional script source directly into the manager.
    pub fn register_handler(&mut self, config: HandlerConfig, script: Option<String>) {
        let guid = config.guid.clone();
        if let Some(src) = script {
            self.handler_scripts.insert(guid.clone(), src);
        }
        self.handlers.insert(guid, config);
    }

    /// Returns the canonical path to `userprofile.json` in the content directory.
    pub fn get_userprofile_path(&self) -> PathBuf {
        let content_dir = get_app_content_path("", self.nucleus_data_dir.as_deref());
        content_dir.join("userprofile.json")
    }

    /// Loads previously configured game profiles from `userprofile.json`.
    pub fn load_user_profile(&mut self) -> NucleusResult<()> {
        let path = self.get_userprofile_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(user_profile) = serde_json::from_str::<UserProfileJson>(&content) {
                    for entry in user_profile.games {
                        tracing::info!(
                            "Loaded persisted game profile for '{}' -> {}",
                            entry.game_guid,
                            entry.exe_path
                        );
                        let mut profile =
                            GameProfile::new(&entry.game_guid, &entry.game_guid, entry.exe_path);
                        profile.total_play_time_seconds = entry.total_play_time_seconds;
                        profile.last_played_timestamp = entry.last_played_timestamp;
                        profile.favorite = entry.favorite;
                        profile.player_count = entry.player_count;
                        profile.layout = entry.layout;
                        profile.custom_arguments = entry.custom_arguments;
                        profile.options = entry.options;
                        profile.assign_gamepads_by_index = entry.assign_gamepads_by_index;
                        profile.assign_gamepads_on_button_press =
                            entry.assign_gamepads_on_button_press;
                        profile.keep_instances_content_folder = entry.keep_instances_content_folder;
                        profile.disable_profile = entry.disable_profile;
                        profile.disable_handler_update = entry.disable_handler_update;
                        profile.custom_game_icon = entry.custom_game_icon;
                        profile.steam_language = entry.steam_language;
                        self.profiles.insert(entry.game_guid, profile);
                    }
                }
            }
        }

        // Cross-reference and sync with %USERPROFILE%\NucleusCoop\metaInfo.json
        for (guid, profile) in &mut self.profiles {
            let meta_entry = self
                .meta_info
                .find_entry(guid)
                .or_else(|| {
                    self.handlers
                        .get(guid)
                        .and_then(|h| self.meta_info.find_entry(&h.game_name))
                })
                .map(|(_, e)| e);

            if let Some(meta) = meta_entry {
                let meta_secs = meta.play_time_seconds();
                if meta_secs > 0 {
                    profile.total_play_time_seconds = meta_secs;
                }
                if let Some(fav) = meta.favorite {
                    profile.favorite = fav;
                }
                if let Some(b) = meta.disable_profiles {
                    profile.disable_profile = b;
                }
                if let Some(b) = meta.keep_sym_link {
                    profile.keep_instances_content_folder = b;
                }
                if let Some(b) = meta.use_api_index {
                    profile.assign_gamepads_by_index = b;
                }
                if let Some(b) = meta.profile_assign_gamepad_by_button_press {
                    profile.assign_gamepads_on_button_press = b;
                }
                if let Some(ref s) = meta.steam_language {
                    profile.steam_language = Some(s.clone());
                }
                if let Some(ref icon) = meta.icon_path {
                    profile.custom_game_icon = Some(icon.clone());
                }
            }
        }

        Ok(())
    }

    /// Persists all currently registered game profiles to `userprofile.json`.
    pub fn save_user_profile(&self) -> NucleusResult<()> {
        let path = self.get_userprofile_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut entries = Vec::new();
        for (guid, profile) in &self.profiles {
            entries.push(UserGameEntry {
                game_guid: guid.clone(),
                exe_path: profile.executable_path.clone(),
                total_play_time_seconds: profile.total_play_time_seconds,
                last_played_timestamp: profile.last_played_timestamp,
                favorite: profile.favorite,
                player_count: profile.player_count,
                layout: profile.layout.clone(),
                custom_arguments: profile.custom_arguments.clone(),
                options: profile.options.clone(),
                assign_gamepads_by_index: profile.assign_gamepads_by_index,
                assign_gamepads_on_button_press: profile.assign_gamepads_on_button_press,
                keep_instances_content_folder: profile.keep_instances_content_folder,
                disable_profile: profile.disable_profile,
                disable_handler_update: profile.disable_handler_update,
                custom_game_icon: profile.custom_game_icon.clone(),
                steam_language: profile.steam_language.clone(),
            });
        }
        let user_profile = UserProfileJson { games: entries };
        if let Ok(json_str) = serde_json::to_string_pretty(&user_profile) {
            let _ = std::fs::write(&path, json_str);
            tracing::info!(
                "Saved {} game profile(s) to {:?}",
                user_profile.games.len(),
                path
            );
        }
        Ok(())
    }

    /// Returns the directory where numbered game profiles are stored: `./game profiles/<guid>`.
    pub fn get_game_profiles_dir(&self, guid: &str) -> PathBuf {
        let base = if let Some(ref data_dir) = self.nucleus_data_dir {
            data_dir.join("game profiles")
        } else {
            PathBuf::from("game profiles")
        };
        base.join(guid)
    }

    /// Lists all saved numbered profiles for a game GUID: returns `Vec<(profile_id, title, GameProfile)>`.
    pub fn list_saved_profiles(&self, guid: &str) -> Vec<(usize, String, GameProfile)> {
        let dir = self.get_game_profiles_dir(guid);
        let mut result = Vec::new();
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if fname.starts_with("Profile[") && fname.ends_with("].json") {
                            let num_str = &fname[8..fname.len() - 6];
                            if let Ok(num) = num_str.parse::<usize>() {
                                if let Ok(profile) = GameProfile::load_from_file(&path) {
                                    let title = profile
                                        .profile_name
                                        .as_deref()
                                        .filter(|s| !s.trim().is_empty())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| format!("Profile {}", num));
                                    result.push((num, title, profile));
                                }
                            }
                        }
                    }
                }
            }
        }
        result.sort_by_key(|(num, _, _)| *num);
        result
    }

    /// Saves a numbered game profile to `game profiles/<guid>/Profile[N].json`.
    /// If `profile_number` is `None`, saves as a new profile (N = max + 1).
    pub fn save_numbered_profile(
        &mut self,
        guid: &str,
        profile_number: Option<usize>,
        profile: &GameProfile,
    ) -> NucleusResult<usize> {
        let dir = self.get_game_profiles_dir(guid);
        let _ = std::fs::create_dir_all(&dir);

        let target_num = match profile_number {
            Some(n) => n,
            None => {
                let existing = self.list_saved_profiles(guid);
                existing.iter().map(|(num, _, _)| *num).max().unwrap_or(0) + 1
            }
        };

        let file_path = dir.join(format!("Profile[{}].json", target_num));
        let mut to_save = profile.clone();
        if to_save
            .profile_name
            .as_deref()
            .map_or(true, |s| s.trim().is_empty())
        {
            to_save.profile_name = Some(format!("Profile {}", target_num));
        }
        to_save.save_to_file(&file_path)?;

        self.platform
            .osd_manager()
            .show(1600, "Handler Profile Saved. Setup Finished");
        Ok(target_num)
    }

    /// Loads a numbered game profile from `game profiles/<guid>/Profile[N].json`.
    pub fn load_numbered_profile(
        &mut self,
        guid: &str,
        profile_number: usize,
    ) -> NucleusResult<GameProfile> {
        let dir = self.get_game_profiles_dir(guid);
        let file_path = dir.join(format!("Profile[{}].json", profile_number));
        let profile = GameProfile::load_from_file(&file_path)?;

        let title = profile.profile_name.as_deref().unwrap_or("");
        let msg = if !title.is_empty() {
            format!("Handler Profile \"{}\" Loaded", title)
        } else {
            format!("Handler Profile N°{} Loaded", profile_number)
        };
        self.platform.osd_manager().show(1000, &msg);
        Ok(profile)
    }

    /// Deletes a numbered game profile `game profiles/<guid>/Profile[N].json`.
    pub fn delete_numbered_profile(
        &mut self,
        guid: &str,
        profile_number: usize,
    ) -> NucleusResult<()> {
        let dir = self.get_game_profiles_dir(guid);
        let file_path = dir.join(format!("Profile[{}].json", profile_number));
        if file_path.exists() {
            let _ = std::fs::remove_file(&file_path);
        }
        self.platform
            .osd_manager()
            .show(500, "Handler Profile Deleted");
        Ok(())
    }

    /// Unloads the active profile and displays OSD notification.
    pub fn unload_profile(&self) {
        self.platform
            .osd_manager()
            .show(500, "Handler Profile Unloaded");
    }

    /// Saves an updated profile in memory and persists to disk.
    pub fn save_game_profile(&mut self, guid: &str, profile: &GameProfile) -> NucleusResult<()> {
        self.profiles.insert(guid.to_string(), profile.clone());
        let _ = self.save_user_profile();

        // Also persist to %USERPROFILE%\NucleusCoop\metaInfo.json
        let entry = self.meta_info.get_or_insert_mut(guid);
        entry.set_play_time_seconds(profile.total_play_time_seconds);
        entry.favorite = Some(profile.favorite);
        entry.disable_profiles = Some(profile.disable_profile);
        entry.keep_sym_link = Some(profile.keep_instances_content_folder);
        entry.use_api_index = Some(profile.assign_gamepads_by_index);
        entry.profile_assign_gamepad_by_button_press =
            Some(profile.assign_gamepads_on_button_press);
        if let Some(ref l) = profile.steam_language {
            entry.steam_language = Some(l.clone());
        }
        if let Some(ref icon) = profile.custom_game_icon {
            entry.icon_path = Some(icon.clone());
        }
        let _ = self.meta_info.save();

        Ok(())
    }

    /// Deletes a game handler and associated files from disk, userprofile.json, and metaInfo.json.
    pub fn delete_game_handler(&mut self, guid_or_name: &str) -> NucleusResult<()> {
        let actual_guid = if self.handlers.contains_key(guid_or_name) {
            guid_or_name.to_string()
        } else if let Some((g, _)) = self.handlers.iter().find(|(g, h)| {
            g.eq_ignore_ascii_case(guid_or_name)
                || h.game_name.eq_ignore_ascii_case(guid_or_name)
                || h.hub_handler_id.as_deref().map_or(false, |hid| hid.eq_ignore_ascii_case(guid_or_name))
        }) {
            g.clone()
        } else {
            guid_or_name.to_string()
        };
        let guid = &actual_guid;
        tracing::info!("Deleting game handler for '{}' (resolved guid: '{}')...", guid_or_name, guid);

        // 1. Delete handler script (.js) & .hubmeta.json sidecar from disk
        if let Some(h) = self.handlers.get(guid) {
            if let Some(ref path_str) = h.handler_path {
                let p = Path::new(path_str);
                if p.is_file() {
                    let _ = std::fs::remove_file(p);
                    tracing::info!("Deleted handler script file: {:?}", p);
                }
                let sidecar = p.with_extension("hubmeta.json");
                if sidecar.is_file() {
                    let _ = std::fs::remove_file(&sidecar);
                }
                if let (Some(parent), Some(stem)) = (p.parent(), p.file_stem()) {
                    let companion = parent.join(stem).join(".hubmeta.json");
                    if companion.is_file() {
                        let _ = std::fs::remove_file(&companion);
                    }
                }
            }
        }
        let direct_js = self.handlers_dir.join(format!("{}.js", guid));
        if direct_js.is_file() {
            let _ = std::fs::remove_file(&direct_js);
            tracing::info!("Deleted handler script file: {:?}", direct_js);
        }
        let direct_sidecar = direct_js.with_extension("hubmeta.json");
        if direct_sidecar.is_file() {
            let _ = std::fs::remove_file(&direct_sidecar);
        }

        // 2. Remove from in-memory maps
        self.handlers.remove(guid);
        self.handler_scripts.remove(guid);

        // 3. Remove from userprofile.json
        self.profiles.remove(guid);
        let _ = self.save_user_profile();

        // 4. Remove from metaInfo.json
        self.meta_info.remove_game(guid);
        let _ = self.meta_info.save();

        // 5. Delete game profiles directory: ./game profiles/<guid>
        let prof_dir = self.get_game_profiles_dir(guid);
        if prof_dir.exists() {
            let _ = std::fs::remove_dir_all(&prof_dir);
            tracing::info!("Deleted game profiles folder: {:?}", prof_dir);
        }

        // 6. Delete instanced content directory: ./content/<guid>
        let content_dir = get_app_content_path(guid, self.nucleus_data_dir.as_deref());
        if content_dir.exists() {
            let _ = std::fs::remove_dir_all(&content_dir);
            tracing::info!("Deleted content directory: {:?}", content_dir);
        }

        Ok(())
    }

    /// Returns a reference to the loaded %USERPROFILE%\NucleusCoop\metaInfo.json store.
    pub fn meta_info(&self) -> &nucleus_profile::MetaInfoStore {
        &self.meta_info
    }

    /// Returns a mutable reference to the %USERPROFILE%\NucleusCoop\metaInfo.json store.
    pub fn meta_info_mut(&mut self) -> &mut nucleus_profile::MetaInfoStore {
        &mut self.meta_info
    }

    /// Initializes the engine, scanning installed handlers from `handlers_dir` and loading `userprofile.json`.
    pub fn initialize(&mut self) -> NucleusResult<()> {
        if self.handlers_dir.exists() && self.handlers_dir.is_dir() {
            self.load_handlers_from_dir(&self.handlers_dir.clone());
        }
        self.meta_info = nucleus_profile::MetaInfoStore::load_default();
        let _ = self.load_user_profile();
        Ok(())
    }

    fn load_handlers_from_dir(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if file_name.eq_ignore_ascii_case("MasterHandler.js") {
                        continue;
                    }
                    if path
                        .extension()
                        .map_or(false, |ext| ext.eq_ignore_ascii_case("js"))
                    {
                        if let Ok(script) = std::fs::read_to_string(&path) {
                            let mut engine = match nucleus_script::ScriptEngine::new() {
                                Ok(e) => e,
                                Err(_) => continue,
                            };
                            if let Ok(mut config) =
                                engine.load_handler_script_with_folder(&script, path.parent())
                            {
                                if config.guid.eq_ignore_ascii_case("My Game")
                                    || config.game_name.eq_ignore_ascii_case("My Game")
                                    || config.guid.eq_ignore_ascii_case("MasterHandler")
                                {
                                    continue;
                                }
                                config.handler_path = Some(path.to_string_lossy().to_string());
                                if let Some(meta) = read_hubmeta_sidecar_for_js(&path) {
                                    if config.hub_handler_id.is_none() {
                                        if let Some(hid) = meta.handler_id {
                                            if !hid.trim().is_empty() {
                                                config.hub_handler_id = Some(hid);
                                            }
                                        }
                                    }
                                    if let Some(ver) = meta.installed_version {
                                        config.hub_handler_version = Some(ver);
                                    }
                                    let _ = meta.package_id;
                                }
                                if !config.guid.is_empty() {
                                    self.register_handler(config, Some(script));
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    self.load_handlers_from_dir(&path);
                }
            }
        }
    }

    /// Adds a game by inspecting an executable path and associating it with a matching handler.
    pub fn add_game(&mut self, exe_path: &Path) -> NucleusResult<Option<String>> {
        let file_name = exe_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let stem = exe_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut matched_guid: Option<String> = None;

        for (guid, config) in &self.handlers {
            if !config.executable_name.is_empty()
                && config.executable_name.eq_ignore_ascii_case(&file_name)
            {
                matched_guid = Some(guid.clone());
                break;
            }
            if !config.game_name.is_empty() && config.game_name.eq_ignore_ascii_case(&stem) {
                matched_guid = Some(guid.clone());
                break;
            }
        }

        if let Some(ref guid) = matched_guid {
            let profile = GameProfile::new(guid, guid, exe_path.to_string_lossy().to_string());
            self.profiles.insert(guid.clone(), profile);
            let _ = self.save_user_profile();
        }

        Ok(matched_guid)
    }

    /// Starts a split-screen gaming session for the specified handler GUID and profile.
    pub fn start_game(&mut self, guid: &str, profile: GameProfile) -> NucleusResult<()> {
        self.stop_game()?;

        let mut config = self.handlers.get(guid).cloned().ok_or_else(|| {
            nucleus_common::error::NucleusError::Other(format!(
                "Handler not found for GUID: {}",
                guid
            ))
        })?;

        // Apply profile emulator preference & PeerParty room override
        if profile.preferred_emulator == "PeerParty" {
            config.use_peerparty = true;
        } else if profile.preferred_emulator == "Goldberg" {
            config.use_peerparty = false;
            config.use_goldberg = true;
        }
        if let Some(ref room) = profile.peerparty_room_override {
            config.peerparty_room = Some(room.clone());
        }

        let exe_path = std::path::PathBuf::from(&profile.executable_path);
        let game_dir = exe_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let staging_dir = get_app_content_path(guid, self.nucleus_data_dir.as_deref());
        if let Err(e) = std::fs::create_dir_all(&staging_dir) {
            tracing::warn!(
                "Failed to create staging directory {:?}: {}",
                staging_dir,
                e
            );
        }
        let player_count = std::cmp::max(1, profile.player_count);

        let split_mode = nucleus_handler::window::split_mode_from_name(&profile.layout);

        let mut profile = profile;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        profile.last_played_timestamp = Some(now);
        self.profiles.insert(guid.to_string(), profile.clone());
        let _ = self.save_user_profile();

        let now_str = nucleus_profile::now_formatted();
        let entry = self.meta_info.get_or_insert_mut(guid);
        entry.last_played_at = Some(now_str.clone());
        entry.first_launch = Some(false);
        if let Some(h) = self.handlers.get(guid) {
            if h.game_name != guid {
                let entry2 = self.meta_info.get_or_insert_mut(&h.game_name);
                entry2.last_played_at = Some(now_str);
                entry2.first_launch = Some(false);
            }
        }
        let _ = self.meta_info.save();

        let mut handler = nucleus_handler::GameHandler::new(
            config,
            game_dir,
            exe_path,
            staging_dir,
            player_count,
        )
        .with_platform(self.platform.clone())
        .with_split_mode(split_mode)
        .with_layout(profile.layout.clone())
        .with_options(profile.options.clone())
        .with_player_device_assignments(profile.player_device_assignments.clone())
        .with_player_monitor_assignments(profile.player_monitor_assignments.clone())
        .with_player_audio_device_assignments(profile.player_audio_device_assignments.clone())
        .with_ludusavi(
            self.ludusavi_driver.clone(),
            profile.player_save_slots.clone(),
            profile.ludusavi_auto_sync,
        );

        if let Some(ref ext_mgr) = self.extension_manager {
            handler = handler.with_extension_manager(ext_mgr.clone());
        }

        if let Some(script) = self.handler_scripts.get(guid) {
            handler = handler.with_script_source(script.clone());
        }

        if let Some(ref ph) = self.prompt_handler {
            handler = handler.with_prompt_handler(ph.clone());
        }

        let mut session = GameSession::new(format!("session_{}", guid), guid, profile);
        session.set_handler(handler);
        session.start()?;

        self.active_session = Some(session);
        Ok(())
    }

    /// Stops the currently active game session.
    pub fn stop_game(&mut self) -> NucleusResult<()> {
        if let Some(mut session) = self.active_session.take() {
            session.stop()?;
            if session.pending_playtime_seconds > 0 {
                let delta = session.pending_playtime_seconds;
                session.pending_playtime_seconds = 0;
                if let Some(prof) = self.profiles.get_mut(&session.game_guid) {
                    prof.total_play_time_seconds += delta;
                    prof.last_played_timestamp = session.profile.last_played_timestamp;
                    let _ = self.save_user_profile();
                }

                let now_str = nucleus_profile::now_formatted();
                self.meta_info
                    .record_playtime(&session.game_guid, delta, &now_str);
                if let Some(h) = self.handlers.get(&session.game_guid) {
                    if h.game_name != session.game_guid {
                        self.meta_info
                            .record_playtime(&h.game_name, delta, &now_str);
                    }
                }
                let _ = self.meta_info.save();
            }
        }
        Ok(())
    }

    /// Starts a split-screen gaming session attached to a step-by-step debug controller.
    pub fn start_game_with_debugger(
        &mut self,
        guid: &str,
        profile: GameProfile,
        controller: nucleus_debug::DebugController,
    ) -> NucleusResult<()> {
        self.stop_game()?;

        let mut config = self.handlers.get(guid).cloned().ok_or_else(|| {
            nucleus_common::error::NucleusError::Other(format!(
                "Handler not found for GUID: {}",
                guid
            ))
        })?;

        // Apply profile emulator preference & PeerParty room override
        if profile.preferred_emulator == "PeerParty" {
            config.use_peerparty = true;
        } else if profile.preferred_emulator == "Goldberg" {
            config.use_peerparty = false;
            config.use_goldberg = true;
        }
        if let Some(ref room) = profile.peerparty_room_override {
            config.peerparty_room = Some(room.clone());
        }

        let exe_path = std::path::PathBuf::from(&profile.executable_path);
        let game_dir = exe_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let staging_dir = get_app_content_path(guid, self.nucleus_data_dir.as_deref());
        if let Err(e) = std::fs::create_dir_all(&staging_dir) {
            tracing::warn!(
                "Failed to create staging directory {:?}: {}",
                staging_dir,
                e
            );
        }
        let player_count = std::cmp::max(1, profile.player_count);
        let split_mode = nucleus_handler::window::split_mode_from_name(&profile.layout);

        let mut profile = profile;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        profile.last_played_timestamp = Some(now);
        self.profiles.insert(guid.to_string(), profile.clone());
        let _ = self.save_user_profile();

        let mut handler = nucleus_handler::GameHandler::new(
            config,
            game_dir,
            exe_path,
            staging_dir,
            player_count,
        )
        .with_platform(self.platform.clone())
        .with_split_mode(split_mode)
        .with_layout(profile.layout.clone())
        .with_options(profile.options.clone())
        .with_player_device_assignments(profile.player_device_assignments.clone())
        .with_player_monitor_assignments(profile.player_monitor_assignments.clone())
        .with_player_audio_device_assignments(profile.player_audio_device_assignments.clone())
        .with_ludusavi(
            self.ludusavi_driver.clone(),
            profile.player_save_slots.clone(),
            profile.ludusavi_auto_sync,
        )
        .with_debug_controller(controller);

        if let Some(ref ext_mgr) = self.extension_manager {
            handler = handler.with_extension_manager(ext_mgr.clone());
        }

        if let Some(script) = self.handler_scripts.get(guid) {
            handler = handler.with_script_source(script.clone());
        }

        if let Some(ref ph) = self.prompt_handler {
            handler = handler.with_prompt_handler(ph.clone());
        }

        let mut session = GameSession::new(format!("session_{}", guid), guid, profile);
        session.set_handler(handler);
        session.start()?;

        self.active_session = Some(session);
        Ok(())
    }

    /// Returns the root save directory: `<data_dir>/saves`.
    pub fn get_saves_dir(&self) -> PathBuf {
        if let Some(ref data_dir) = self.nucleus_data_dir {
            data_dir.join("saves")
        } else if let Some(globals) = nucleus_common::Globals::try_get() {
            globals.app_data_path.join("saves")
        } else {
            PathBuf::from("content").join("saves")
        }
    }

    /// Returns the save slots manifest path for a game: `<data_dir>/saves/<guid>/manifest.json`.
    pub fn get_save_slots_manifest_path(&self, guid: &str) -> PathBuf {
        nucleus_profile::save_slots::GameSaveSlotsManifest::manifest_path(
            &self.get_saves_dir(),
            guid,
        )
    }

    /// Lists all save slots configured for the given game GUID.
    pub fn list_save_slots(
        &self,
        guid: &str,
    ) -> NucleusResult<Vec<nucleus_profile::save_slots::SaveSlot>> {
        let saves_dir = self.get_saves_dir();
        let manifest =
            nucleus_profile::save_slots::GameSaveSlotsManifest::load_from_dir(&saves_dir, guid)?;
        Ok(manifest.slots)
    }

    /// Creates a new save slot for a game.
    pub fn create_save_slot(
        &mut self,
        guid: &str,
        display_name: &str,
        ludusavi_game_title: &str,
        source_hint: &str,
    ) -> NucleusResult<nucleus_profile::save_slots::SaveSlot> {
        let saves_dir = self.get_saves_dir();
        let mut manifest =
            nucleus_profile::save_slots::GameSaveSlotsManifest::load_from_dir(&saves_dir, guid)?;
        let slot = manifest
            .create_slot(display_name, ludusavi_game_title, source_hint)
            .clone();
        manifest.save_to_dir(&saves_dir)?;
        Ok(slot)
    }

    /// Clones an existing save slot to a new slot name, replicating backup files on disk.
    pub fn clone_save_slot(
        &mut self,
        guid: &str,
        source_id: &str,
        new_name: &str,
    ) -> NucleusResult<nucleus_profile::save_slots::SaveSlot> {
        let saves_dir = self.get_saves_dir();
        let mut manifest =
            nucleus_profile::save_slots::GameSaveSlotsManifest::load_from_dir(&saves_dir, guid)?;
        let (title, hint) = if let Some(source_slot) = manifest.get_slot(source_id) {
            (
                source_slot.ludusavi_game_title.clone(),
                source_slot.source_hint.clone(),
            )
        } else {
            return Err(nucleus_common::error::NucleusError::Other(format!(
                "Slot '{}' not found",
                source_id
            )));
        };
        let new_slot = manifest
            .create_slot(new_name, &title, &format!("{}_clone", hint))
            .clone();

        // Replicate slot repository folder on disk if it exists
        let game_dir = saves_dir.join(guid);
        let src_dir = game_dir.join(source_id);
        let dst_dir = game_dir.join(&new_slot.slot_id);
        if src_dir.is_dir() {
            let _ = nucleus_common::file_util::copy_directory(&src_dir, &dst_dir, true);
        }

        manifest.save_to_dir(&saves_dir)?;
        Ok(new_slot)
    }

    /// Deletes a save slot and removes its backup files from disk.
    pub fn delete_save_slot(&mut self, guid: &str, slot_id: &str) -> NucleusResult<()> {
        let saves_dir = self.get_saves_dir();
        let mut manifest =
            nucleus_profile::save_slots::GameSaveSlotsManifest::load_from_dir(&saves_dir, guid)?;
        manifest.delete_slot(slot_id);
        manifest.save_to_dir(&saves_dir)?;

        let slot_dir = saves_dir.join(guid).join(slot_id);
        if slot_dir.exists() {
            let _ = std::fs::remove_dir_all(&slot_dir);
        }

        Ok(())
    }

    /// Sets the active save slot for a player slot in a game profile.
    pub fn set_player_save_slot(
        &mut self,
        guid: &str,
        player_index: usize,
        slot_id: Option<String>,
    ) -> NucleusResult<()> {
        if let Some(profile) = self.profiles.get_mut(guid) {
            while profile.player_save_slots.len() <= player_index {
                profile.player_save_slots.push(None);
            }
            profile.player_save_slots[player_index] = slot_id;
            self.save_user_profile()?;
        }
        Ok(())
    }

    /// Returns a reference to the ExtensionManager if available.
    pub fn extension_manager(
        &self,
    ) -> Option<Arc<parking_lot::Mutex<nucleus_plugin::ExtensionManager>>> {
        self.extension_manager.clone()
    }

    /// Installs an extension archive (.nucplugin or .nucaddon).
    pub fn install_extension(&mut self, archive_path: &Path) -> NucleusResult<String> {
        if let Some(ref ext_mgr) = self.extension_manager {
            ext_mgr.lock().install_archive(archive_path)
        } else {
            Err(nucleus_common::error::NucleusError::Plugin(
                "Extension manager not initialized".to_string(),
            ))
        }
    }

    /// Retrieves saved Hub session credentials using platform credential store.
    pub fn get_hub_credentials(&self) -> Option<nucleus_hub::auth::HubSessionToken> {
        let store = self.platform.credential_store();
        if let Ok(Some(bytes)) = store.load_secret("splitscreen_hub", "session") {
            if let Ok(creds) = serde_json::from_slice::<nucleus_hub::auth::HubSessionToken>(&bytes)
            {
                return Some(creds);
            }
        }
        None
    }

    /// Saves Hub session credentials using platform credential store.
    pub fn save_hub_credentials(
        &self,
        creds: &nucleus_hub::auth::HubSessionToken,
    ) -> NucleusResult<()> {
        let store = self.platform.credential_store();
        let bytes = serde_json::to_vec(creds).map_err(nucleus_common::error::NucleusError::Json)?;
        store.save_secret("splitscreen_hub", "session", &bytes)
    }

    /// Clears saved Hub session credentials.
    pub fn clear_hub_credentials(&self) -> NucleusResult<()> {
        let store = self.platform.credential_store();
        store.delete_secret("splitscreen_hub", "session")
    }

    /// Scans local storage drives for supported game installations.
    /// Cross-references found executables against loaded handler configs.
    pub fn scan_disks(&mut self) -> NucleusResult<Vec<FoundGame>> {
        self.scanner.load_handlers(&self.handlers);
        self.scanner.scan_drives()
    }

    /// Scans specific paths for game executables, cross-referencing handlers.
    pub fn scan_paths(&mut self, paths: &[&std::path::Path]) -> NucleusResult<Vec<FoundGame>> {
        self.scanner.load_handlers(&self.handlers);
        self.scanner.scan(paths)
    }

    /// Returns a reference to the active session, if one is currently running.
    pub fn active_session(&self) -> Option<&GameSession> {
        self.active_session.as_ref()
    }

    /// Returns a mutable reference to the active session, if one is currently running.
    pub fn active_session_mut(&mut self) -> Option<&mut GameSession> {
        self.active_session.as_mut()
    }

    /// Updates the In-Game OSD visibility flag on the active session.
    pub fn set_osd_visible(&mut self, visible: bool) {
        if let Some(session) = &mut self.active_session {
            session.is_osd_visible = visible;
        }
    }

    /// Returns a reference to the Community Hub client.
    pub fn hub(&self) -> &HubClient {
        &self.hub
    }

    /// Returns a reference to the platform services.
    pub fn platform(&self) -> &Arc<dyn PlatformServices> {
        &self.platform
    }

    /// Returns a registered handler configuration by its GUID.
    pub fn get_handler(&self, guid: &str) -> Option<&HandlerConfig> {
        self.handlers.get(guid)
    }

    /// Returns all registered handler configurations.
    pub fn get_handlers(&self) -> &HashMap<String, HandlerConfig> {
        &self.handlers
    }

    /// Returns all registered game profiles.
    pub fn profiles(&self) -> &HashMap<String, GameProfile> {
        &self.profiles
    }

    /// Returns a reference to a registered game profile by game GUID, with fallback to case-insensitive and handler name matching.
    pub fn get_profile(&self, guid: &str) -> Option<&GameProfile> {
        if let Some(p) = self.profiles.get(guid) {
            return Some(p);
        }
        for (k, v) in &self.profiles {
            if k.eq_ignore_ascii_case(guid) || v.handler_guid.eq_ignore_ascii_case(guid) {
                return Some(v);
            }
        }
        if let Some(h) = self.handlers.get(guid) {
            for v in self.profiles.values() {
                if v.handler_guid.eq_ignore_ascii_case(&h.game_name)
                    || v.handler_guid.eq_ignore_ascii_case(&h.executable_name)
                {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Sets or updates a game profile.
    pub fn set_profile(&mut self, guid: String, profile: GameProfile) {
        self.profiles.insert(guid, profile);
    }

    /// Reads the raw handler JavaScript source code for a game by GUID or game name.
    pub fn read_handler_script(&self, guid_or_name: &str) -> Result<String, String> {
        if let Some(prof) = self.get_profile(guid_or_name) {
            let path = std::path::Path::new(&prof.handler_path);
            if path.exists() {
                return std::fs::read_to_string(path).map_err(|e| e.to_string());
            }
        }
        let candidate = self.handlers_dir.join(format!("{}.js", guid_or_name));
        if candidate.exists() {
            return std::fs::read_to_string(&candidate).map_err(|e| e.to_string());
        }
        if let Ok(entries) = std::fs::read_dir(&self.handlers_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map_or(false, |ext| ext == "js") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        if stem.eq_ignore_ascii_case(guid_or_name) {
                            return std::fs::read_to_string(&p).map_err(|e| e.to_string());
                        }
                    }
                }
            }
        }
        Err(format!("Handler script not found for '{}'", guid_or_name))
    }

    /// Saves updated handler JavaScript source code to disk.
    pub fn save_handler_script(&self, guid_or_name: &str, content: &str) -> Result<(), String> {
        let target_path = if let Some(prof) = self.get_profile(guid_or_name) {
            if !prof.handler_path.is_empty() {
                std::path::PathBuf::from(&prof.handler_path)
            } else {
                self.handlers_dir.join(format!("{}.js", guid_or_name))
            }
        } else {
            self.handlers_dir.join(format!("{}.js", guid_or_name))
        };
        std::fs::write(&target_path, content).map_err(|e| e.to_string())
    }

    /// Ticks the currently active session, re-enforcing window bounds, checking process liveness,
    /// and auto-flushing periodic playtime every 60 seconds (parity with C# SaveUserProfile timer).
    pub fn tick(&mut self) -> NucleusResult<()> {
        let flush_data = if let Some(ref mut session) = self.active_session {
            session.tick()?;
            if session.pending_playtime_seconds > 0 {
                let delta = session.pending_playtime_seconds;
                session.pending_playtime_seconds = 0;
                Some((
                    session.game_guid.clone(),
                    delta,
                    session.profile.last_played_timestamp,
                ))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((guid, delta, last_played)) = flush_data {
            if let Some(prof) = self.profiles.get_mut(&guid) {
                prof.total_play_time_seconds += delta;
                prof.last_played_timestamp = last_played;
                let _ = self.save_user_profile();
            }

            let now_str = nucleus_profile::now_formatted();
            self.meta_info.record_playtime(&guid, delta, &now_str);
            if let Some(h) = self.handlers.get(&guid) {
                if h.game_name != guid {
                    self.meta_info
                        .record_playtime(&h.game_name, delta, &now_str);
                }
            }
            let _ = self.meta_info.save();
        }

        // Re-enforce active Cheat Engine parity frozen memory values across all instances
        nucleus_hook::memory_engine::MemoryEngine::apply_frozen_slots();

        Ok(())
    }

    /// Switches/cycles the splitscreen layout of active instances during a session.
    pub fn switch_layout(&mut self) -> NucleusResult<Option<String>> {
        if let Some(ref mut session) = self.active_session {
            session.switch_layout()
        } else {
            Ok(None)
        }
    }

    /// Toggles minimizing or restoring all active game windows (Ctrl+T parity).
    pub fn toggle_minimize_windows(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut session) = self.active_session {
            session.toggle_minimize_windows()
        } else {
            Ok(None)
        }
    }

    /// Toggles cutscenes presentation mode (Ctrl+M parity).
    pub fn toggle_cutscenes_mode(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut session) = self.active_session {
            session.toggle_cutscenes_mode()
        } else {
            Ok(None)
        }
    }

    /// Freezes (suspends) all running game processes in the active session.
    pub fn freeze_session(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut session) = self.active_session {
            session.freeze_all()?;
            self.platform
                .osd_manager()
                .show(1500, "Instances Frozen (NtSuspendProcess)");
            Ok(Some(true))
        } else {
            Ok(None)
        }
    }

    /// Resumes (unfreezes) all running game processes in the active session.
    pub fn unfreeze_session(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut session) = self.active_session {
            session.unfreeze_all()?;
            self.platform
                .osd_manager()
                .show(1500, "Instances Resumed (NtResumeProcess)");
            Ok(Some(false))
        } else {
            Ok(None)
        }
    }

    /// Toggles freeze / resume for all instances in the active session.
    pub fn toggle_freeze_session(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut session) = self.active_session {
            if session.is_frozen() {
                session.unfreeze_all()?;
                self.platform
                    .osd_manager()
                    .show(1500, "Instances Resumed (NtResumeProcess)");
                Ok(Some(false))
            } else {
                session.freeze_all()?;
                self.platform
                    .osd_manager()
                    .show(1500, "Instances Frozen (NtSuspendProcess)");
                Ok(Some(true))
            }
        } else {
            Ok(None)
        }
    }

    /// Reassigns the controller device ID for a specific player slot in the active session.
    pub fn reassign_session_controller(
        &mut self,
        slot_idx: usize,
        new_device_id: usize,
    ) -> NucleusResult<()> {
        if let Some(ref mut session) = self.active_session {
            session.reassign_controller(slot_idx, new_device_id)?;
            let msg = format!(
                "Player {} Controller Reassigned to Device #{}",
                slot_idx + 1,
                new_device_id
            );
            self.platform.osd_manager().show(1500, &msg);
        }
        Ok(())
    }

    /// Re-enforces window boundaries and positions for all instances in the active session.
    pub fn re_enforce_session_windows(&mut self) -> NucleusResult<()> {
        if let Some(ref mut session) = self.active_session {
            session.re_enforce_windows()?;
            self.platform
                .osd_manager()
                .show(1200, "Window Bounds Re-enforced");
        }
        Ok(())
    }

    /// Drops out an active player slot mid-session and re-tiles surviving instances.
    pub fn drop_out_session_player(
        &mut self,
        slot_idx: usize,
        new_layout_name: &str,
    ) -> NucleusResult<usize> {
        let session = self.active_session.as_mut().ok_or_else(|| {
            nucleus_common::error::NucleusError::Other("No active game session".to_string())
        })?;
        let remaining = session.drop_out_player(slot_idx, new_layout_name)?;
        self.platform.osd_manager().show(
            1600,
            &format!(
                "Player {} Dropped Out ({} Remaining)",
                slot_idx + 1,
                remaining
            ),
        );
        Ok(remaining)
    }

    /// Drops in a new player mid-session into the lowest vacant slot and re-tiles active windows.
    pub fn drop_in_session_player(
        &mut self,
        device_assignment: &str,
        new_layout_name: &str,
    ) -> NucleusResult<usize> {
        let session = self.active_session.as_mut().ok_or_else(|| {
            nucleus_common::error::NucleusError::Other("No active game session".to_string())
        })?;
        let slot_idx = session.drop_in_player(device_assignment, new_layout_name)?;
        self.platform.osd_manager().show(
            1600,
            &format!("Player {} Dropped In ({})", slot_idx + 1, new_layout_name),
        );
        Ok(slot_idx)
    }

    /// Changes the active splitscreen layout explicitly and re-tiles active instances.
    pub fn change_session_layout(&mut self, new_layout_name: &str) -> NucleusResult<String> {
        let session = self.active_session.as_mut().ok_or_else(|| {
            nucleus_common::error::NucleusError::Other("No active game session".to_string())
        })?;
        let applied = session.change_layout(new_layout_name)?;
        self.platform
            .osd_manager()
            .show(1500, &format!("Layout: {}", applied));
        Ok(applied)
    }

    /// Returns a live snapshot of all active slots in the running session for the Couch OSD.
    pub fn active_session_snapshot(&self) -> Option<ActiveSessionSnapshot> {
        let session = self.active_session.as_ref()?;
        let game_name = self
            .handlers
            .get(&session.game_guid)
            .map(|h| h.game_name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| session.game_guid.clone());
        Some(session.active_snapshot(&game_name))
    }

    /// Deletes the UserProfile save data directory for the given handler GUID (%USERPROFILE%\NucleusCoop\<GameName>).
    pub fn delete_user_save_path(&self, guid: &str) -> NucleusResult<()> {
        let game_name = self
            .handlers
            .get(guid)
            .map(|h| h.game_name.clone())
            .unwrap_or_else(|| guid.to_string());
        if let Some(home) = dirs::home_dir() {
            let user_save_dir = home.join("NucleusCoop").join(&game_name);
            if user_save_dir.exists() {
                let _ = std::fs::remove_dir_all(&user_save_dir);
                tracing::info!("Deleted UserProfile save path: {:?}", user_save_dir);
            }
        }
        self.platform
            .osd_manager()
            .show(1600, "UserProfile Save Path Deleted");
        Ok(())
    }

    /// Removes a game entry and its profile configuration from the library.
    /// Removes a game handler and deletes its files from disk, userprofile, and metaInfo.
    pub fn remove_game(&mut self, guid: &str) -> NucleusResult<()> {
        self.delete_game_handler(guid)?;
        self.platform
            .osd_manager()
            .show(1600, "Game Removed From List");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleus_platform::OsdManager;

    #[derive(Default)]
    struct MockOsdManager {
        last_message: std::sync::Mutex<Option<(u32, String)>>,
    }

    impl nucleus_platform::OsdManager for MockOsdManager {
        fn show(&self, duration_ms: u32, text: &str) {
            *self.last_message.lock().unwrap() = Some((duration_ms, text.to_string()));
        }
        fn hide(&self) {
            *self.last_message.lock().unwrap() = None;
        }
        fn current_message(&self) -> Option<String> {
            self.last_message
                .lock()
                .unwrap()
                .as_ref()
                .map(|(_, t)| t.clone())
        }
        fn set_color(&self, _color: &str) {}
    }

    #[derive(Default)]
    struct MockPlatform {
        osd: MockOsdManager,
        cred: nucleus_platform::credentials::InMemoryCredentialStore,
    }
    impl PlatformServices for MockPlatform {
        fn credential_store(&self) -> &dyn nucleus_platform::credentials::CredentialStore {
            &self.cred
        }
        fn window_manager(&self) -> &dyn nucleus_platform::WindowManager {
            panic!("MockPlatform: window_manager not implemented")
        }
        fn process_manager(&self) -> &dyn nucleus_platform::ProcessManager {
            panic!("MockPlatform: process_manager not implemented")
        }
        fn display_manager(&self) -> &dyn nucleus_platform::DisplayManager {
            panic!("MockPlatform: display_manager not implemented")
        }
        fn input_hook_manager(&self) -> &dyn nucleus_platform::InputHookManager {
            panic!("MockPlatform: input_hook_manager not implemented")
        }
        fn filesystem_manager(&self) -> &dyn nucleus_platform::FilesystemManager {
            panic!("MockPlatform: filesystem_manager not implemented")
        }
        fn registry_manager(&self) -> Option<&dyn nucleus_platform::RegistryManager> {
            None
        }
        fn taskbar_manager(&self) -> &dyn nucleus_platform::TaskbarManager {
            panic!("MockPlatform: taskbar_manager not implemented")
        }
        fn mutex_manager(&self) -> &dyn nucleus_platform::MutexManager {
            panic!("MockPlatform: mutex_manager not implemented")
        }
        fn user_account_manager(&self) -> Option<&dyn nucleus_platform::UserAccountManager> {
            None
        }
        fn network_manager(&self) -> &dyn nucleus_platform::NetworkManager {
            panic!("MockPlatform: network_manager not implemented")
        }
        fn elevation_manager(&self) -> &dyn nucleus_platform::ElevationManager {
            panic!("MockPlatform: elevation_manager not implemented")
        }
        fn osd_manager(&self) -> &dyn nucleus_platform::OsdManager {
            &self.osd
        }
    }

    #[test]
    fn test_game_manager_handler_registration_and_add_game() {
        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform).unwrap();

        let mut config = HandlerConfig::default();
        config.guid = "portal2".to_string();
        config.game_name = "Portal 2".to_string();
        config.executable_name = "portal2.exe".to_string();

        gm.register_handler(config, None);

        let detected = gm
            .add_game(Path::new("C:\\Games\\Portal 2\\portal2.exe"))
            .unwrap();
        assert_eq!(detected, Some("portal2".to_string()));
        assert!(gm.profiles().contains_key("portal2"));

        let unmatched = gm
            .add_game(Path::new("C:\\Games\\Unknown\\unknown.exe"))
            .unwrap();
        assert_eq!(unmatched, None);
    }

    #[test]
    fn test_game_manager_reload_handlers() {
        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform).unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_gm_handlers_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let handler1 = temp_dir.join("test1.js");
        std::fs::write(
            &handler1,
            r#"
            Game.GUID = "test1";
            Game.GameName = "Test Game 1";
            Game.ExecutableName = "test1.exe";
        "#,
        )
        .unwrap();

        gm.set_handlers_dir(&temp_dir);
        assert_eq!(gm.handlers_dir(), temp_dir.as_path());
        gm.initialize().unwrap();
        assert_eq!(gm.get_handlers().len(), 1);

        // Add a second handler in a subdirectory
        let sub_dir = temp_dir.join("subdir");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let handler2 = sub_dir.join("test2.js");
        std::fs::write(
            &handler2,
            r#"
            Game.GUID = "test2";
            Game.GameName = "Test Game 2";
            Game.ExecutableName = "test2.exe";
        "#,
        )
        .unwrap();

        gm.reload_handlers().unwrap();
        assert_eq!(gm.get_handlers().len(), 2);
        assert!(gm.get_handler("test1").is_some());
        assert!(gm.get_handler("test2").is_some());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_canonical_staging_path_resolution() {
        let path = get_app_content_path("Left 4 Dead 2", None);
        let temp = std::env::temp_dir();
        // The default staging folder sits next to the executable, so the check only means
        // something when the test binary itself was not built under the temp folder.
        let built_in_temp = std::env::current_exe()
            .map(|exe| exe.starts_with(&temp))
            .unwrap_or(false);
        if !built_in_temp {
            assert!(
                !path.starts_with(&temp),
                "Staging path should not be in temp_dir: {:?}",
                path
            );
        }
        assert!(path.ends_with("Left 4 Dead 2"));

        let custom = std::path::PathBuf::from("C:\\NucleusCustom");
        let path_custom = get_app_content_path("Left 4 Dead 2", Some(&custom));
        assert_eq!(path_custom, custom.join("Left 4 Dead 2"));
    }

    #[test]
    fn test_game_manager_userprofile_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_gm_userprof_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let fake_exe = temp_dir.join("test_game.exe");
        std::fs::write(&fake_exe, b"FAKE_EXE").unwrap();

        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform).unwrap();
        gm.set_nucleus_data_dir(&temp_dir);

        let mut config = HandlerConfig::default();
        config.guid = "TestGuid".to_string();
        config.game_name = "Test Game".to_string();
        config.executable_name = "test_game.exe".to_string();
        gm.register_handler(config, None);

        let detected = gm.add_game(&fake_exe).unwrap();
        assert_eq!(detected, Some("TestGuid".to_string()));

        let userprof_path = gm.get_userprofile_path();
        assert!(userprof_path.exists(), "userprofile.json should exist");

        // Fresh GameManager should restore game on initialize()
        let platform2 = Arc::new(MockPlatform::default());
        let mut gm2 = GameManager::new(platform2).unwrap();
        gm2.set_nucleus_data_dir(&temp_dir);
        gm2.initialize().unwrap();

        assert!(
            gm2.profiles().contains_key("TestGuid"),
            "TestGuid should be loaded from userprofile.json"
        );
        assert_eq!(
            gm2.get_profile("TestGuid").unwrap().executable_path,
            fake_exe.to_string_lossy()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_manager_numbered_profiles_and_osd_parity() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_gm_profiles_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform.clone()).unwrap();
        gm.set_nucleus_data_dir(&temp_dir);

        let guid = "l4d2";

        // 1. Initial list should be empty
        let initial_list = gm.list_saved_profiles(guid);
        assert!(initial_list.is_empty());

        // 2. Save profile 1 without explicit number
        let mut prof1 = GameProfile::default();
        prof1.player_count = 4;
        prof1.layout = "2x2 (Grid)".to_string();
        let num1 = gm.save_numbered_profile(guid, None, &prof1).unwrap();
        assert_eq!(num1, 1);
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Handler Profile Saved. Setup Finished")
        );

        // 3. Save profile 2 without explicit number
        let mut prof2 = GameProfile::default();
        prof2.player_count = 2;
        prof2.layout = "1x2 (Stacked)".to_string();
        let num2 = gm.save_numbered_profile(guid, None, &prof2).unwrap();
        assert_eq!(num2, 2);

        // 4. Overwrite profile 1 with custom title
        let mut prof1_custom = GameProfile::default();
        prof1_custom.profile_name = Some("Campaign Co-op 4P".to_string());
        prof1_custom.player_count = 4;
        let num1_updated = gm
            .save_numbered_profile(guid, Some(1), &prof1_custom)
            .unwrap();
        assert_eq!(num1_updated, 1);

        // 5. List profiles
        let profiles = gm.list_saved_profiles(guid);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].0, 1);
        assert_eq!(profiles[0].1, "Campaign Co-op 4P");
        assert_eq!(profiles[1].0, 2);
        assert_eq!(profiles[1].1, "Profile 2");

        // 6. Load profile 1: verifies custom title in OSD
        let loaded1 = gm.load_numbered_profile(guid, 1).unwrap();
        assert_eq!(loaded1.profile_name.as_deref(), Some("Campaign Co-op 4P"));
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Handler Profile \"Campaign Co-op 4P\" Loaded")
        );

        // 7. Load profile 2: verifies fallback N°2 in OSD
        let loaded2 = gm.load_numbered_profile(guid, 2).unwrap();
        assert_eq!(loaded2.player_count, 2);
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Handler Profile \"Profile 2\" Loaded")
        );

        // 8. Unload profile
        gm.unload_profile();
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Handler Profile Unloaded")
        );

        // 9. Delete profile 1
        gm.delete_numbered_profile(guid, 1).unwrap();
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Handler Profile Deleted")
        );
        let remaining = gm.list_saved_profiles(guid);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, 2);

        // 10. Remove game
        gm.remove_game(guid).unwrap();
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Game Removed From List")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_manager_uninstalled_handler_favorite_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("nucleus_gm_fav_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform.clone()).unwrap();
        gm.set_nucleus_data_dir(&temp_dir);

        let guid = "halo-custom-edition";
        assert!(gm.get_profile(guid).is_none());

        // Favorite an uninstalled handler whose .exe has not yet been located
        let mut p = GameProfile::new(guid, guid, "");
        p.favorite = true;
        p.player_count = 4;
        gm.save_game_profile(guid, &p).unwrap();

        assert!(gm.get_profile(guid).unwrap().favorite);

        // Verify that on reload / restart, the uninstalled game's favorite state is preserved
        let platform2 = Arc::new(MockPlatform::default());
        let mut gm2 = GameManager::new(platform2.clone()).unwrap();
        gm2.set_nucleus_data_dir(&temp_dir);
        gm2.initialize().unwrap();

        let loaded = gm2.get_profile(guid);
        assert!(
            loaded.is_some(),
            "Uninstalled handler profile should persist in userprofile.json"
        );
        assert!(
            loaded.unwrap().favorite,
            "Favorite status must persist across restarts for uninstalled handlers"
        );

        // Test delete_user_save_path
        gm2.delete_user_save_path(guid).unwrap();
        assert_eq!(
            platform2.osd.current_message().as_deref(),
            Some("UserProfile Save Path Deleted")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_handlers_from_dir_populates_handler_path() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_gm_hpath_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let script_file = temp_dir.join("TestGame.js");
        let script_content = r#"
            Game.GUID = "test_guid_hpath";
            Game.GameName = "Test Game HPath";
            Game.ExecutableName = "test.exe";
            Game.MaxPlayers = 4;
        "#;
        std::fs::write(&script_file, script_content).unwrap();

        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform).unwrap();
        gm.load_handlers_from_dir(&temp_dir);

        let handler = gm.get_handler("test_guid_hpath");
        assert!(handler.is_some(), "Handler should be loaded from directory");
        assert_eq!(
            handler.unwrap().handler_path.as_deref(),
            Some(script_file.to_string_lossy().as_ref()),
            "handler_path must be populated with the exact .js script path on disk"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_manager_freeze_and_controller_reassignment() {
        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform.clone()).unwrap();

        // When no active session is running:
        assert_eq!(gm.freeze_session().unwrap(), None);
        assert_eq!(gm.unfreeze_session().unwrap(), None);
        assert_eq!(gm.toggle_freeze_session().unwrap(), None);
        assert!(gm.reassign_session_controller(0, 2).is_ok());
        assert!(gm.re_enforce_session_windows().is_ok());

        // Attach a mock active session
        let session = GameSession::mock_two_player_session();
        gm.active_session = Some(session);

        // Freeze session
        let res = gm.freeze_session().unwrap();
        assert_eq!(res, Some(true));
        assert!(gm.active_session().unwrap().is_frozen());
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Instances Frozen (NtSuspendProcess)")
        );

        // Toggle freeze -> unfreezes
        let res2 = gm.toggle_freeze_session().unwrap();
        assert_eq!(res2, Some(false));
        assert!(!gm.active_session().unwrap().is_frozen());
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Instances Resumed (NtResumeProcess)")
        );

        // Toggle freeze -> freezes
        let res3 = gm.toggle_freeze_session().unwrap();
        assert_eq!(res3, Some(true));
        assert!(gm.active_session().unwrap().is_frozen());

        // Unfreeze explicitly
        let res4 = gm.unfreeze_session().unwrap();
        assert_eq!(res4, Some(false));
        assert!(!gm.active_session().unwrap().is_frozen());

        // Reassign controller
        gm.reassign_session_controller(0, 5).unwrap();
        assert_eq!(gm.active_session().unwrap().players[0].device_id, 5);
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Player 1 Controller Reassigned to Device #5")
        );

        // Re-enforce windows
        gm.re_enforce_session_windows().unwrap();
        assert_eq!(
            platform.osd.current_message().as_deref(),
            Some("Window Bounds Re-enforced")
        );
    }

    #[test]
    fn test_load_handlers_from_dir_reads_hubmeta_sidecar() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_gm_hubmeta_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Case 1: <stem>.hubmeta.json next to <stem>.js
        let js_file = temp_dir.join("TestSidecar.js");
        std::fs::write(
            &js_file,
            r#"
            Game.GUID = "sidecar_guid";
            Game.GameName = "Test Sidecar";
            Game.ExecutableName = "sidecar.exe";
            "#,
        )
        .unwrap();
        let meta_file = temp_dir.join("TestSidecar.hubmeta.json");
        std::fs::write(
            &meta_file,
            r#"{"handler_id":"hid_abc","package_id":"pkg_v2","installed_version":2}"#,
        )
        .unwrap();

        // Case 2: <dir>/<stem>/.hubmeta.json fallback
        let js_file2 = temp_dir.join("SubFolderGame.js");
        std::fs::write(
            &js_file2,
            r#"
            Game.GUID = "sub_sidecar_guid";
            Game.GameName = "SubFolder Game";
            Game.ExecutableName = "subgame.exe";
            "#,
        )
        .unwrap();
        let sub_dir = temp_dir.join("SubFolderGame");
        std::fs::create_dir_all(&sub_dir).unwrap();
        std::fs::write(
            sub_dir.join(".hubmeta.json"),
            r#"{"handler_id":"hid_sub","package_id":"pkg_v4","installed_version":4}"#,
        )
        .unwrap();

        let platform = Arc::new(MockPlatform::default());
        let mut gm = GameManager::new(platform).unwrap();
        gm.set_handlers_dir(&temp_dir);
        gm.set_nucleus_data_dir(&temp_dir);
        gm.load_handlers_from_dir(&temp_dir);

        let h1 = gm
            .get_handler("sidecar_guid")
            .expect("sidecar handler should load");
        assert_eq!(h1.hub_handler_version, Some(2));
        assert_eq!(h1.hub_handler_id.as_deref(), Some("hid_abc"));

        let h2 = gm
            .get_handler("sub_sidecar_guid")
            .expect("subfolder sidecar handler should load");
        assert_eq!(h2.hub_handler_version, Some(4));
        assert_eq!(h2.hub_handler_id.as_deref(), Some("hid_sub"));

        gm.delete_game_handler("sidecar_guid").unwrap();
        assert!(!js_file.exists());
        assert!(
            !meta_file.exists(),
            "TestSidecar.hubmeta.json should be deleted on delete_game_handler"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
