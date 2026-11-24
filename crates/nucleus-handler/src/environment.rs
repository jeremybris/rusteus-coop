//! Instance process environment configuration and variables.

use nucleus_common::error::NucleusResult;
use nucleus_tools::users::{PersonalFolderGuard, PlayerEnvironment, UsersTool};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context managing the lifecycle of isolated player environments and Windows shell redirection for a game session.
#[derive(Debug, Default)]
pub struct SessionEnvironmentContext {
    /// Isolated player environments for each participant.
    pub player_environments: Vec<PlayerEnvironment>,
    /// Active RAII guard for the Windows Explorer `Personal` shell folder redirection.
    pub personal_folder_guard: Option<PersonalFolderGuard>,
    /// Directory used for registry backups.
    pub backup_dir: PathBuf,
}

impl SessionEnvironmentContext {
    /// Creates a new `SessionEnvironmentContext`.
    pub fn new(backup_dir: PathBuf) -> Self {
        Self {
            player_environments: Vec::new(),
            personal_folder_guard: None,
            backup_dir,
        }
    }

    /// Returns a reference to the isolated environment for a specific player index.
    pub fn player_environment(&self, player_index: usize) -> Option<&PlayerEnvironment> {
        self.player_environments.get(player_index)
    }

    /// Builds the environment variable key-value map for a specific player instance.
    pub fn build_environment_for_player(
        &self,
        player_index: usize,
        app_id: Option<&str>,
    ) -> HashMap<String, String> {
        EnvironmentSetup::build_environment(
            player_index,
            app_id,
            self.player_environments.get(player_index),
        )
    }

    /// Restores any shell folder redirections and cleans up session-level environment overrides.
    pub fn restore(&mut self) -> NucleusResult<()> {
        if let Some(mut guard) = self.personal_folder_guard.take() {
            guard.restore()?;
        }
        let _ = UsersTool::restore_user_shell_folders_from_backup(&self.backup_dir);
        Ok(())
    }
}

/// Manages customized environment variables and process context for instance isolation.
pub struct EnvironmentSetup;

impl EnvironmentSetup {
    /// Builds the environment variable key-value map for a specific player instance.
    pub fn build_environment(
        player_index: usize,
        app_id: Option<&str>,
        player_env: Option<&PlayerEnvironment>,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();
        let player_id = (player_index + 1).to_string();
        env.insert("NUCLEUS_PLAYER_ID".to_string(), player_id);
        env.insert("NUCLEUS_PLAYER_INDEX".to_string(), player_index.to_string());
        env.insert(
            "NUCLEUS_INSTANCE_INDEX".to_string(),
            player_index.to_string(),
        );
        env.insert(
            "NUCLEUS_PIPE_NAME".to_string(),
            format!(r"\\.\pipe\nucleus_instance_{}", player_index),
        );
        env.insert(
            "NUCLEUS_SHARED_MEM_NAME".to_string(),
            format!("Local\\nucleus_instance_{}", player_index),
        );

        if let Some(id) = app_id {
            env.insert("SteamAppId".to_string(), id.to_string());
            env.insert("SteamGameId".to_string(), id.to_string());
            env.insert("SteamOverlayGameId".to_string(), id.to_string());
        }

        if let Some(p_env) = player_env {
            p_env.apply_to_env(&mut env);
        }

        env
    }

    /// Initializes isolated player environments, configures file replication, purges save/config files,
    /// and redirects the Windows Explorer `Personal` shell folder if required.
    pub fn initialize_player_session(
        num_players: usize,
        use_nucleus_environment: bool,
        user_profile_config_path: Option<&str>,
        user_profile_save_path: Option<&str>,
        user_profile_save_path_no_copy: bool,
        documents_config_path: Option<&str>,
        documents_save_path: Option<&str>,
        delete_config_patterns: Option<&[String]>,
        delete_save_patterns: Option<&[String]>,
        transfer_profiles: bool,
        backup_dir: Option<&Path>,
    ) -> NucleusResult<SessionEnvironmentContext> {
        let b_dir = backup_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("utils").join("backup"));

        let mut context = SessionEnvironmentContext::new(b_dir.clone());

        for i in 0..num_players {
            let player_nick = format!("Player{}", i + 1);
            let mut env = UsersTool::create_player_environment(&player_nick)?;

            if let Some(path) = user_profile_config_path {
                let _ = UsersTool::copy_user_profile_config_path(&env, path, false);
                let config_dir = env.root.join(path);
                env = env.with_config_path(config_dir);
            }

            if let Some(path) = user_profile_save_path {
                if !user_profile_save_path_no_copy {
                    let _ = UsersTool::copy_user_profile_config_path(&env, path, false);
                }
                let save_dir = env.root.join(path);
                env = env.with_save_path(save_dir);
            }

            if let Some(path) = documents_config_path {
                let _ = UsersTool::copy_documents_path(&env, path, false);
            }

            if let Some(path) = documents_save_path {
                let _ = UsersTool::copy_documents_path(&env, path, false);
            }

            // Purge config/save files matching configured deletion patterns prior to launch
            if let Some(patterns) = delete_config_patterns {
                let _ = UsersTool::delete_files_in_config_path(&env, patterns);
            }
            if let Some(patterns) = delete_save_patterns {
                let _ = UsersTool::delete_files_in_save_path(&env, patterns);
            }

            // Transfer profile data from Player1 if enabled
            if transfer_profiles && i > 0 {
                let _ = UsersTool::transfer_nucleus_user_account_profiles("Player1", &player_nick);
            }

            context.player_environments.push(env);
        }

