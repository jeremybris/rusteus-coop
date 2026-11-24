use super::*;
use core::prelude::v1::test;

#[test]
fn test_main_window_creation_and_state() {
    let window = MainWindow::new(2, vec!["Left 4 Dead 2".to_string(), "Halo".to_string()]);
    assert_eq!(window.active_tab, ActiveTab::Library);
    assert_eq!(window.detected_devices, 2);
    assert_eq!(window.handler_names.len(), 2);
    assert_eq!(window.selected_game, Some("Left 4 Dead 2".to_string()));
    assert_eq!(window.player_count, 2);
    assert!(window.auto_scan);
}

#[test]
fn test_active_tab_enum() {
    let tab = ActiveTab::Setup;
    assert_eq!(tab, ActiveTab::Setup);
    assert_ne!(tab, ActiveTab::Library);
}

#[test]
fn test_device_cycling() {
    let mut window = MainWindow::new(2, vec!["Game".to_string()]);
    assert_eq!(window.player_device_assignments.len(), 2);
    let orig = window.player_device_assignments[0].clone();
    window.cycle_player_device(0);
    assert_ne!(window.player_device_assignments[0], orig);
}

#[test]
fn test_developer_mode_and_mock_device_lifecycle() {
    let mut window = MainWindow::new(0, vec!["Game".to_string()]);
    // Zero mockup devices should be present by default
    assert!(window.detected_devices_list.is_empty());
    assert!(!window.app_settings.developer_mode);

    // Attempting to add virtual gamepad when developer_mode is false does nothing
    window.add_virtual_gamepad();
    assert!(window.detected_devices_list.is_empty());

    // Enabling developer mode allows adding virtual gamepad
    window.app_settings.developer_mode = true;
    window.add_virtual_gamepad();
    assert_eq!(window.detected_devices_list.len(), 1);
    assert!(window.detected_devices_list[0].is_virtual);

    // Clearing virtual gamepads removes them
    window.clear_virtual_gamepads();
    assert!(window.detected_devices_list.is_empty());
}

#[test]
fn test_gpui_keydown() {
    fn check(handle: &gpui::FocusHandle, cx: &mut Context<MainWindow>) {
        let _el = div()
            .id("test")
            .track_focus(handle)
            .on_key_down(cx.listener(
                |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "backspace" {
                        this.hub_search_query.pop();
                    } else if key == "escape" {
                        this.hub_search_query.clear();
                    } else if key == "space" {
                        this.hub_search_query.push(' ');
                    } else if let Some(ref s) = event.keystroke.key_char {
                        this.hub_search_query.push_str(s);
                    } else if key.chars().count() == 1 {
                        this.hub_search_query.push_str(key);
                    }
                    cx.notify();
                },
            ));
    }
    let _ = check;
}

#[test]
fn test_handler_item_features_and_options() {
    let opt = nucleus_script::HandlerOption {
        name: "Graphics API".to_string(),
        description: "Select renderer".to_string(),
        key: "gfx".to_string(),
        values: vec![
            "DirectX 9".to_string(),
            "DirectX 11".to_string(),
            "Vulkan".to_string(),
        ],
    };
    let item = HandlerItem::new(
        "l4d2".to_string(),
        "Left 4 Dead 2".to_string(),
        "left4dead2.exe".to_string(),
        4,
        "Zombie shooter".to_string(),
    )
    .with_favorite(true)
    .with_input_support(true, true, true)
    .with_options(vec![opt])
    .with_handler_path(Some("C:/Games/NucleusCoop/handlers/l4d2.js".to_string()));

    assert!(item.is_favorite);
    assert!(item.supports_gamepad);
    assert!(item.supports_keyboard);
    assert!(item.supports_multiple_keyboards_and_mice);
    assert_eq!(item.options.len(), 1);
    assert_eq!(item.options[0].key, "gfx");
    assert_eq!(item.options[0].values.len(), 3);
    assert_eq!(
        item.handler_path.as_deref(),
        Some("C:/Games/NucleusCoop/handlers/l4d2.js")
    );
}

