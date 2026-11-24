//! Unit tests for PeerParty types, service enums, and configuration serialization.

use nucleus_common::dll_injection::{InjectionMethod, InjectionTiming, TargetArchitecture};
use nucleus_tools::peerparty_types::{PeerPartyConfig, PeerPartyLaunchMode, PeerPartyService};
use std::path::PathBuf;
use std::str::FromStr;

#[test]
fn test_peerparty_service_roundtrip_and_aliases() {
    assert_eq!(
        PeerPartyService::from_str("steam").unwrap(),
        PeerPartyService::Steamworks
    );
    assert_eq!(
        PeerPartyService::from_str("Steamworks").unwrap(),
        PeerPartyService::Steamworks
    );
    assert_eq!(
        PeerPartyService::from_str("EOS").unwrap(),
        PeerPartyService::EpicOnlineServices
    );
    assert_eq!(
        PeerPartyService::from_str("epic").unwrap(),
        PeerPartyService::EpicOnlineServices
    );
    assert_eq!(
        PeerPartyService::from_str("playfab").unwrap(),
        PeerPartyService::PlayFab
    );
    assert_eq!(
        PeerPartyService::from_str("galaxy").unwrap(),
        PeerPartyService::GogGalaxy
    );
    assert_eq!(
        PeerPartyService::from_str("photon").unwrap(),
        PeerPartyService::Photon
    );
    assert_eq!(
        PeerPartyService::from_str("pun").unwrap(),
        PeerPartyService::Photon
    );
    assert_eq!(
        PeerPartyService::from_str("ugs").unwrap(),
        PeerPartyService::UnityGamingServices
    );
    assert_eq!(
        PeerPartyService::from_str("demonware").unwrap(),
        PeerPartyService::Demonware
    );
    assert_eq!(
        PeerPartyService::from_str("xbox").unwrap(),
        PeerPartyService::XboxLive
    );

    assert_eq!(PeerPartyService::Steamworks.as_str(), "steam");
    assert_eq!(PeerPartyService::EpicOnlineServices.as_str(), "eos");
    assert_eq!(PeerPartyService::Photon.as_str(), "photon");
    assert_eq!(PeerPartyService::UnityGamingServices.as_str(), "ugs");
}

#[test]
fn test_peerparty_config_serialization_and_builder() {
    let config = PeerPartyConfig::new()
        .with_service(PeerPartyService::Steamworks)
        .with_service(PeerPartyService::Photon)
        .with_room_code("TEST_ROOM_123")
        .with_public_sessions(true)
        .with_copy_saves(true);

    assert!(config.enabled);
    assert!(config.handles_service(PeerPartyService::Steamworks));
    assert!(config.handles_service(PeerPartyService::Photon));
    assert!(!config.handles_service(PeerPartyService::Demonware));
    assert_eq!(config.room_code.as_deref(), Some("TEST_ROOM_123"));
    assert!(config.public_sessions);
    assert!(config.copy_saves);

    let json = serde_json::to_string_pretty(&config).expect("failed to serialize PeerPartyConfig");
    let deserialized: PeerPartyConfig =
        serde_json::from_str(&json).expect("failed to deserialize PeerPartyConfig");

    assert_eq!(config, deserialized);
}

#[test]
fn test_peerparty_dll_injection_config_conversion() {
    let config = PeerPartyConfig::new();
    let dll_path = PathBuf::from("utils/PeerParty/x64/peerparty_hook.dll");
    let inj_cfg = config.to_dll_injection_config(dll_path.clone());

    assert_eq!(inj_cfg.path, dll_path.to_string_lossy());
    assert_eq!(inj_cfg.timing, InjectionTiming::AtStartupSuspended);
    assert_eq!(inj_cfg.method, InjectionMethod::QueueUserApc);
    assert_eq!(inj_cfg.architecture, TargetArchitecture::Auto);
    assert!(inj_cfg.required);
}
