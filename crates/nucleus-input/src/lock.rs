//! Input lock and unlock state management.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Process-wide mirror of the most recent lock state, so the running session can apply it
/// to the game instances without holding a reference to the UI's `InputLock`.
static GLOBAL_LOCKED: AtomicBool = AtomicBool::new(false);

/// Pending lock change requested outside the UI (0 = none, 1 = lock, 2 = unlock).
static EXTERNAL_REQUEST: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Locks or unlocks input on behalf of the running session (for example a handler's
/// `LockInputAtStart`). The next `InputLock` query adopts the requested state, so the
/// lock key keeps toggling from what the user actually sees.
pub fn request_input_lock(locked: bool) {
    EXTERNAL_REQUEST.store(if locked { 1 } else { 2 }, Ordering::SeqCst);
    GLOBAL_LOCKED.store(locked, Ordering::SeqCst);
}

/// Whether input is currently locked to the game instances.
pub fn global_input_locked() -> bool {
    GLOBAL_LOCKED.load(Ordering::SeqCst)
}

/// Default virtual key code for toggling input lock (VK_END = 0x23).
pub const DEFAULT_LOCK_TOGGLE_KEY: u32 = 0x23;

/// Manages the locked state for keyboard and mouse focus across game instances.
#[derive(Debug, Clone)]
pub struct InputLock {
    /// Atomic flag indicating whether input lock is currently active.
    is_locked: Arc<AtomicBool>,
    /// Virtual key code assigned to toggle the lock state.
    toggle_key: u32,
}

impl InputLock {
    /// Creates a new `InputLock` with default unlocked state and standard toggle key.
    pub fn new() -> Self {
        Self {
            is_locked: Arc::new(AtomicBool::new(false)),
            toggle_key: DEFAULT_LOCK_TOGGLE_KEY,
        }
    }

    /// Returns `true` if input is currently locked to game instances.
    pub fn is_locked(&self) -> bool {
        self.adopt_external_request();
        self.is_locked.load(Ordering::SeqCst)
    }

    /// Takes over a lock state requested through [`request_input_lock`].
    fn adopt_external_request(&self) {
        match EXTERNAL_REQUEST.swap(0, Ordering::SeqCst) {
            1 => self.is_locked.store(true, Ordering::SeqCst),
            2 => self.is_locked.store(false, Ordering::SeqCst),
            _ => {}
        }
    }

    /// Sets the input lock state directly.
    pub fn set_locked(&self, locked: bool) {
        self.is_locked.store(locked, Ordering::SeqCst);
        GLOBAL_LOCKED.store(locked, Ordering::SeqCst);
    }

    /// Toggles the lock state and returns the new state.
    pub fn toggle(&self) -> bool {
        self.adopt_external_request();
        let prev = self.is_locked.fetch_xor(true, Ordering::SeqCst);
        GLOBAL_LOCKED.store(!prev, Ordering::SeqCst);
        !prev
    }

    /// Toggles the lock state and displays the corresponding OSD notification.
    /// Parity with C# `RawInputProcessor.cs:450`:
    /// Locked: "Inputs Locked" (800ms)
    /// Unlocked: "Inputs Unlocked" (800ms)
    pub fn toggle_with_osd(&self, osd: Option<&dyn nucleus_platform::OsdManager>) -> bool {
        let new_state = self.toggle();
        if let Some(mgr) = osd {
            let msg = if new_state {
                "Inputs Locked"
            } else {
                "Inputs Unlocked"
            };
            mgr.show(800, msg);
        }
        new_state
    }

    /// Returns the virtual key code assigned for toggling.
    pub fn toggle_key(&self) -> u32 {
        self.toggle_key
    }

    /// Sets the virtual key code assigned for toggling.
    pub fn set_toggle_key(&mut self, key_code: u32) {
        self.toggle_key = key_code;
    }
}

impl Default for InputLock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleus_platform::OsdManager;

    struct MockOsd {
        last_msg: std::sync::Mutex<Option<(u32, String)>>,
    }

    impl MockOsd {
        fn new() -> Self {
            Self {
                last_msg: std::sync::Mutex::new(None),
            }
        }
    }

    impl nucleus_platform::OsdManager for MockOsd {
        fn show(&self, duration_ms: u32, text: &str) {
            *self.last_msg.lock().unwrap() = Some((duration_ms, text.to_string()));
        }
        fn hide(&self) {
            *self.last_msg.lock().unwrap() = None;
        }
        fn current_message(&self) -> Option<String> {
            self.last_msg
                .lock()
                .unwrap()
                .as_ref()
                .map(|(_, msg)| msg.clone())
        }
        fn set_color(&self, _color: &str) {}
    }

    #[test]
    fn test_input_lock_toggle_and_osd() {
        let lock = InputLock::new();
        let osd = MockOsd::new();

        assert!(!lock.is_locked());

        // Toggle to locked
        let state = lock.toggle_with_osd(Some(&osd));
        assert!(state);
        assert!(lock.is_locked());
        assert_eq!(osd.current_message(), Some("Inputs Locked".to_string()));

        // Toggle to unlocked
        let state = lock.toggle_with_osd(Some(&osd));
        assert!(!state);
        assert!(!lock.is_locked());
        assert_eq!(osd.current_message(), Some("Inputs Unlocked".to_string()));
    }

    #[test]
    fn test_external_lock_request_is_adopted_by_the_ui_lock() {
        // The only test that uses the external request, so it cannot race with others.
        let lock = InputLock::new();
        request_input_lock(true);
        assert!(global_input_locked());
        assert!(lock.is_locked(), "the UI lock adopts the session's request");
        // The lock key now releases instead of needing a second press.
        assert!(!lock.toggle());
        assert!(!global_input_locked());
    }
}
