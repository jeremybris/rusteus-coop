//! Remaining `Game.*` handler properties of the original `GenericGameInfo`, plus the
//! `Game.ProtoInput.*` option block.
//!
//! [`crate::HandlerConfig`] carries the properties the launcher was first built around. Handlers
//! written for the original application use many more; they are collected here so a handler
//! that sets them is honoured instead of silently ignored.

use boa_engine::object::JsObject;
use boa_engine::{Context, JsString, JsValue};
use nucleus_hook::proto_input::{
    DrawFakeCursor, MoveWindowHook, ProtoInputConfig, PutMouseInsideWindow, SetRemoveBorderHook,
    SetWindowPosHook,
};
use serde::{Deserialize, Serialize};

fn get(obj: &JsObject, key: &str, ctx: &mut Context) -> Option<JsValue> {
    obj.get(JsString::from(key), ctx)
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

fn get_bool(obj: &JsObject, key: &str, ctx: &mut Context) -> Option<bool> {
    get(obj, key, ctx).map(|v| v.to_boolean())
}

fn get_string(obj: &JsObject, key: &str, ctx: &mut Context) -> Option<String> {
    let v = get(obj, key, ctx)?;
    if v.is_object() {
        return None;
    }
    v.to_string(ctx).ok().map(|s| s.to_std_string_escaped())
}

fn get_i64(obj: &JsObject, key: &str, ctx: &mut Context) -> Option<i64> {
    let v = get(obj, key, ctx)?;
    if let Some(n) = v.as_number() {
        return Some(n as i64);
    }
    if let Some(b) = v.as_boolean() {
        return Some(b as i64);
    }
    None
}

fn get_string_array(obj: &JsObject, key: &str, ctx: &mut Context) -> Vec<String> {
    let Some(v) = get(obj, key, ctx) else {
        return Vec::new();
    };
    let Some(arr) = v.as_object().filter(|o| o.is_array()).cloned() else {
        return Vec::new();
    };
    let len = arr
        .get(JsString::from("length"), ctx)
        .ok()
        .and_then(|n| n.as_number())
        .unwrap_or(0.0) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(item) = arr.get(i, ctx) {
            if item.is_undefined() || item.is_null() {
                out.push(String::new());
            } else if let Ok(s) = item.to_string(ctx) {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    out
}

macro_rules! legacy_props {
    (
        bools { $( $(#[$bm:meta])* $bf:ident = $bjs:literal, $bdef:expr; )* }
        strings { $( $(#[$sm:meta])* $sf:ident = $sjs:literal; )* }
        arrays { $( $(#[$am:meta])* $af:ident = $ajs:literal; )* }
        callbacks { $( $(#[$cm:meta])* $cf:ident = $cjs:literal; )* }
    ) => {
        /// `Game.*` properties beyond the core [`crate::HandlerConfig`] set.
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct LegacyGameProps {
            $( $(#[$bm])* pub $bf: bool, )*
            $( $(#[$sm])* pub $sf: String, )*
            $( $(#[$am])* pub $af: Vec<String>, )*
            $( $(#[$cm])* pub $cf: bool, )*
        }

        impl Default for LegacyGameProps {
            fn default() -> Self {
                Self {
                    $( $bf: $bdef, )*
                    $( $sf: String::new(), )*
                    $( $af: Vec::new(), )*
                    $( $cf: false, )*
                }
            }
        }

        impl LegacyGameProps {
            /// JS property names handled by this block (used by parity tests).
            pub const JS_NAMES: &'static [&'static str] =
                &[ $( $bjs, )* $( $sjs, )* $( $ajs, )* $( $cjs, )* ];

            /// Reads every property from the evaluated `Game` object.
            pub fn extract(game: &JsObject, ctx: &mut Context) -> Self {
                let mut p = Self::default();
                $( if let Some(v) = get_bool(game, $bjs, ctx) { p.$bf = v; } )*
                $( if let Some(v) = get_string(game, $sjs, ctx) { p.$sf = v; } )*
                $( p.$af = get_string_array(game, $ajs, ctx); )*
                $(
                    p.$cf = get(game, $cjs, ctx)
                        .map(|v| v.is_callable())
                        .unwrap_or(false);
                )*
                p
            }
        }
    };
}

legacy_props! {
    bools {
        binaries_folder_path_fix = "BinariesFolderPathFix", false;
        block_raw_input = "BlockRawInput", false;
        change_ip_per_instance = "ChangeIPPerInstance", false;
        change_ip_per_instance_alt = "ChangeIPPerInstanceAlt", false;
        documents_config_path_no_copy = "DocumentsConfigPathNoCopy", false;
        documents_save_path_no_copy = "DocumentsSavePathNoCopy", false;
        draw_fake_mouse_cursor = "DrawFakeMouseCursor", true;
        draw_fake_mouse_cursor_for_controllers = "DrawFakeMouseCursorForControllers", false;
        fake_focus_send_activate = "FakeFocusSendActivate", true;
        fake_focus_send_activate_ignore_kb = "FakeFocusSendActivateIgnoreKB", false;
        force_bind_ip_delay = "ForceBindIPDelay", false;
        force_bind_ip_no_dummy = "ForceBindIPNoDummy", false;
        force_documents_config_copy = "ForceDocumentsConfigCopy", false;
        force_documents_save_copy = "ForceDocumentsSaveCopy", false;
        force_environment_use = "ForceEnvironmentUse", false;
        /// Re-create the instance folder even when `KeepSymLinkOnExit` left one behind.
        force_symlink = "ForceSymlink", false;
        force_user_profile_config_copy = "ForceUserProfileConfigCopy", false;
        force_user_profile_save_copy = "ForceUserProfileSaveCopy", false;
        forward_window_to_proto_input = "ForwardWindowToProtoInput", false;
        /// Run `Game.Play` only after the instance process has been started.
        game_play_after_launch = "GamePlayAfterLaunch", false;
        /// Run `Game.Play` before emulators, hex edits and utilities are applied.
        game_play_before_game_setup = "GamePlayBeforeGameSetup", false;
        hook_dont_update_legacy_in_mouse_msg = "HookDontUpdateLegacyInMouseMsg", false;
        hook_filter_mouse_messages = "HookFilterMouseMessages", false;
        hook_filter_raw_input = "HookFilterRawInput", false;
        hook_get_async_key_state = "HookGetAsyncKeyState", true;
        hook_get_cursor_pos = "HookGetCursorPos", true;
        hook_get_key_state = "HookGetKeyState", true;
        hook_get_keyboard_state = "HookGetKeyboardState", true;
        hook_init = "HookInit", false;
        hook_init_delay = "HookInitDelay", false;
        hook_mouse_visibility = "HookMouseVisibility", true;
        hook_re_register_raw_input = "HookReRegisterRawInput", false;
        hook_re_register_raw_input_keyboard = "HookReRegisterRawInputKeyboard", true;
        hook_re_register_raw_input_mouse = "HookReRegisterRawInputMouse", true;
        hook_set_cursor_pos = "HookSetCursorPos", true;
        hook_use_legacy_input = "HookUseLegacyInput", false;
        inject_dinput_to_xinput_translation = "InjectDinputToXinputTranslation", false;
        inject_hook_xinput = "InjectHookXinput", false;
        keyboard_player_skip_fake_focus = "KeyboardPlayerSkipFakeFocus", false;
        launch_as_different_users = "LaunchAsDifferentUsers", false;
        launch_as_different_users_alt = "LaunchAsDifferentUsersAlt", false;
        lock_input_at_start = "LockInputAtStart", false;
        lock_input_suspends_explorer = "LockInputSuspendsExplorer", false;
        need_steam_client = "NeedSteamClient", false;
        prevent_game_focus = "PreventGameFocus", false;
        save_custom_user_general_values = "SaveCustomUserGeneralValues", false;
        save_custom_user_player_values = "SaveCustomUserPlayerValues", false;
        save_custom_user_instance_values = "SaveCustomUserInstanceValues", false;
        save_and_edit_custom_user_general_values = "SaveAndEditCustomUserGeneralValues", false;
        save_and_edit_custom_user_player_values = "SaveAndEditCustomUserPlayerValues", false;
        save_and_edit_custom_user_instance_values = "SaveAndEditCustomUserInstanceValues", false;
        send_fake_focus_msg = "SendFakeFocusMsg", false;
        send_normal_keyboard_input = "SendNormalKeyboardInput", true;
        send_normal_mouse_input = "SendNormalMouseInput", true;
        send_scroll_wheel = "SendScrollWheel", false;
        set_window_hook = "SetWindowHook", false;
        set_window_hook_start = "SetWindowHookStart", false;
        split_div_compatibility = "SplitDivCompatibility", true;
        toggle_unfocus_on_inputs_lock = "ToggleUnfocusOnInputsLock", false;
        transfer_nucleus_user_account_profiles = "TransferNucleusUserAccountProfiles", false;
        update_fake_mouse_with_internal_input = "UpdateFakeMouseWithInternalInput", false;
        use_current_user_environment = "UseCurrentUserEnvironment", false;
        use_di8_coop_lvl_unlocker = "UseDI8CoopLvlUnlocker", false;
        /// Deploy the DirectInput blocker `dinput8.dll` next to the game executable.
        use_dinput_blocker = "UseDInputBlocker", false;
        use_goldberg_no_og_steam_dlls = "UseGoldbergNoOGSteamDlls", false;
        /// Assign the IDs in `PlayerSteamIDs` instead of generated ones.
        use_handler_steam_ids = "UseHandlerSteamIds", false;
        /// The handler assigns ProtoInput controllers itself from `Game.Play`.
        use_manual_proto_controllers_setup = "UseManualProtoControllersSetup", false;
        user_profile_config_path_no_copy = "UserProfileConfigPathNoCopy", false;
        /// Deploy the XInput Plus DLLs without writing `XInputPlus.ini`.
        xinput_plus_no_ini = "XInputPlusNoIni", false;
        xinput_plus_old_dll = "XInputPlusOldDll", false;
    }
    strings {
        dpi_awareness_mode = "DpiAwarenessMode";
        fake_focus_instances = "FakeFocusInstances";
        hook_focus_instances = "HookFocusInstances";
        orig_steam_dll_path = "OrigSteamDllPath";
        post_hook_instances = "PostHookInstances";
        start_hook_instances = "StartHookInstances";
    }
    arrays {
        /// `utilName|relativeDestination|instances` entries copied from `utils\User`.
        copy_custom_utils = "CopyCustomUtils";
        copy_env_folders_to_nucleus_accounts = "CopyEnvFoldersToNucleusAccounts";
        /// `sourceFolder|destinationFolder` pairs copied into each instance.
        copy_folders_to = "CopyFoldersTo";
        custom_hotkeys = "CustomHotkeys";
        custom_user_general_prompts = "CustomUserGeneralPrompts";
        custom_user_player_prompts = "CustomUserPlayerPrompts";
        custom_user_instance_prompts = "CustomUserInstancePrompts";
        /// `[instance|]relativeFile` entries deleted from the instance folder.
        delete_files = "DeleteFiles";
        delete_files_in_config_path = "DeleteFilesInConfigPath";
        delete_files_in_save_path = "DeleteFilesInSavePath";
        hardlink_folders_to = "HardlinkFoldersTo";
        /// `find|replace` ASCII patches applied to every instance's executable.
        hex_edit_all_exes = "HexEditAllExes";
        /// `file|find|replace` ASCII patches applied in every instance.
        hex_edit_all_files = "HexEditAllFiles";
        /// `[instance|]address|hexBytes` patches for the executable.
        hex_edit_exe_address = "HexEditExeAddress";
        /// One `file|find|replace` entry per player (index = player slot).
        hex_edit_file = "HexEditFile";
        /// `[instance|]file|address|hexBytes` patches.
        hex_edit_file_address = "HexEditFileAddress";
        player_steam_ids = "PlayerSteamIDs";
        /// `[instance|]from|to` renames/moves inside the instance folder.
        rename_and_or_move_files = "RenameAndOrMoveFiles";
        sse_additional_lines = "SSEAdditionalLines";
        /// Files linked by `Context.ProceedSymlink()`.
        symlink_files = "SymlinkFiles";
        symlink_folders_to = "SymlinkFoldersTo";
    }
    callbacks {
        /// `Game.OnStop` is defined and must run when the session ends.
        has_on_stop = "OnStop";
        has_on_custom_hotkey_1 = "OnCustomHotKey_1";
        has_on_custom_hotkey_2 = "OnCustomHotKey_2";
        has_on_custom_hotkey_3 = "OnCustomHotKey_3";
    }
}

/// JS names accepted under `Game.ProtoInput` (used by parity tests).
pub const PROTO_INPUT_JS_NAMES: &[&str] = &[
    "InjectStartup",
    "InjectRuntime_RemoteLoadMethod",
    "InjectRuntime_EasyHookMethod",
    "InjectRuntime_EasyHookStealthMethod",
    "RegisterRawInputHook",
    "GetRawInputDataHook",
    "MessageFilterHook",
    "GetCursorPosHook",
    "SetCursorPosHook",
    "GetKeyStateHook",
    "GetAsyncKeyStateHook",
    "GetKeyboardStateHook",
    "CursorVisibilityHook",
    "ClipCursorHook",
    "ClipCursorHookCreatesFakeClip",
    "FocusHooks",
    "RenameHandlesHook",
    "XinputHook",
    "DinputDeviceHook",
    "SetWindowPosHook",
    "BlockRawInputHook",
    "FindWindowHook",
    "CreateSingleHIDHook",
    "SetWindowStyleHook",
    "MoveWindowHook",
    "AdjustWindowRectHook",
    "SetRemoveBorderHook",
    "RawInputFilter",
    "MouseMoveFilter",
    "MouseActivateFilter",
    "WindowActivateFilter",
    "WindowActvateAppFilter",
    "MouseWheelFilter",
    "MouseButtonFilter",
    "KeyboardButtonFilter",
    "BlockedMessages",
    "RenameHandles",
    "RenameNamedPipes",
    "SendMouseWheelMessages",
    "SendMouseButtonMessages",
    "SendMouseMovementMessages",
    "SendKeyboardButtonMessages",
    "SendMouseDblClkMessages",
    "EnableFocusMessageLoop",
    "FocusLoopIntervalMilliseconds",
    "FocusLoop_WM_ACTIVATE",
    "FocusLoop_WM_ACTIVATEAPP",
    "FocusLoop_WM_NCACTIVATE",
    "FocusLoop_WM_SETFOCUS",
    "FocusLoop_WM_MOUSEACTIVATE",
    "DrawFakeCursor",
    "AllowFakeCursorOutOfBounds",
    "ExtendFakeCursorBounds",
    "EnableToggleFakeCursorVisibilityShortcut",
    "ToggleFakeCursorVisibilityShortcutVkey",
    "DontShowCursorWhenImageUpdated",
    "PutMouseInsideWindow",
    "UseDinputRedirection",
    "UseOpenXinput",
    "MultipleProtoControllers",
    "DinputHookAlsoHooksGetDeviceState",
    "FreezeExternalInputWhenInputNotLocked",
    "AutoHideTaskbar",
];

/// Reads an enum-typed option that handlers set as `true`, a number, or a member name.
fn enum_ordinal(obj: &JsObject, key: &str, names: &[&str], ctx: &mut Context) -> Option<u32> {
    let v = get(obj, key, ctx)?;
    if let Some(b) = v.as_boolean() {
        return Some(b as u32);
    }
    if let Some(n) = v.as_number() {
        return Some(n.max(0.0) as u32);
    }
    let s = v.as_string()?.to_std_string_escaped();
    names
        .iter()
        .position(|n| n.eq_ignore_ascii_case(s.trim()))
        .map(|i| i as u32)
}

/// Reads the `Game.ProtoInput` option block into a [`ProtoInputConfig`].
pub fn extract_proto_input(game: &JsObject, ctx: &mut Context) -> ProtoInputConfig {
    let mut c = ProtoInputConfig::default();
    let Some(pi) = get(game, "ProtoInput", ctx).and_then(|v| v.as_object().cloned()) else {
        return c;
    };

    macro_rules! b {
        ($field:ident, $key:literal) => {
            if let Some(v) = get_bool(&pi, $key, ctx) {
                c.$field = v;
            }
        };
    }
    b!(inject_startup, "InjectStartup");
    b!(inject_runtime_remote_load, "InjectRuntime_RemoteLoadMethod");
    b!(inject_runtime_easyhook, "InjectRuntime_EasyHookMethod");
    b!(
        inject_runtime_easyhook_stealth,
        "InjectRuntime_EasyHookStealthMethod"
    );
    b!(register_raw_input_hook, "RegisterRawInputHook");
    b!(get_raw_input_data_hook, "GetRawInputDataHook");
    b!(message_filter_hook, "MessageFilterHook");
    b!(get_cursor_pos_hook, "GetCursorPosHook");
    b!(set_cursor_pos_hook, "SetCursorPosHook");
    b!(get_key_state_hook, "GetKeyStateHook");
    b!(get_async_key_state_hook, "GetAsyncKeyStateHook");
    b!(get_keyboard_state_hook, "GetKeyboardStateHook");
    b!(cursor_visibility_hook, "CursorVisibilityHook");
    b!(clip_cursor_hook, "ClipCursorHook");
    b!(
        clip_cursor_hook_creates_fake_clip,
        "ClipCursorHookCreatesFakeClip"
    );
    b!(focus_hooks, "FocusHooks");
    b!(rename_handles_hook, "RenameHandlesHook");
    b!(xinput_hook, "XinputHook");
    b!(dinput_device_hook, "DinputDeviceHook");
    b!(block_raw_input_hook, "BlockRawInputHook");
    b!(find_window_hook, "FindWindowHook");
    b!(create_single_hid_hook, "CreateSingleHIDHook");
    b!(set_window_style_hook, "SetWindowStyleHook");
    b!(adjust_window_rect_hook, "AdjustWindowRectHook");
    b!(raw_input_filter, "RawInputFilter");
    b!(mouse_move_filter, "MouseMoveFilter");
    b!(mouse_activate_filter, "MouseActivateFilter");
    b!(window_activate_filter, "WindowActivateFilter");
    // The original spells this option "WindowActvateAppFilter"; accept the corrected form too.
    b!(window_activate_app_filter, "WindowActivateAppFilter");
    b!(window_activate_app_filter, "WindowActvateAppFilter");
    b!(mouse_wheel_filter, "MouseWheelFilter");
    b!(mouse_button_filter, "MouseButtonFilter");
    b!(keyboard_button_filter, "KeyboardButtonFilter");
    b!(send_mouse_wheel_messages, "SendMouseWheelMessages");
    b!(send_mouse_button_messages, "SendMouseButtonMessages");
    b!(send_mouse_movement_messages, "SendMouseMovementMessages");
    b!(send_keyboard_button_messages, "SendKeyboardButtonMessages");
    b!(send_mouse_dbl_clk_messages, "SendMouseDblClkMessages");
    b!(enable_focus_message_loop, "EnableFocusMessageLoop");
    b!(focus_loop_wm_activate, "FocusLoop_WM_ACTIVATE");
    b!(focus_loop_wm_activateapp, "FocusLoop_WM_ACTIVATEAPP");
    b!(focus_loop_wm_ncactivate, "FocusLoop_WM_NCACTIVATE");
    b!(focus_loop_wm_setfocus, "FocusLoop_WM_SETFOCUS");
    b!(focus_loop_wm_mouseactivate, "FocusLoop_WM_MOUSEACTIVATE");
    b!(
        allow_fake_cursor_out_of_bounds,
        "AllowFakeCursorOutOfBounds"
    );
    b!(extend_fake_cursor_bounds, "ExtendFakeCursorBounds");
    b!(
        enable_toggle_fake_cursor_visibility_shortcut,
        "EnableToggleFakeCursorVisibilityShortcut"
    );
    b!(
        dont_show_cursor_when_image_updated,
        "DontShowCursorWhenImageUpdated"
    );
    b!(use_dinput_redirection, "UseDinputRedirection");
    b!(use_open_xinput, "UseOpenXinput");
    b!(multiple_proto_controllers, "MultipleProtoControllers");
    b!(
        dinput_hook_also_hooks_get_device_state,
        "DinputHookAlsoHooksGetDeviceState"
    );
    b!(
        freeze_external_input_when_input_not_locked,
        "FreezeExternalInputWhenInputNotLocked"
    );
    b!(auto_hide_taskbar, "AutoHideTaskbar");

    if let Some(n) = get_i64(&pi, "FocusLoopIntervalMilliseconds", ctx) {
        c.focus_loop_interval_milliseconds = n as i32;
    }
    if let Some(n) = get_i64(&pi, "ToggleFakeCursorVisibilityShortcutVkey", ctx) {
        c.toggle_fake_cursor_visibility_shortcut_vkey = n as u32;
    }

    const POS: &[&str] = &["None", "True", "DontResize", "DontReposition"];
    c.set_window_pos_hook = match enum_ordinal(&pi, "SetWindowPosHook", POS, ctx) {
        Some(1) => SetWindowPosHook::True,
        Some(2) => SetWindowPosHook::DontResize,
        Some(3) => SetWindowPosHook::DontReposition,
        _ => SetWindowPosHook::None,
    };
    c.move_window_hook = match enum_ordinal(&pi, "MoveWindowHook", POS, ctx) {
        Some(1) => MoveWindowHook::True,
        Some(2) => MoveWindowHook::DontResize,
        Some(3) => MoveWindowHook::DontReposition,
        _ => MoveWindowHook::None,
    };
    c.set_remove_border_hook = match enum_ordinal(
        &pi,
        "SetRemoveBorderHook",
        &["None", "True", "DontWait"],
        ctx,
    ) {
        Some(1) => SetRemoveBorderHook::True,
        Some(2) => SetRemoveBorderHook::DontWait,
        _ => SetRemoveBorderHook::None,
    };
    c.draw_fake_cursor = match enum_ordinal(&pi, "DrawFakeCursor", &["None", "True", "Fix"], ctx) {
        Some(1) => DrawFakeCursor::True,
        Some(2) => DrawFakeCursor::Fix,
        _ => DrawFakeCursor::None,
    };
    c.put_mouse_inside_window = match enum_ordinal(
        &pi,
        "PutMouseInsideWindow",
        &["None", "True", "IgnoreTopLeft", "IgnoreBottomRight"],
        ctx,
    ) {
        Some(1) => PutMouseInsideWindow::True,
        Some(2) => PutMouseInsideWindow::IgnoreTopLeft,
        Some(3) => PutMouseInsideWindow::IgnoreBottomRight,
        _ => PutMouseInsideWindow::None,
    };

    c.rename_handles = get_string_array(&pi, "RenameHandles", ctx);
    c.rename_named_pipes = get_string_array(&pi, "RenameNamedPipes", ctx);
    c.blocked_messages = get_string_array(&pi, "BlockedMessages", ctx)
        .iter()
        .filter_map(|s| {
            let t = s.trim();
            match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => t.parse::<f64>().ok().map(|f| f as u32),
            }
        })
        .collect();
    c
}

/// Reads the `Game.ProtoInput` options served by newer ProtoInput builds: controller/mouse
/// translation, input re-registration, sub-window messages and manual scaling.
pub fn extract_proto_extra(
    game: &JsObject,
    ctx: &mut Context,
) -> nucleus_hook::proto_extras::ProtoInputExtraConfig {
    let mut c = nucleus_hook::proto_extras::ProtoInputExtraConfig::default();
    let Some(pi) = get(game, "ProtoInput", ctx).and_then(|v| v.as_object().cloned()) else {
        return c;
    };
    macro_rules! b {
        ($field:ident, $key:literal) => {
            if let Some(v) = get_bool(&pi, $key, ctx) {
                c.$field = v;
            }
        };
    }
    macro_rules! n {
        ($field:ident, $key:literal) => {
            if let Some(v) = get_i64(&pi, $key, ctx) {
                c.$field = v as i32;
            }
        };
    }
    b!(translate_xinput_to_mkb, "TranslateXinputtoMKB");
    b!(translate_mkb_to_xinput, "TranslateMKBtoXinput");
    b!(re_register_input, "ReRegisterInput");
    b!(pointer_in_mouse, "PointerInMouse");
    b!(message_sub_windows, "MessageSubWindows");
    b!(get_cursor_info_hook, "GetCursorInfoHook");
    b!(set_windows_hook_hook, "SetWindowsHookHook");
    n!(scale_from_x, "ScaleFromX");
    n!(scale_from_y, "ScaleFromY");
    n!(button_a, "TranslateXButtonA");
    n!(button_b, "TranslateXButtonB");
    n!(button_x, "TranslateXButtonX");
    n!(button_y, "TranslateXButtonY");
    n!(button_rs, "TranslateXButtonRS");
    n!(button_ls, "TranslateXButtonLS");
    n!(button_right, "TranslateXButtonRight");
    n!(button_left, "TranslateXButtonLeft");
    n!(button_up, "TranslateXButtonUp");
    n!(button_down, "TranslateXButtonDown");
    n!(stick_r, "TranslateXStickR");
    n!(stick_l, "TranslateXStickL");
    n!(stick_right, "TranslateXStickRight");
    n!(stick_left, "TranslateXStickLeft");
    n!(stick_up, "TranslateXStickUp");
    n!(stick_down, "TranslateXStickDown");
    n!(button_option, "TranslateXButtonOption");
    n!(button_start, "TranslateXButtonStart");
    n!(sensitivity, "TranslateXSensitivity");
    // The original spells this option "TranslateXAccelration".
    n!(acceleration, "TranslateXAcceleration");
    n!(acceleration, "TranslateXAccelration");
    n!(deadzone, "TranslateXDeadzone");
    b!(swap_sticks, "TranslateXSwapSticks");
    b!(scanner, "TranslateXScanner");
    b!(shoulders_next_point, "TranslateXShouldersNextPoint");
    b!(a_static, "TranslateXAstatic");
    b!(a_point_click, "TranslateXAPointClick");
    b!(a_point_move, "TranslateXAPointMove");
    b!(b_static, "TranslateXBstatic");
    b!(b_point_click, "TranslateXBPointClick");
    b!(b_point_move, "TranslateXBPointMove");
    b!(x_static, "TranslateXXstatic");
    b!(x_point_click, "TranslateXXPointClick");
    b!(x_point_move, "TranslateXXPointMove");
    b!(y_static, "TranslateXYstatic");
    b!(y_point_click, "TranslateXYPointClick");
    b!(y_point_move, "TranslateXYPointMove");
    c
}

/// Whether the handler asked for ProtoInput to be injected at all.
pub fn proto_input_requested(c: &ProtoInputConfig) -> bool {
    c.inject_startup
        || c.inject_runtime_remote_load
        || c.inject_runtime_easyhook
        || c.inject_runtime_easyhook_stealth
}

/// Translates the hook settings of the original Nucleus hook DLL (`Game.SetWindowHook`,
/// `Game.HookFilterRawInput`, `Game.SendNormalMouseInput`, ...) into the equivalent
/// ProtoInput configuration.
///
/// That DLL is not part of this project; ProtoInput is its successor and provides the same
/// hooks. Returns `None` when the handler uses none of the older hook features.
pub fn proto_from_legacy_hooks(cfg: &crate::HandlerConfig) -> Option<ProtoInputConfig> {
    let l = &cfg.legacy;
    let multi_kbm = cfg.supports_multiple_keyboards_and_mice;
    let wants_hooks = l.set_window_hook
        || l.set_window_hook_start
        || l.hook_init
        || l.block_raw_input
        || l.inject_hook_xinput
        || l.inject_dinput_to_xinput_translation
        || cfg.prevent_window_deactivation
        || multi_kbm;
    if !wants_hooks {
        return None;
    }

    let mut p = ProtoInputConfig::default();
    // Settings that must be in place before the game creates its window are applied by
    // starting the game through ProtoInput; everything else is injected once it runs.
    if l.hook_init || l.set_window_hook_start || l.block_raw_input {
        p.inject_startup = true;
    } else {
        p.inject_runtime_easyhook = true;
    }

    if l.set_window_hook || l.set_window_hook_start {
        p.set_window_pos_hook = SetWindowPosHook::True;
        p.move_window_hook = MoveWindowHook::True;
    }
    if cfg.prevent_window_deactivation || cfg.hook_focus && multi_kbm {
        p.focus_hooks = true;
        p.window_activate_filter = true;
        p.window_activate_app_filter = true;
        p.mouse_activate_filter = true;
        p.message_filter_hook = true;
    }
    if l.block_raw_input {
        p.block_raw_input_hook = true;
    }
    if l.inject_hook_xinput || l.inject_dinput_to_xinput_translation {
        p.xinput_hook = true;
    }
    if l.inject_dinput_to_xinput_translation {
        p.use_dinput_redirection = true;
    }

    if multi_kbm {
        p.register_raw_input_hook = l.hook_re_register_raw_input || l.hook_filter_raw_input;
        p.get_raw_input_data_hook = l.hook_filter_raw_input;
        p.raw_input_filter = l.hook_filter_raw_input;
        if l.hook_filter_mouse_messages {
            p.message_filter_hook = true;
            p.mouse_move_filter = true;
            p.mouse_button_filter = true;
            p.mouse_wheel_filter = true;
            p.keyboard_button_filter = true;
        }
        p.get_cursor_pos_hook = l.hook_get_cursor_pos;
        p.set_cursor_pos_hook = l.hook_set_cursor_pos;
        p.get_key_state_hook = l.hook_get_key_state;
        p.get_async_key_state_hook = l.hook_get_async_key_state;
        p.get_keyboard_state_hook = l.hook_get_keyboard_state;
        p.cursor_visibility_hook = l.hook_mouse_visibility;
        p.send_mouse_movement_messages = l.send_normal_mouse_input;
        p.send_mouse_button_messages = l.send_normal_mouse_input;
        p.send_mouse_dbl_clk_messages = l.send_normal_mouse_input;
        p.send_keyboard_button_messages = l.send_normal_keyboard_input;
        p.send_mouse_wheel_messages = l.send_scroll_wheel;
        if l.draw_fake_mouse_cursor {
            p.draw_fake_cursor = DrawFakeCursor::True;
        }
        p.enable_focus_message_loop = l.send_fake_focus_msg;
        if l.send_fake_focus_msg {
            p.focus_loop_wm_activate = l.fake_focus_send_activate;
            p.focus_loop_wm_activateapp = l.fake_focus_send_activate;
            p.focus_loop_wm_ncactivate = l.fake_focus_send_activate;
            p.focus_loop_wm_setfocus = true;
            p.focus_loop_wm_mouseactivate = true;
            if cfg.fake_focus_interval > 0 {
                p.focus_loop_interval_milliseconds = cfg.fake_focus_interval;
            }
        }
    }
    Some(p)
}
