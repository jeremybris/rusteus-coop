use nucleus_engine::session::GameSession;
use nucleus_hook::memory_engine::MemoryEngine;
use nucleus_profile::GameProfile;

#[test]
fn test_session_tick_applies_frozen_memory_values() {
    let profile = GameProfile::new("test-player", "test-game", "game.exe");
    let mut session = GameSession::new("test-session", "test-game", profile);

    // Set up a target memory buffer in the current process
    let mut target_val: i32 = 100;
    let target_addr = &mut target_val as *mut i32 as usize;
    let current_pid = std::process::id();

    // Freeze value to 999
    let freeze_val: i32 = 999;
    let freeze_bytes = freeze_val.to_ne_bytes().to_vec();
    let slot_id = MemoryEngine::freeze_value(current_pid, target_addr, freeze_bytes);

    // Tamper with value
    target_val = 42;
    assert_eq!(target_val, 42);

    // Call session.tick()
    session.tick().expect("session.tick failed");

    // Value must be restored to 999
    assert_eq!(
        target_val, 999,
        "session.tick must enforce frozen memory values"
    );

    // Clean up
    assert!(MemoryEngine::unfreeze_value(slot_id));
}