        // Redirect Windows Explorer Personal Shell Folder if requested
        let needs_shell_redirection = use_nucleus_environment
            || documents_config_path.is_some()
            || documents_save_path.is_some();

        if needs_shell_redirection && !context.player_environments.is_empty() {
            let p1_docs = &context.player_environments[0].documents;
            match UsersTool::redirect_personal_folder(p1_docs, Some(&b_dir)) {
                Ok(guard) => {
                    context.personal_folder_guard = Some(guard);
                }
                Err(err) => {
                    tracing::warn!("Failed to redirect Personal shell folder: {}", err);
                }
            }
        }

        Ok(context)
    }

    /// Helper to purge matching files in player config and save directories.
    pub fn purge_player_files(
        player_env: &PlayerEnvironment,
        config_patterns: Option<&[String]>,
        save_patterns: Option<&[String]>,
    ) -> NucleusResult<usize> {
        let mut total = 0;
        if let Some(pats) = config_patterns {
            total += UsersTool::delete_files_in_config_path(player_env, pats)?;
        }
        if let Some(pats) = save_patterns {
            total += UsersTool::delete_files_in_save_path(player_env, pats)?;
        }
        Ok(total)
    }

    /// Helper to transfer user profile save data between players.
    pub fn transfer_player_profile(
        source_nickname: &str,
        target_nickname: &str,
    ) -> NucleusResult<usize> {
        UsersTool::transfer_nucleus_user_account_profiles(source_nickname, target_nickname)
    }

    /// Restores any session environment overrides, including Windows Explorer shell folder redirection.
    pub fn teardown_session_environment(
        context: &mut SessionEnvironmentContext,
        backup_dir: Option<&Path>,
    ) -> NucleusResult<()> {
        context.restore()?;
        if let Some(dir) = backup_dir {
            let _ = UsersTool::restore_user_shell_folders_from_backup(dir);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleus_common::file_util;

    #[test]
    fn test_build_environment_with_player_isolation() {
        let p_env = PlayerEnvironment::new(
            PathBuf::from("C:\\Users\\Test\\NucleusCoop\\Player1"),
            PathBuf::from("C:\\Users\\Test\\NucleusCoop\\Player1\\Documents"),
            PathBuf::from("C:\\Users\\Test\\NucleusCoop\\Player1\\AppData\\Roaming"),
            PathBuf::from("C:\\Users\\Test\\NucleusCoop\\Player1\\AppData\\Local"),
            "\\Users\\Test\\NucleusCoop\\Player1".to_string(),
        );

        let env = EnvironmentSetup::build_environment(0, Some("550"), Some(&p_env));
        assert_eq!(env.get("NUCLEUS_PLAYER_ID").unwrap(), "1");
        assert_eq!(env.get("SteamAppId").unwrap(), "550");
        assert_eq!(
            env.get("USERPROFILE").unwrap(),
            "C:\\Users\\Test\\NucleusCoop\\Player1"
        );
        assert_eq!(
            env.get("APPDATA").unwrap(),
            "C:\\Users\\Test\\NucleusCoop\\Player1\\AppData\\Roaming"
        );
        assert_eq!(
            env.get("LOCALAPPDATA").unwrap(),
            "C:\\Users\\Test\\NucleusCoop\\Player1\\AppData\\Local"
        );
        assert_eq!(
            env.get("HOMEPATH").unwrap(),
            "\\Users\\Test\\NucleusCoop\\Player1"
        );
    }

    #[test]
    fn test_purge_player_files_helper() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_env_purge_test_{}", std::process::id()));
        let root = temp_dir.join("NucleusCoop").join("Player1");
        let docs = root.join("Documents");
        let roaming = root.join("AppData").join("Roaming");
        let local = root.join("AppData").join("Local");

        file_util::ensure_dir(&docs).unwrap();
        file_util::ensure_dir(&roaming).unwrap();
        file_util::ensure_dir(&local).unwrap();

        let cfg = roaming.join("config.cfg");
        let save = docs.join("save.dat");
        let keep = docs.join("keep.txt");

        file_util::write_text_file(&cfg, "test_config").unwrap();
        file_util::write_text_file(&save, "test_save").unwrap();
        file_util::write_text_file(&keep, "keep_this").unwrap();

        let p_env = PlayerEnvironment::new(
            root,
            docs,
            roaming,
            local,
            "\\Users\\Test\\NucleusCoop\\Player1".to_string(),
        );

        let config_pats = vec!["*.cfg".to_string()];
        let save_pats = vec!["*.dat".to_string()];

        let count =
            EnvironmentSetup::purge_player_files(&p_env, Some(&config_pats), Some(&save_pats))
                .unwrap();

        assert_eq!(count, 2);
        assert!(!cfg.exists());
        assert!(!save.exists());
        assert!(keep.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_environment_context_lifecycle() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_env_ctx_test_{}", std::process::id()));
        let backup_dir = temp_dir.join("backup");
        file_util::ensure_dir(&backup_dir).unwrap();

        let mut ctx = SessionEnvironmentContext::new(backup_dir.clone());
        let p_env = PlayerEnvironment::new(
            temp_dir.join("Player1"),
            temp_dir.join("Player1").join("Documents"),
            temp_dir.join("Player1").join("AppData").join("Roaming"),
            temp_dir.join("Player1").join("AppData").join("Local"),
            "\\Users\\Test\\NucleusCoop\\Player1".to_string(),
        );
        ctx.player_environments.push(p_env);

        let env_map = ctx.build_environment_for_player(0, Some("480"));
        assert_eq!(env_map.get("NUCLEUS_PLAYER_ID").unwrap(), "1");
        assert_eq!(env_map.get("SteamAppId").unwrap(), "480");

        assert!(ctx.restore().is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