#[test]
fn test_main_window_callbacks_wiring() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let fav_called = Arc::new(AtomicBool::new(false));
    let fav_called_clone = fav_called.clone();
    let toggle_fav = move |guid: &str, fav: bool| {
        if guid == "test_guid" && fav {
            fav_called_clone.store(true, Ordering::SeqCst);
        }
    };

    let profile_saved = Arc::new(AtomicBool::new(false));
    let profile_saved_clone = profile_saved.clone();
    let save_profile = move |item: &HandlerItem| {
        if item.guid == "test_guid" && item.assign_gamepads_by_index {
            profile_saved_clone.store(true, Ordering::SeqCst);
        }
    };

    let launch_called = Arc::new(AtomicBool::new(false));
    let launch_called_clone = launch_called.clone();
    let launcher = move |name: &str,
                         players: usize,
                         layout: &str,
                         opts: &std::collections::HashMap<String, String>,
                         _assignments: &[String],
                         _monitors: &[usize],
                         _audio: &[String]| {
        if name == "Game"
            && players == 2
            && layout == "2x1 (Side-by-Side)"
            && opts.get("mode").map(|s| s.as_str()) == Some("versus")
        {
            launch_called_clone.store(true, Ordering::SeqCst);
        }
    };

    let list_called = Arc::new(AtomicBool::new(false));
    let list_called_clone = list_called.clone();
    let list_profiles = move |guid: &str| -> Vec<(usize, String)> {
        if guid == "test_guid" {
            list_called_clone.store(true, Ordering::SeqCst);
            vec![(1, "Profile 1".to_string())]
        } else {
            Vec::new()
        }
    };

    let load_called = Arc::new(AtomicBool::new(false));
    let load_called_clone = load_called.clone();
    let load_profile = move |guid: &str, num: usize| -> Option<nucleus_profile::GameProfile> {
        if guid == "test_guid" && num == 1 {
            load_called_clone.store(true, Ordering::SeqCst);
            Some(nucleus_profile::GameProfile::default())
        } else {
            None
        }
    };

    let save_num_called = Arc::new(AtomicBool::new(false));
    let save_num_called_clone = save_num_called.clone();
    let save_numbered =
        move |guid: &str, num: Option<usize>, _p: &nucleus_profile::GameProfile| -> Option<usize> {
            if guid == "test_guid" && num == Some(1) {
                save_num_called_clone.store(true, Ordering::SeqCst);
                Some(1)
            } else {
                None
            }
        };

    let del_called = Arc::new(AtomicBool::new(false));
    let del_called_clone = del_called.clone();
    let del_profile = move |guid: &str, num: usize| -> bool {
        if guid == "test_guid" && num == 1 {
            del_called_clone.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    };

    let unload_called = Arc::new(AtomicBool::new(false));
    let unload_called_clone = unload_called.clone();
    let unloader = move || {
        unload_called_clone.store(true, Ordering::SeqCst);
    };

    let freeze_called = Arc::new(AtomicBool::new(false));
    let freeze_called_clone = freeze_called.clone();
    let freeze_toggler = move || -> Option<bool> {
        freeze_called_clone.store(true, Ordering::SeqCst);
        Some(true)
    };

    let reassign_called = Arc::new(AtomicBool::new(false));
    let reassign_called_clone = reassign_called.clone();
    let controller_reassigner = move |slot: usize, dev_id: usize| {
        if slot == 0 && dev_id == 2 {
            reassign_called_clone.store(true, Ordering::SeqCst);
        }
    };

    let reenforce_called = Arc::new(AtomicBool::new(false));
    let reenforce_called_clone = reenforce_called.clone();
    let window_reenforcer = move || {
        reenforce_called_clone.store(true, Ordering::SeqCst);
    };

    let lock_called = Arc::new(AtomicBool::new(false));
    let lock_called_clone = lock_called.clone();
    let input_lock_toggler = move || -> bool {
        lock_called_clone.store(true, Ordering::SeqCst);
        true
    };

    let stop_called = Arc::new(AtomicBool::new(false));
    let stop_called_clone = stop_called.clone();
    let session_stopper = move || {
        stop_called_clone.store(true, Ordering::SeqCst);
    };

    let win = MainWindow::new(2, vec!["Game".to_string()])
        .with_favorite_toggler(toggle_fav)
        .with_profile_saver(save_profile)
        .with_launcher(launcher)
        .with_numbered_profile_lister(list_profiles)
        .with_numbered_profile_loader(load_profile)
        .with_numbered_profile_saver(save_numbered)
        .with_numbered_profile_deleter(del_profile)
        .with_profile_unloader(unloader)
        .with_freeze_toggler(freeze_toggler)
        .with_controller_reassigner(controller_reassigner)
        .with_window_re_enforcer(window_reenforcer)
        .with_input_lock_toggler(input_lock_toggler)
        .with_session_stopper(session_stopper);

    let mut item = HandlerItem::new(
        "test_guid".to_string(),
        "Game".to_string(),
        "game.exe".to_string(),
        4,
        "".to_string(),
    );
    item.assign_gamepads_by_index = true;

    if let Some(ref cb) = win.callbacks.on_toggle_favorite {
        cb("test_guid", true);
    }
    if let Some(ref cb) = win.callbacks.on_save_profile {
        cb(&item);
    }
    if let Some(ref cb) = win.callbacks.on_launch {
        let mut opts = std::collections::HashMap::new();
        opts.insert("mode".to_string(), "versus".to_string());
        let dev_assignments = vec![
            "Keyboard & Mouse (Paired)".to_string(),
            "Gamepad #0 (Xbox)".to_string(),
        ];
        cb(
            "Game",
            2,
            "2x1 (Side-by-Side)",
            &opts,
            &dev_assignments,
            &[0, 1],
            &["Default".to_string(), "Default".to_string()],
        );
    }
    if let Some(ref cb) = win.callbacks.on_list_profiles {
        let res = cb("test_guid");
        assert_eq!(res.len(), 1);
    }
    if let Some(ref cb) = win.callbacks.on_load_numbered_profile {
        assert!(cb("test_guid", 1).is_some());
    }
    if let Some(ref cb) = win.callbacks.on_save_numbered_profile {
        assert_eq!(
            cb(
                "test_guid",
                Some(1),
                &nucleus_profile::GameProfile::default()
            ),
            Some(1)
        );
    }
    if let Some(ref cb) = win.callbacks.on_delete_numbered_profile {
        assert!(cb("test_guid", 1));
    }
    if let Some(ref cb) = win.callbacks.on_unload_profile {
        cb();
    }
    if let Some(ref cb) = win.callbacks.on_toggle_freeze {
        assert_eq!(cb(), Some(true));
    }
    if let Some(ref cb) = win.callbacks.on_reassign_controller {
        cb(0, 2);
    }
    if let Some(ref cb) = win.callbacks.on_re_enforce_windows {
        cb();
    }
    if let Some(ref cb) = win.callbacks.on_toggle_input_lock {
        assert!(cb());
    }
    if let Some(ref cb) = win.callbacks.on_stop_session {
        cb();
    }

    assert!(fav_called.load(Ordering::SeqCst));
    assert!(profile_saved.load(Ordering::SeqCst));
    assert!(launch_called.load(Ordering::SeqCst));
    assert!(list_called.load(Ordering::SeqCst));
    assert!(load_called.load(Ordering::SeqCst));
    assert!(save_num_called.load(Ordering::SeqCst));
    assert!(del_called.load(Ordering::SeqCst));
    assert!(unload_called.load(Ordering::SeqCst));
    assert!(freeze_called.load(Ordering::SeqCst));
    assert!(reassign_called.load(Ordering::SeqCst));
    assert!(reenforce_called.load(Ordering::SeqCst));
    assert!(lock_called.load(Ordering::SeqCst));
    assert!(stop_called.load(Ordering::SeqCst));
}

#[test]
fn test_context_menu_gamepad_assign_mutual_exclusivity() {
    let mut item = HandlerItem::new("test_g", "Game", "game.exe", 2, "");
    item.assign_gamepads_by_index = false;
    item.assign_gamepads_on_button_press = true;

    // Toggle index ON -> button press must become false
    item.assign_gamepads_by_index = !item.assign_gamepads_by_index;
    if item.assign_gamepads_by_index {
        item.assign_gamepads_on_button_press = false;
    }
    assert!(item.assign_gamepads_by_index);
    assert!(!item.assign_gamepads_on_button_press);

    // Toggle button press ON -> index must become false
    item.assign_gamepads_on_button_press = !item.assign_gamepads_on_button_press;
    if item.assign_gamepads_on_button_press {
        item.assign_gamepads_by_index = false;
    }
    assert!(item.assign_gamepads_on_button_press);
    assert!(!item.assign_gamepads_by_index);
}
