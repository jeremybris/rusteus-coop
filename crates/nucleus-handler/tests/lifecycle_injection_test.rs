use nucleus_common::dll_injection::{DllInjectionConfig, InjectionTiming, TargetInstanceFilter};
use nucleus_handler::environment::EnvironmentSetup;
use nucleus_handler::game_handler::GameHandler;
use nucleus_handler::traits::IGameHandler;
use nucleus_script::HandlerConfig;
use std::path::PathBuf;

#[test]
fn test_environment_telemetry_variables() {
    let env_0 = EnvironmentSetup::build_environment(0, Some("480"), None);
    assert_eq!(
        env_0.get("NUCLEUS_INSTANCE_INDEX").map(|s| s.as_str()),
        Some("0")
    );
    assert_eq!(
        env_0.get("NUCLEUS_PLAYER_INDEX").map(|s| s.as_str()),
        Some("0")
    );
    assert_eq!(
        env_0.get("NUCLEUS_PLAYER_ID").map(|s| s.as_str()),
        Some("1")
    );
    assert_eq!(
        env_0.get("NUCLEUS_PIPE_NAME").map(|s| s.as_str()),
        Some(r"\\.\pipe\nucleus_instance_0")
    );
    assert_eq!(
        env_0.get("NUCLEUS_SHARED_MEM_NAME").map(|s| s.as_str()),
        Some("Local\\nucleus_instance_0")
    );

    let env_1 = EnvironmentSetup::build_environment(1, None, None);
    assert_eq!(
        env_1.get("NUCLEUS_INSTANCE_INDEX").map(|s| s.as_str()),
        Some("1")
    );
    assert_eq!(
        env_1.get("NUCLEUS_PIPE_NAME").map(|s| s.as_str()),
        Some(r"\\.\pipe\nucleus_instance_1")
    );
}

#[test]
fn test_game_handler_dll_path_resolution_and_filtering() {
    let temp_dir =
        std::env::temp_dir().join(format!("nucleus_dll_lifecycle_test_{}", std::process::id()));
    let game_dir = temp_dir.join("game");
    let staging_dir = temp_dir.join("staging");
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();

    let exe_path = game_dir.join("game.exe");
    std::fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    // Create a dummy DLL inside game_dir
    let game_dll = game_dir.join("game_mod.dll");
    std::fs::write(&game_dll, b"DUMMY_DLL").unwrap();

    let mut config = HandlerConfig::default();
    config.game_name = "TestDllLifecycleGame".to_string();
    config.guid = "test-dll-lifecycle".to_string();
    config.max_players = 2;

    let mut dll_first = DllInjectionConfig::new("game_mod.dll");
    dll_first.target_instances = TargetInstanceFilter::First;
    dll_first.timing = InjectionTiming::AtStartupSuspended;
    dll_first.eject_on_close = true;

    let mut dll_second = DllInjectionConfig::new("mods/player2.dll");
    dll_second.target_instances = TargetInstanceFilter::ExceptFirst;
    dll_second.timing = InjectionTiming::OnWindowCreated;

    config.injected_dlls = vec![dll_first, dll_second];

    let mut handler = GameHandler::new(config, game_dir.clone(), exe_path, staging_dir, 2);
    handler
        .prepare_instances()
        .expect("Prepare instances failed");

    // Test DLL path resolution: instance-staged file is prioritized for isolation
    let resolved_first = handler.resolve_dll_path(&handler.config.injected_dlls[0], 0);
    let expected_inst_dll =
        PathBuf::from(&handler.contexts[0].instance_folder).join("game_mod.dll");
    assert_eq!(PathBuf::from(&resolved_first.path), expected_inst_dll);
    assert!(PathBuf::from(&resolved_first.path).exists());

    // Test fallback to game_dir when file is only in game_dir (not staged in instance)
    let extra_dll = game_dir.join("non_staged.dll");
    std::fs::write(&extra_dll, b"EXTRA_DLL").unwrap();
    let cfg_extra = DllInjectionConfig::new("non_staged.dll");
    let resolved_extra = handler.resolve_dll_path(&cfg_extra, 0);
    assert_eq!(PathBuf::from(&resolved_extra.path), extra_dll);

    // Test filter logic
    assert!(
        handler.config.injected_dlls[0]
            .target_instances
            .applies_to(0, 2)
    );
    assert!(
        !handler.config.injected_dlls[0]
            .target_instances
            .applies_to(1, 2)
    );
    assert!(
        !handler.config.injected_dlls[1]
            .target_instances
            .applies_to(0, 2)
    );
    assert!(
        handler.config.injected_dlls[1]
            .target_instances
            .applies_to(1, 2)
    );

    // Test injected modules tracking and teardown
    handler.injected_modules.push((1234, 0x10000000, true));
    handler.stop().expect("Stop handler failed");
    assert!(
        handler.injected_modules.is_empty(),
        "Injected modules should be cleared after stop"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
