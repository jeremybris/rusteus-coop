//! PeerParty runtime driver and environment generator for multi-instance game isolation.

use crate::peerparty_types::{PeerPartyConfig, PeerPartyService};
use nucleus_common::dll_injection::DllInjectionConfig;
use nucleus_common::error::{NucleusError, NucleusResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// High-level management tool for staging PeerParty hook DLLs, configuring environment variables,
/// and coordinating multiplayer middleware emulation across player instances.
#[derive(Debug, Clone)]
pub struct PeerPartyTool {
    utils_dir: PathBuf,
}

impl PeerPartyTool {
    /// Creates a new `PeerPartyTool` rooted at the given utility directory (typically `./utils`).
    pub fn new(utils_dir: PathBuf) -> Self {
        Self { utils_dir }
    }

    /// Resolves the absolute or relative path to the PeerParty hook DLL matching the target process bitness.
    pub fn resolve_hook_dll(&self, is_64bit: bool) -> Option<PathBuf> {
        let pp_dir = self.utils_dir.join("PeerParty");

        if is_64bit {
            let p1 = pp_dir.join("x64").join("peerparty_hook.dll");
            let p2 = pp_dir.join("peerparty_hook.dll");
            if p1.exists() {
                Some(p1)
            } else if p2.exists() {
                Some(p2)
            } else {
                None
            }
        } else {
            let p1 = pp_dir.join("x86").join("peerparty_hook32.dll");
            let p2 = pp_dir.join("peerparty_hook32.dll");
            let p3 = pp_dir.join("peerparty_hook.dll");
            if p1.exists() {
                Some(p1)
            } else if p2.exists() {
                Some(p2)
            } else if p3.exists() {
                Some(p3)
            } else {
                None
            }
        }
    }

    /// Resolves the path to the PeerParty launcher executable `peerparty.exe` if present.
    pub fn resolve_launcher_exe(&self) -> Option<PathBuf> {
        let p1 = self.utils_dir.join("PeerParty").join("peerparty.exe");
        let p2 = self.utils_dir.join("peerparty.exe");
        if p1.exists() {
            Some(p1)
        } else if p2.exists() {
            Some(p2)
        } else {
            None
        }
    }

    /// Generates CLI arguments for invoking `peerparty.exe` directly as a game launcher.
    pub fn generate_launcher_args(
        &self,
        config: &PeerPartyConfig,
        player_index: usize,
        player_nick: &str,
    ) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(ref room) = config.room_code {
            args.push("--room".to_string());
            args.push(room.clone());
        }
        if config.public_sessions {
            args.push("--public".to_string());
        }
        if config.multi_instance {
            args.push("--multi-instance".to_string());
        }
        if config.copy_saves && player_index > 0 {
            args.push("--copy-saves".to_string());
        }
        let voice_str = if config.voice_enabled { "on" } else { "off" };
        let overlay_str = if config.overlay_enabled { "on" } else { "off" };
        args.push("--set".to_string());
        args.push(format!(
            "name={},voice={},overlay={}",
            player_nick, voice_str, overlay_str
        ));
        args
    }

    /// Generates the `DllInjectionConfig` for injecting `peerparty_hook.dll` into target instances.
    pub fn get_injection_config(
        &self,
        is_64bit: bool,
        config: &PeerPartyConfig,
    ) -> NucleusResult<DllInjectionConfig> {
        let dll_path = self.resolve_hook_dll(is_64bit).ok_or_else(|| {
            NucleusError::Platform(format!(
                "PeerParty hook DLL not found under {:?} (is_64bit: {})",
                self.utils_dir.join("PeerParty"),
                is_64bit
            ))
        })?;

        Ok(config.to_dll_injection_config(dll_path))
    }

    /// Generates per-instance environment variables for PeerParty runtime isolation.
    pub fn generate_env_vars(
        &self,
        player_index: usize,
        player_nick: &str,
        instance_dir: &Path,
        config: &PeerPartyConfig,
    ) -> HashMap<String, String> {
        let mut envs = HashMap::new();

        if !config.enabled {
            return envs;
        }

        envs.insert(
            "PEERPARTY_GAME_ROOT".to_string(),
            instance_dir.to_string_lossy().to_string(),
        );
        envs.insert("PEERPARTY_PLAYER_NAME".to_string(), player_nick.to_string());
        envs.insert("PEERPARTY_SERVICES".to_string(), config.to_services_env());

        if config.multi_instance {
            envs.insert("PEERPARTY_MULTI_INSTANCE".to_string(), "1".to_string());
        }

        if config.copy_saves && player_index > 0 {
            envs.insert("PEERPARTY_COPY_SAVES".to_string(), "1".to_string());
        }

        if let Some(ref room) = config.room_code {
            envs.insert("PEERPARTY_ROOM".to_string(), room.clone());
        }

        if config.public_sessions {
            envs.insert("PEERPARTY_PUBLIC".to_string(), "1".to_string());
        }

        // Formats setting overrides: name=<Nick>,voice=<on|off>,overlay=<on|off>
        let voice_str = if config.voice_enabled { "on" } else { "off" };
        let overlay_str = if config.overlay_enabled { "on" } else { "off" };
        let set_str = format!(
            "name={},voice={},overlay={}",
            player_nick, voice_str, overlay_str
        );
        envs.insert("PEERPARTY_SET".to_string(), set_str);

        envs
    }

    /// Checks whether legacy Goldberg Steam emulator deployment should be suppressed.
    pub fn should_suppress_goldberg(&self, config: &PeerPartyConfig) -> bool {
        config.handles_service(PeerPartyService::Steamworks)
    }

    /// Checks whether legacy Nemirtingas Epic Games emulator deployment should be suppressed.
    pub fn should_suppress_nemirtingas_epic(&self, config: &PeerPartyConfig) -> bool {
        config.handles_service(PeerPartyService::EpicOnlineServices)
    }

    /// Checks whether legacy Nemirtingas GOG Galaxy emulator deployment should be suppressed.
    pub fn should_suppress_nemirtingas_galaxy(&self, config: &PeerPartyConfig) -> bool {
        config.handles_service(PeerPartyService::GogGalaxy)
    }
}
