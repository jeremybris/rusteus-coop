//! Unit tests for PeerParty runtime driver and environment generator.

use nucleus_tools::peerparty::PeerPartyTool;
use nucleus_tools::peerparty_types::{PeerPartyConfig, PeerPartyService};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_peerparty_driver_env_generation() {
    let tool = PeerPartyTool::new(PathBuf::from("utils"));
    let config = PeerPartyConfig::new()
        .with_service(PeerPartyService::Steamworks)
        .with_service(PeerPartyService::Photon)
        .with_room_code("TEST_ROOM_42")
        .with_public_sessions(true)
        .with_copy_saves(true);

    let instance_dir = Path::new("C:/Games/Nucleus/Instance1");
    let envs = tool.generate_env_vars(1, "PlayerTwo", instance_dir, &config);

    assert_eq!(
        envs.get("PEERPARTY_GAME_ROOT"),
        Some(&"C:/Games/Nucleus/Instance1".to_string())
    );
    assert_eq!(
        envs.get("PEERPARTY_PLAYER_NAME"),
        Some(&"PlayerTwo".to_string())
    );
    assert_eq!(envs.get("PEERPARTY_MULTI_INSTANCE"), Some(&"1".to_string()));
    assert_eq!(envs.get("PEERPARTY_COPY_SAVES"), Some(&"1".to_string()));
    assert_eq!(
        envs.get("PEERPARTY_ROOM"),
        Some(&"TEST_ROOM_42".to_string())
    );
    assert_eq!(envs.get("PEERPARTY_PUBLIC"), Some(&"1".to_string()));
    assert!(envs.get("PEERPARTY_SERVICES").unwrap().contains("steam"));
    assert!(envs.get("PEERPARTY_SERVICES").unwrap().contains("photon"));
    assert!(
        envs.get("PEERPARTY_SET")
            .unwrap()
            .contains("name=PlayerTwo")
    );
}

#[test]
fn test_peerparty_emulator_suppression() {
    let tool = PeerPartyTool::new(PathBuf::from("utils"));
    let config = PeerPartyConfig::new()
        .with_service(PeerPartyService::Steamworks)
        .with_service(PeerPartyService::EpicOnlineServices);

    assert!(tool.should_suppress_goldberg(&config));
    assert!(tool.should_suppress_nemirtingas_epic(&config));
    assert!(!tool.should_suppress_nemirtingas_galaxy(&config));
}

#[test]
fn test_peerparty_dll_resolution_and_injection_config() {
    let temp_dir = std::env::temp_dir().join("nucleus_test_pp_utils");
    let x64_dir = temp_dir.join("PeerParty").join("x64");
    fs::create_dir_all(&x64_dir).unwrap();
    let fake_dll = x64_dir.join("peerparty_hook.dll");
    fs::write(&fake_dll, b"MZ").unwrap();

    let tool = PeerPartyTool::new(temp_dir.clone());
    let resolved = tool.resolve_hook_dll(true);
    assert_eq!(resolved, Some(fake_dll.clone()));

    let config = PeerPartyConfig::new();
    let inj_cfg = tool.get_injection_config(true, &config).unwrap();
    assert_eq!(inj_cfg.path, fake_dll.to_string_lossy());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_peerparty_launcher_args_generation() {
    let tool = PeerPartyTool::new(PathBuf::from("utils"));
    let config = PeerPartyConfig::new()
        .with_room_code("COOP_ROOM_99")
        .with_public_sessions(true)
        .with_copy_saves(true);

    let args = tool.generate_launcher_args(&config, 1, "PlayerOne");
    assert!(args.contains(&"--room".to_string()));
    assert!(args.contains(&"COOP_ROOM_99".to_string()));
    assert!(args.contains(&"--public".to_string()));
    assert!(args.contains(&"--multi-instance".to_string()));
    assert!(args.contains(&"--copy-saves".to_string()));
    assert!(args.contains(&"--set".to_string()));
    assert!(args.iter().any(|a| a.contains("name=PlayerOne")));
}
