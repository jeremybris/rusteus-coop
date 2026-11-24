//! Unit tests for PeerParty script properties and Boa JS bridge.

use nucleus_common::dll_injection::{InjectionMethod, InjectionTiming};
use nucleus_script::engine::ScriptEngine;
use nucleus_script::handler_context::HandlerContext;

#[test]
fn test_evaluate_handler_with_peerparty_properties() {
    let script = r#"
        Game.GameName = "PeerParty Test Game";
        Game.UsePeerParty = true;
        Game.PeerPartyServices = ["Steam", "Photon", "UGS"];
        Game.PeerPartyRoom = "SECRET_ROOM_99";
        Game.PeerPartyPublic = true;
        Game.PeerPartyVoice = true;
        Game.PeerPartyOverlay = false;
        Game.PeerPartyCopySaves = true;
    "#;

    let mut engine = ScriptEngine::new().expect("failed to create ScriptEngine");
    let config = engine
        .load_handler_script(script)
        .expect("failed to evaluate handler script");

    assert_eq!(config.game_name, "PeerParty Test Game");
    assert!(config.use_peerparty);
    assert!(config.peerparty_public);
    assert!(config.peerparty_voice);
    assert!(!config.peerparty_overlay);
    assert!(config.peerparty_copy_saves);
    assert_eq!(config.peerparty_room.as_deref(), Some("SECRET_ROOM_99"));

    let services = config.peerparty_services.unwrap();
    assert_eq!(services, vec!["Steam", "Photon", "UGS"]);

    // Verify hook DLL was automatically appended to injected_dlls
    let hook_dll = config
        .injected_dlls
        .iter()
        .find(|d| d.path.contains("peerparty_hook"))
        .expect("peerparty_hook.dll not found in injected_dlls");

    assert_eq!(hook_dll.timing, InjectionTiming::AtStartupSuspended);
    assert_eq!(hook_dll.method, InjectionMethod::QueueUserApc);
    assert!(hook_dll.required);
}

#[test]
fn test_inject_context_peerparty_reflection() {
    let mut ctx = HandlerContext::default();
    ctx.use_peerparty = true;
    ctx.peerparty_room = Some("ROOM_ALPHA".to_string());

    let mut engine = ScriptEngine::new().unwrap();
    engine
        .inject_context(&ctx)
        .expect("failed to inject context");

    let res_use = engine
        .execute("Context.UsePeerParty;")
        .expect("failed to eval");
    assert_eq!(res_use.as_boolean(), Some(true));

    let res_room = engine
        .execute("Context.PeerPartyRoom;")
        .expect("failed to eval");
    assert_eq!(
        res_room.as_string().map(|s| s.to_std_string_escaped()),
        Some("ROOM_ALPHA".to_string())
    );
}
