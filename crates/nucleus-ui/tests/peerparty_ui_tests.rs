//! Unit tests for PeerParty UI controls and settings cards.

use nucleus_ui::settings_view::{SETTINGS_CARDS, SettingsCategory, filter_settings_cards};

#[test]
fn test_peerparty_settings_card_registration() {
    let card = SETTINGS_CARDS
        .iter()
        .find(|c| c.id == "card-peerparty-config");
    assert!(
        card.is_some(),
        "card-peerparty-config must be registered in SETTINGS_CARDS"
    );

    let card = card.unwrap();
    assert_eq!(card.category, SettingsCategory::Network);
    assert!(card.title.contains("PeerParty"));
}

#[test]
fn test_peerparty_settings_card_search() {
    let matches = filter_settings_cards("peerparty");
    assert!(!matches.is_empty());
    assert!(matches.iter().any(|c| c.id == "card-peerparty-config"));

    let matches_dht = filter_settings_cards("DHT");
    assert!(matches_dht.iter().any(|c| c.id == "card-peerparty-config"));
}
