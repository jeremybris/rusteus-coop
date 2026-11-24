//! ProtoInput native hook integration and dynamic FFI runtime for NucleusCoop.
//!
//! Provides 100% backward compatibility with legacy C# NucleusCoop ProtoInput
//! (`ProtoInput.cs`, `ProtoInputEnumsOptions.cs`, `ProtoInputLauncher.cs`, `ProtoInputOptions.cs`).
//!
//! Dynamically resolves and loads `ProtoInputLoader32.dll` / `ProtoInputLoader64.dll`
//! and `ProtoInputUtilDynamic32.dll` / `ProtoInputUtilDynamic64.dll` at runtime without
//! crashing or panicking if the native libraries are absent (e.g. running under test suites
//! or headless CI environments).

use nucleus_common::error::{NucleusError, NucleusResult};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

// ============================================================================
// 1. Enums: Hooks, Message Filters, and Window Options
// ============================================================================

/// ProtoInput Hook identifiers matching legacy C# `ProtoInput.ProtoHookIDs`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtoHookIDs {
    RegisterRawInput = 0,
    GetRawInputData = 1,
    MessageFilter = 2,
    GetCursorPos = 3,
    SetCursorPos = 4,
    GetKeyState = 5,
    GetAsyncKeyState = 6,
    GetKeyboardState = 7,
    CursorVisibilityState = 8,
    ClipCursor = 9,
    FocusHooks = 10,
    RenameHandles = 11,
    Xinput = 12,
    DinputOrder = 13,
    SetWindowPos = 14,
    BlockRawInput = 15,
    FindWindow = 16,
    CreateSingleHID = 17,
    WindowStyle = 18,
    MoveWindow = 19,
    AdjustWindowRect = 20,
    RemoveBorder = 21,
}

/// ProtoInput Message Filter identifiers matching legacy C# `ProtoInput.ProtoMessageFilterIDs`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtoMessageFilterIDs {
    RawInput = 0,
    MouseMove = 1,
    MouseActivate = 2,
    WindowActivate = 3,
    WindowActivateApp = 4,
    MouseWheel = 5,
    MouseButton = 6,
    KeyboardButton = 7,
}

/// Type alias for message filter IDs.
pub type ProtoInputMessageFilterIDs = ProtoMessageFilterIDs;

impl ProtoHookIDs {
    /// Maps the numeric ID handler scripts pass (`ProtoInput.Values.*HookID`) to a hook.
    pub fn from_id(id: u32) -> Option<Self> {
        use ProtoHookIDs::*;
        Some(match id {
            0 => RegisterRawInput,
            1 => GetRawInputData,
            2 => MessageFilter,
            3 => GetCursorPos,
            4 => SetCursorPos,
            5 => GetKeyState,
            6 => GetAsyncKeyState,
            7 => GetKeyboardState,
            8 => CursorVisibilityState,
            9 => ClipCursor,
            10 => FocusHooks,
            11 => RenameHandles,
            12 => Xinput,
            13 => DinputOrder,
            14 => SetWindowPos,
            15 => BlockRawInput,
            16 => FindWindow,
            17 => CreateSingleHID,
            18 => WindowStyle,
            19 => MoveWindow,
            20 => AdjustWindowRect,
            21 => RemoveBorder,
            _ => return None,
        })
    }
}

impl ProtoMessageFilterIDs {
    /// Maps the numeric ID handler scripts pass (`ProtoInput.Values.*FilterID`) to a filter.
    pub fn from_id(id: u32) -> Option<Self> {
        use ProtoMessageFilterIDs::*;
        Some(match id {
            0 => RawInput,
            1 => MouseMove,
            2 => MouseActivate,
            3 => WindowActivate,
            4 => WindowActivateApp,
            5 => MouseWheel,
            6 => MouseButton,
            7 => KeyboardButton,
            _ => return None,
        })
    }
}

/// SetWindowPos hook configuration mode matching C# `SetWindowPosHook`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SetWindowPosHook {
    #[default]
    None = 0,
    True = 1,
    DontResize = 2,
    DontReposition = 3,
}

/// MoveWindow hook configuration mode matching C# `MoveWindowHook`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MoveWindowHook {
    #[default]
    None = 0,
    True = 1,
    DontResize = 2,
    DontReposition = 3,
}

/// DrawFakeCursor hook configuration mode matching C# `DrawFakeCursor`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DrawFakeCursor {
    #[default]
    None = 0,
    True = 1,
    Fix = 2,
}

/// PutMouseInsideWindow hook configuration mode matching C# `PutMouseInsideWindow`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PutMouseInsideWindow {
    #[default]
    None = 0,
    True = 1,
    IgnoreTopLeft = 2,
    IgnoreBottomRight = 3,
}

/// SetRemoveBorderHook hook configuration mode matching C# `SetRemoveBorderHook`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SetRemoveBorderHook {
    #[default]
    None = 0,
    True = 1,
    DontWait = 2,
}

/// Target process bitness for dynamic DLL selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessBitness {
    X86,
    X64,
}

/// Injection strategy matching ProtoInput options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum InjectMethod {
    #[default]
    RemoteLoad,
    EasyHook,
    EasyHookStealth,
}

// ============================================================================
// 2. GUID Translation for DirectInput
// ============================================================================

/// Converts a 16-byte GUID byte array (in Windows/C# `Guid.ToByteArray()` layout)
/// into the Data1, Data2, Data3, Data4a..Data4h parameters expected by `SetDinputDeviceGUID`.
pub fn guid_bytes_to_dinput_fields(
    bytes: &[u8; 16],
) -> (u32, u16, u16, u8, u8, u8, u8, u8, u8, u8, u8) {
    let data1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let data2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let data3 = u16::from_le_bytes([bytes[6], bytes[7]]);
    (
        data1, data2, data3, bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13],
        bytes[14], bytes[15],
    )
}

/// Parses a GUID string (standard 8-4-4-4-12 hex format or wrapped in braces `{...}`)
/// and returns `(Data1, Data2, Data3, Data4a..Data4h)`.
pub fn guid_to_dinput_fields(
    guid_str: &str,
) -> NucleusResult<(u32, u16, u16, u8, u8, u8, u8, u8, u8, u8, u8)> {
    let cleaned = guid_str.trim().trim_matches(|c| c == '{' || c == '}');
    let uuid = uuid::Uuid::parse_str(cleaned).map_err(|e| {
        NucleusError::Platform(format!("Invalid GUID format '{}': {}", guid_str, e))
    })?;

    // In RFC 4122 format, Uuid::as_bytes() provides network order (big-endian).
    // Windows GUID layout stores Data1 as uint (little endian in memory), Data2 as ushort,
    // Data3 as ushort, and Data4 as 8 individual bytes.
    let (d1, d2, d3, d4) = uuid.as_fields();
    let d4_bytes = d4;
    Ok((
        d1,
        d2,
        d3,
        d4_bytes[0],
        d4_bytes[1],
        d4_bytes[2],
        d4_bytes[3],
        d4_bytes[4],
        d4_bytes[5],
        d4_bytes[6],
        d4_bytes[7],
    ))
}

// ============================================================================
// 3. Configuration Models
// ============================================================================

const fn default_focus_loop_interval() -> i32 {
    5
}

const fn default_toggle_vkey() -> u32 {
    0x24 // VK_HOME
}

/// High-level ProtoInput configuration matching `ProtoInputOptions.cs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtoInputConfig {
    #[serde(default)]
    pub inject_startup: bool,
    #[serde(default)]
    pub inject_runtime_remote_load: bool,
    #[serde(default)]
    pub inject_runtime_easyhook: bool,
    #[serde(default)]
    pub inject_runtime_easyhook_stealth: bool,

    #[serde(default)]
    pub register_raw_input_hook: bool,
    #[serde(default)]
    pub get_raw_input_data_hook: bool,
    #[serde(default)]
    pub message_filter_hook: bool,
    #[serde(default)]
    pub get_cursor_pos_hook: bool,
    #[serde(default)]
    pub set_cursor_pos_hook: bool,
    #[serde(default)]
    pub get_key_state_hook: bool,
    #[serde(default)]
    pub get_async_key_state_hook: bool,
    #[serde(default)]
    pub get_keyboard_state_hook: bool,
    #[serde(default)]
    pub cursor_visibility_hook: bool,
    #[serde(default)]
    pub clip_cursor_hook: bool,
    #[serde(default)]
    pub clip_cursor_hook_creates_fake_clip: bool,
    #[serde(default)]
    pub focus_hooks: bool,
    #[serde(default)]
    pub rename_handles_hook: bool,
    #[serde(default)]
    pub xinput_hook: bool,
    #[serde(default)]
    pub dinput_device_hook: bool,
    #[serde(default)]
    pub set_window_pos_hook: SetWindowPosHook,
    #[serde(default)]
    pub block_raw_input_hook: bool,
    #[serde(default)]
    pub find_window_hook: bool,
    #[serde(default)]
    pub create_single_hid_hook: bool,
    #[serde(default)]
    pub set_window_style_hook: bool,
    #[serde(default)]
    pub move_window_hook: MoveWindowHook,
    #[serde(default)]
    pub adjust_window_rect_hook: bool,
    #[serde(default)]
    pub set_remove_border_hook: SetRemoveBorderHook,

    #[serde(default)]
    pub raw_input_filter: bool,
    #[serde(default)]
    pub mouse_move_filter: bool,
    #[serde(default)]
    pub mouse_activate_filter: bool,
    #[serde(default)]
    pub window_activate_filter: bool,
    #[serde(default)]
    pub window_activate_app_filter: bool,
    #[serde(default)]
    pub mouse_wheel_filter: bool,
    #[serde(default)]
    pub mouse_button_filter: bool,
    #[serde(default)]
    pub keyboard_button_filter: bool,

    #[serde(default)]
    pub blocked_messages: Vec<u32>,

    #[serde(default)]
    pub rename_handles: Vec<String>,
    #[serde(default)]
    pub rename_named_pipes: Vec<String>,

    #[serde(default)]
    pub send_mouse_wheel_messages: bool,
    #[serde(default)]
    pub send_mouse_button_messages: bool,
    #[serde(default)]
    pub send_mouse_movement_messages: bool,
    #[serde(default)]
    pub send_keyboard_button_messages: bool,
    #[serde(default)]
    pub send_mouse_dbl_clk_messages: bool,

    #[serde(default)]
    pub enable_focus_message_loop: bool,
    #[serde(default = "default_focus_loop_interval")]
    pub focus_loop_interval_milliseconds: i32,
    #[serde(default)]
    pub focus_loop_wm_activate: bool,
    #[serde(default)]
    pub focus_loop_wm_activateapp: bool,
    #[serde(default)]
    pub focus_loop_wm_ncactivate: bool,
    #[serde(default)]
    pub focus_loop_wm_setfocus: bool,
    #[serde(default)]
    pub focus_loop_wm_mouseactivate: bool,

    #[serde(default)]
    pub draw_fake_cursor: DrawFakeCursor,
    #[serde(default)]
    pub allow_fake_cursor_out_of_bounds: bool,
    #[serde(default)]
    pub extend_fake_cursor_bounds: bool,
    #[serde(default)]
    pub enable_toggle_fake_cursor_visibility_shortcut: bool,
    #[serde(default = "default_toggle_vkey")]
    pub toggle_fake_cursor_visibility_shortcut_vkey: u32,
    #[serde(default)]
    pub dont_show_cursor_when_image_updated: bool,
    #[serde(default)]
    pub put_mouse_inside_window: PutMouseInsideWindow,

    #[serde(default)]
    pub use_dinput_redirection: bool,
    #[serde(default)]
    pub use_open_xinput: bool,
    #[serde(default)]
    pub multiple_proto_controllers: bool,
    #[serde(default)]
    pub dinput_hook_also_hooks_get_device_state: bool,
    #[serde(default)]
    pub freeze_external_input_when_input_not_locked: bool,
    #[serde(default)]
    pub auto_hide_taskbar: bool,
}

impl Default for ProtoInputConfig {
    fn default() -> Self {
        Self {
            inject_startup: false,
            inject_runtime_remote_load: false,
            inject_runtime_easyhook: false,
            inject_runtime_easyhook_stealth: false,
            register_raw_input_hook: false,
            get_raw_input_data_hook: false,
            message_filter_hook: false,
            get_cursor_pos_hook: false,
            set_cursor_pos_hook: false,
            get_key_state_hook: false,
            get_async_key_state_hook: false,
            get_keyboard_state_hook: false,
            cursor_visibility_hook: false,
            clip_cursor_hook: false,
            clip_cursor_hook_creates_fake_clip: false,
            focus_hooks: false,
            rename_handles_hook: false,
            xinput_hook: false,
            dinput_device_hook: false,
            set_window_pos_hook: SetWindowPosHook::None,
            block_raw_input_hook: false,
            find_window_hook: false,
            create_single_hid_hook: false,
            set_window_style_hook: false,
            move_window_hook: MoveWindowHook::None,
            adjust_window_rect_hook: false,
            set_remove_border_hook: SetRemoveBorderHook::None,
            raw_input_filter: false,
            mouse_move_filter: false,
            mouse_activate_filter: false,
            window_activate_filter: false,
            window_activate_app_filter: false,
            mouse_wheel_filter: false,
            mouse_button_filter: false,
            keyboard_button_filter: false,
            blocked_messages: Vec::new(),
            rename_handles: Vec::new(),
            rename_named_pipes: Vec::new(),
            send_mouse_wheel_messages: false,
            send_mouse_button_messages: false,
            send_mouse_movement_messages: false,
            send_keyboard_button_messages: false,
            send_mouse_dbl_clk_messages: false,
            enable_focus_message_loop: false,
            focus_loop_interval_milliseconds: 5,
            focus_loop_wm_activate: false,
            focus_loop_wm_activateapp: false,
            focus_loop_wm_ncactivate: false,
            focus_loop_wm_setfocus: false,
            focus_loop_wm_mouseactivate: false,
            draw_fake_cursor: DrawFakeCursor::None,
            allow_fake_cursor_out_of_bounds: false,
            extend_fake_cursor_bounds: false,
            enable_toggle_fake_cursor_visibility_shortcut: false,
            toggle_fake_cursor_visibility_shortcut_vkey: 0x24,
            dont_show_cursor_when_image_updated: false,
            put_mouse_inside_window: PutMouseInsideWindow::None,
            use_dinput_redirection: false,
            use_open_xinput: false,
            multiple_proto_controllers: false,
            dinput_hook_also_hooks_get_device_state: false,
            freeze_external_input_when_input_not_locked: false,
            auto_hide_taskbar: false,
        }
    }
}

impl ProtoInputConfig {
    /// Builder for keyboard & mouse redirection profile.
    pub fn with_raw_input(mut self) -> Self {
        self.register_raw_input_hook = true;
        self.get_raw_input_data_hook = true;
        self.message_filter_hook = true;
        self.get_cursor_pos_hook = true;
        self.set_cursor_pos_hook = true;
        self.get_key_state_hook = true;
        self.get_async_key_state_hook = true;
        self.get_keyboard_state_hook = true;
        self.raw_input_filter = true;
        self.mouse_move_filter = true;
        self.mouse_button_filter = true;
        self.keyboard_button_filter = true;
        self.send_mouse_button_messages = true;
        self.send_mouse_movement_messages = true;
        self.send_keyboard_button_messages = true;
        self
    }

    /// Builder for fake cursor options.
    pub fn with_fake_cursor(mut self, mode: DrawFakeCursor) -> Self {
        self.draw_fake_cursor = mode;
        self.cursor_visibility_hook = true;
        self
    }

    /// Builder for focus message loop.
    pub fn with_focus_message_loop(mut self, interval_ms: i32) -> Self {
        self.enable_focus_message_loop = true;
        self.focus_loop_interval_milliseconds = interval_ms;
        self.focus_loop_wm_activate = true;
        self.focus_loop_wm_activateapp = true;
        self.focus_loop_wm_ncactivate = true;
        self.focus_loop_wm_setfocus = true;
        self.focus_loop_wm_mouseactivate = true;
        self
    }
}

/// Player instance configuration passed during ProtoInput setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerProtoInputConfig {
    pub instance_handle: u32,
    pub raw_hid: Option<String>,
    pub is_dinput: bool,
    pub gamepad_guid: Option<String>,
    pub mouse_handle: i32,
    pub keyboard_handle: i32,
    pub controller_index: u32,
    pub controller_index2: u32,
    pub controller_index3: u32,
    pub controller_index4: u32,
    pub monitor_bounds_x: i32,
    pub monitor_bounds_y: i32,
    pub monitor_bounds_width: i32,
    pub monitor_bounds_height: i32,
}

impl Default for PlayerProtoInputConfig {
    fn default() -> Self {
        Self {
            instance_handle: 0,
            raw_hid: None,
            is_dinput: false,
            gamepad_guid: None,
            mouse_handle: -1,
            keyboard_handle: -1,
            controller_index: 0,
            controller_index2: 0,
            controller_index3: 0,
            controller_index4: 0,
            monitor_bounds_x: 0,
            monitor_bounds_y: 0,
            monitor_bounds_width: 1920,
            monitor_bounds_height: 1080,
        }
    }
}

impl PlayerProtoInputConfig {
    pub fn new(instance_handle: u32) -> Self {
        Self {
            instance_handle,
            ..Default::default()
        }
    }

    pub fn with_bounds(mut self, x: i32, y: i32, width: i32, height: i32) -> Self {
        self.monitor_bounds_x = x;
        self.monitor_bounds_y = y;
        self.monitor_bounds_width = width;
        self.monitor_bounds_height = height;
        self
    }

    pub fn with_raw_handles(mut self, keyboard: i32, mouse: i32) -> Self {
        self.keyboard_handle = keyboard;
        self.mouse_handle = mouse;
        self
    }

    pub fn with_controllers(mut self, c1: u32, c2: u32, c3: u32, c4: u32) -> Self {
        self.controller_index = c1;
        self.controller_index2 = c2;
        self.controller_index3 = c3;
        self.controller_index4 = c4;
        self
    }
}

// ============================================================================
// 4. Raw C-ABI Function Signatures
// ============================================================================

pub type FnRemoteLoadLibraryInjectRuntime =
    unsafe extern "C" fn(pid: u32, dll_folder_path: *const u16) -> u32;
pub type FnEasyHookInjectRuntime =
    unsafe extern "C" fn(pid: u32, dll_folder_path: *const u16) -> u32;
pub type FnEasyHookStealthInjectRuntime =
    unsafe extern "C" fn(pid: u32, dll_folder_path: *const u16) -> u32;
pub type FnEasyHookInjectStartup = unsafe extern "C" fn(
    exe_path: *const u16,
    cmd_line: *const u16,
    flags: u32,
    dll_folder_path: *const u16,
    out_pid: *mut u32,
    environment: *mut c_void,
) -> u32;
pub type FnInstallHook = unsafe extern "C" fn(instance_handle: u32, hook_id: u32);
pub type FnUninstallHook = unsafe extern "C" fn(instance_handle: u32, hook_id: u32);
pub type FnEnableMessageFilter = unsafe extern "C" fn(instance_handle: u32, filter_id: u32);
pub type FnDisableMessageFilter = unsafe extern "C" fn(instance_handle: u32, filter_id: u32);
pub type FnEnableMessageBlock = unsafe extern "C" fn(instance_handle: u32, message_id: u32);
pub type FnDisableMessageBlock = unsafe extern "C" fn(instance_handle: u32, message_id: u32);
pub type FnWakeUpProcess = unsafe extern "C" fn(instance_handle: u32);
pub type FnUpdateMainWindowHandle = unsafe extern "C" fn(instance_handle: u32, hwnd: u64);
pub type FnSetupState = unsafe extern "C" fn(instance_handle: u32, instance_index: i32);
pub type FnSetupMessagesToSend = unsafe extern "C" fn(
    instance_handle: u32,
    wheel: bool,
    button: bool,
    move_msg: bool,
    keyboard: bool,
    dblclk: bool,
);
pub type FnStartFocusMessageLoop = unsafe extern "C" fn(
    instance_handle: u32,
    ms: i32,
    activate: bool,
    activateapp: bool,
    ncactivate: bool,
    setfocus: bool,
    mouseactivate: bool,
);
pub type FnStopFocusMessageLoop = unsafe extern "C" fn(instance_handle: u32);
pub type FnSetDrawFakeCursor = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetDrawFakeCursorFix = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetExternalFreezeFakeInput =
    unsafe extern "C" fn(instance_handle: u32, enable_freeze: bool);
pub type FnAddSelectedMouseHandle = unsafe extern "C" fn(instance_handle: u32, mouse_handle: u32);
pub type FnAddSelectedKeyboardHandle =
    unsafe extern "C" fn(instance_handle: u32, keyboard_handle: u32);
pub type FnSetControllerIndex =
    unsafe extern "C" fn(instance_handle: u32, c1: u32, c2: u32, c3: u32, c4: u32);
pub type FnSetUseDinputRedirection =
    unsafe extern "C" fn(instance_handle: u32, use_redirection: bool);
pub type FnSetUseOpenXinput = unsafe extern "C" fn(instance_handle: u32, use_open_xinput: bool);
pub type FnSetShowCursorWhenImageUpdated = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetPutMouseInsideWindow = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetDefaultTopLeftMouseBounds = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetDefaultBottomRightMouseBounds =
    unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetDontWaitWindowBorder = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnAddHandleToRename = unsafe extern "C" fn(instance_handle: u32, name: *const u16);
pub type FnAddNamedPipeToRename = unsafe extern "C" fn(instance_handle: u32, name: *const u16);
pub type FnSetDinputDeviceGUID = unsafe extern "C" fn(
    instance_handle: u32,
    data1: u32,
    data2: u16,
    data3: u16,
    data4a: u8,
    data4b: u8,
    data4c: u8,
    data4d: u8,
    data4e: u8,
    data4f: u8,
    data4g: u8,
    data4h: u8,
);
pub type FnDinputHookAlsoHooksGetDeviceState =
    unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetSetWindowPosSettings =
    unsafe extern "C" fn(instance_handle: u32, posx: i32, posy: i32, width: i32, height: i32);
pub type FnSetSetWindowPosDontResize = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetSetWindowPosDontReposition = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetCreateSingleHIDName = unsafe extern "C" fn(instance_handle: u32, name: *const u16);
pub type FnSetCursorClipOptions =
    unsafe extern "C" fn(instance_handle: u32, use_fake_clip_cursor: bool);
pub type FnAllowFakeCursorOutOfBounds =
    unsafe extern "C" fn(instance_handle: u32, allow_out_of_bounds: bool, extend_bounds: bool);
pub type FnSetToggleFakeCursorVisibilityShortcut =
    unsafe extern "C" fn(instance_handle: u32, enabled: bool, vkey: u32);
pub type FnSetRawInputBypass = unsafe extern "C" fn(instance_handle: u32, enabled: bool);
pub type FnSetMoveWindowSettings =
    unsafe extern "C" fn(instance_handle: u32, posx: i32, posy: i32, width: i32, height: i32);
pub type FnSetMoveWindowDontResize = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetMoveWindowDontReposition = unsafe extern "C" fn(instance_handle: u32, enable: bool);
pub type FnSetAdjustWindowRectSettings =
    unsafe extern "C" fn(instance_handle: u32, posx: i32, posy: i32, width: i32, height: i32);

// ProtoInputUtilDynamic exported signatures
pub type FnLockInput = unsafe extern "C" fn(lock: bool) -> u32;
pub type FnSuspendExplorer = unsafe extern "C" fn();
pub type FnRestartExplorer = unsafe extern "C" fn();
pub type FnSetTaskbarVisibility = unsafe extern "C" fn(auto_hide: bool, always_on_top: bool);
pub type FnGetTaskbarVisibility =
    unsafe extern "C" fn(out_auto_hide: *mut bool, out_always_on_top: *mut bool);

// ============================================================================
// 5. Function Pointer Tables
// ============================================================================

#[derive(Clone, Copy)]
pub struct ProtoInputLoaderFunctions {
    pub remote_load_library_inject_runtime: FnRemoteLoadLibraryInjectRuntime,
    pub easy_hook_inject_runtime: FnEasyHookInjectRuntime,
    pub easy_hook_stealth_inject_runtime: FnEasyHookStealthInjectRuntime,
    pub easy_hook_inject_startup: FnEasyHookInjectStartup,
    pub install_hook: FnInstallHook,
    pub uninstall_hook: FnUninstallHook,
    pub enable_message_filter: FnEnableMessageFilter,
    pub disable_message_filter: FnDisableMessageFilter,
    pub enable_message_block: FnEnableMessageBlock,
    pub disable_message_block: FnDisableMessageBlock,
    pub wake_up_process: FnWakeUpProcess,
    pub update_main_window_handle: FnUpdateMainWindowHandle,
    pub setup_state: FnSetupState,
    pub setup_messages_to_send: FnSetupMessagesToSend,
    pub start_focus_message_loop: FnStartFocusMessageLoop,
    pub stop_focus_message_loop: FnStopFocusMessageLoop,
    pub set_draw_fake_cursor: FnSetDrawFakeCursor,
    pub set_draw_fake_cursor_fix: FnSetDrawFakeCursorFix,
    pub set_external_freeze_fake_input: FnSetExternalFreezeFakeInput,
    pub add_selected_mouse_handle: FnAddSelectedMouseHandle,
    pub add_selected_keyboard_handle: FnAddSelectedKeyboardHandle,
    pub set_controller_index: FnSetControllerIndex,
    pub set_use_dinput_redirection: FnSetUseDinputRedirection,
    pub set_use_open_xinput: FnSetUseOpenXinput,
    pub set_show_cursor_when_image_updated: FnSetShowCursorWhenImageUpdated,
    pub set_put_mouse_inside_window: FnSetPutMouseInsideWindow,
    pub set_default_top_left_mouse_bounds: FnSetDefaultTopLeftMouseBounds,
    pub set_default_bottom_right_mouse_bounds: FnSetDefaultBottomRightMouseBounds,
    pub set_dont_wait_window_border: FnSetDontWaitWindowBorder,
    pub add_handle_to_rename: FnAddHandleToRename,
    pub add_named_pipe_to_rename: FnAddNamedPipeToRename,
    pub set_dinput_device_guid: FnSetDinputDeviceGUID,
    pub dinput_hook_also_hooks_get_device_state: FnDinputHookAlsoHooksGetDeviceState,
    pub set_set_window_pos_settings: FnSetSetWindowPosSettings,
    pub set_set_window_pos_dont_resize: FnSetSetWindowPosDontResize,
    pub set_set_window_pos_dont_reposition: FnSetSetWindowPosDontReposition,
    pub set_create_single_hid_name: FnSetCreateSingleHIDName,
    pub set_cursor_clip_options: FnSetCursorClipOptions,
    pub allow_fake_cursor_out_of_bounds: FnAllowFakeCursorOutOfBounds,
    pub set_toggle_fake_cursor_visibility_shortcut: FnSetToggleFakeCursorVisibilityShortcut,
    pub set_raw_input_bypass: FnSetRawInputBypass,
    pub set_move_window_settings: FnSetMoveWindowSettings,
    pub set_move_window_dont_resize: FnSetMoveWindowDontResize,
    pub set_move_window_dont_reposition: FnSetMoveWindowDontReposition,
    pub set_adjust_window_rect_settings: FnSetAdjustWindowRectSettings,
}

#[derive(Clone, Copy)]
pub struct ProtoInputUtilFunctions {
    pub lock_input: FnLockInput,
    pub suspend_explorer: FnSuspendExplorer,
    pub restart_explorer: FnRestartExplorer,
    pub set_taskbar_visibility: FnSetTaskbarVisibility,
    pub get_taskbar_visibility: FnGetTaskbarVisibility,
}

// ============================================================================
// 6. Native Loader Implementations (Windows & Stubs)
// ============================================================================

/// Safe RAII wrapper around `ProtoInputLoader32.dll` or `ProtoInputLoader64.dll`.
pub struct ProtoInputLoader {
    dll_path: PathBuf,
    bitness: ProcessBitness,
    funcs: ProtoInputLoaderFunctions,
    #[cfg(windows)]
    module: windows::Win32::Foundation::HMODULE,
}

// ProtoInputLoader holds raw function pointers and an OS module handle which is safe to Send/Sync.
unsafe impl Send for ProtoInputLoader {}
unsafe impl Sync for ProtoInputLoader {}

impl ProtoInputLoader {
    /// Loads ProtoInputLoader from the given DLL path.
    pub fn load(dll_path: impl AsRef<Path>, bitness: ProcessBitness) -> NucleusResult<Self> {
        let path = dll_path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(NucleusError::Platform(format!(
                "ProtoInput loader DLL does not exist: {:?}",
                path
            )));
        }

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
            use windows::core::{PCSTR, PCWSTR};

            let wide_path: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // SAFETY: Loading DLL dynamically with absolute verified path.
            let module = unsafe { LoadLibraryW(PCWSTR(wide_path.as_ptr())) }.map_err(|e| {
                NucleusError::Platform(format!("Failed to load ProtoInput DLL '{:?}': {}", path, e))
            })?;

            // Helper macro to resolve exported symbols.
            macro_rules! resolve_sym {
                ($name:ident, $sym_str:literal, $ty:ty) => {{
                    let cstr = concat!($sym_str, "\0");
                    // SAFETY: module is valid; symbol name is null-terminated ASCII.
                    let proc = unsafe { GetProcAddress(module, PCSTR(cstr.as_ptr())) };
                    match proc {
                        Some(p) => unsafe { std::mem::transmute_copy::<_, $ty>(&p) },
                        None => {
                            unsafe {
                                let _ = windows::Win32::Foundation::FreeLibrary(module);
                            }
                            return Err(NucleusError::Platform(format!(
                                "Missing symbol '{}' in ProtoInput DLL '{:?}'",
                                $sym_str, path
                            )));
                        }
                    }
                }};
            }

            let funcs = ProtoInputLoaderFunctions {
                remote_load_library_inject_runtime: resolve_sym!(
                    remote_load,
                    "RemoteLoadLibraryInjectRuntime",
                    FnRemoteLoadLibraryInjectRuntime
                ),
                easy_hook_inject_runtime: resolve_sym!(
                    eh_inject,
                    "EasyHookInjectRuntime",
                    FnEasyHookInjectRuntime
                ),
                easy_hook_stealth_inject_runtime: resolve_sym!(
                    eh_stealth,
                    "EasyHookStealthInjectRuntime",
                    FnEasyHookStealthInjectRuntime
                ),
                easy_hook_inject_startup: resolve_sym!(
                    eh_startup,
                    "EasyHookInjectStartup",
                    FnEasyHookInjectStartup
                ),
                install_hook: resolve_sym!(install_hook, "InstallHook", FnInstallHook),
                uninstall_hook: resolve_sym!(uninstall_hook, "UninstallHook", FnUninstallHook),
                enable_message_filter: resolve_sym!(
                    enable_mf,
                    "EnableMessageFilter",
                    FnEnableMessageFilter
                ),
                disable_message_filter: resolve_sym!(
                    disable_mf,
                    "DisableMessageFilter",
                    FnDisableMessageFilter
                ),
                enable_message_block: resolve_sym!(
                    enable_mb,
                    "EnableMessageBlock",
                    FnEnableMessageBlock
                ),
                disable_message_block: resolve_sym!(
                    disable_mb,
                    "DisableMessageBlock",
                    FnDisableMessageBlock
                ),
                wake_up_process: resolve_sym!(wake_up, "WakeUpProcess", FnWakeUpProcess),
                update_main_window_handle: resolve_sym!(
                    update_hwnd,
                    "UpdateMainWindowHandle",
                    FnUpdateMainWindowHandle
                ),
                setup_state: resolve_sym!(setup_state, "SetupState", FnSetupState),
                setup_messages_to_send: resolve_sym!(
                    setup_msgs,
                    "SetupMessagesToSend",
                    FnSetupMessagesToSend
                ),
                start_focus_message_loop: resolve_sym!(
                    start_loop,
                    "StartFocusMessageLoop",
                    FnStartFocusMessageLoop
                ),
                stop_focus_message_loop: resolve_sym!(
                    stop_loop,
                    "StopFocusMessageLoop",
                    FnStopFocusMessageLoop
                ),
                set_draw_fake_cursor: resolve_sym!(
                    draw_cursor,
                    "SetDrawFakeCursor",
                    FnSetDrawFakeCursor
                ),
                set_draw_fake_cursor_fix: resolve_sym!(
                    draw_cursor_fix,
                    "SetDrawFakeCursorFix",
                    FnSetDrawFakeCursorFix
                ),
                set_external_freeze_fake_input: resolve_sym!(
                    freeze_input,
                    "SetExternalFreezeFakeInput",
                    FnSetExternalFreezeFakeInput
                ),
                add_selected_mouse_handle: resolve_sym!(
                    mouse_handle,
                    "AddSelectedMouseHandle",
                    FnAddSelectedMouseHandle
                ),
                add_selected_keyboard_handle: resolve_sym!(
                    kb_handle,
                    "AddSelectedKeyboardHandle",
                    FnAddSelectedKeyboardHandle
                ),
                set_controller_index: resolve_sym!(
                    ctrl_index,
                    "SetControllerIndex",
                    FnSetControllerIndex
                ),
                set_use_dinput_redirection: resolve_sym!(
                    dinput_redir,
                    "SetUseDinputRedirection",
                    FnSetUseDinputRedirection
                ),
                set_use_open_xinput: resolve_sym!(
                    open_xinput,
                    "SetUseOpenXinput",
                    FnSetUseOpenXinput
                ),
                set_show_cursor_when_image_updated: resolve_sym!(
                    show_cursor,
                    "SetShowCursorWhenImageUpdated",
                    FnSetShowCursorWhenImageUpdated
                ),
                set_put_mouse_inside_window: resolve_sym!(
                    mouse_inside,
                    "SetPutMouseInsideWindow",
                    FnSetPutMouseInsideWindow
                ),
                set_default_top_left_mouse_bounds: resolve_sym!(
                    tl_bounds,
                    "SetDefaultTopLeftMouseBounds",
                    FnSetDefaultTopLeftMouseBounds
                ),
                set_default_bottom_right_mouse_bounds: resolve_sym!(
                    br_bounds,
                    "SetDefaultBottomRightMouseBounds",
                    FnSetDefaultBottomRightMouseBounds
                ),
                set_dont_wait_window_border: resolve_sym!(
                    dont_wait,
                    "SetDontWaitWindowBorder",
                    FnSetDontWaitWindowBorder
                ),
                add_handle_to_rename: resolve_sym!(
                    handle_rename,
                    "AddHandleToRename",
                    FnAddHandleToRename
                ),
                add_named_pipe_to_rename: resolve_sym!(
                    pipe_rename,
                    "AddNamedPipeToRename",
                    FnAddNamedPipeToRename
                ),
                set_dinput_device_guid: resolve_sym!(
                    dinput_guid,
                    "SetDinputDeviceGUID",
                    FnSetDinputDeviceGUID
                ),
                dinput_hook_also_hooks_get_device_state: resolve_sym!(
                    dinput_state,
                    "DinputHookAlsoHooksGetDeviceState",
                    FnDinputHookAlsoHooksGetDeviceState
                ),
                set_set_window_pos_settings: resolve_sym!(
                    swp_settings,
                    "SetSetWindowPosSettings",
                    FnSetSetWindowPosSettings
                ),
                set_set_window_pos_dont_resize: resolve_sym!(
                    swp_no_resize,
                    "SetSetWindowPosDontResize",
                    FnSetSetWindowPosDontResize
                ),
                set_set_window_pos_dont_reposition: resolve_sym!(
                    swp_no_repos,
                    "SetSetWindowPosDontReposition",
                    FnSetSetWindowPosDontReposition
                ),
                set_create_single_hid_name: resolve_sym!(
                    hid_name,
                    "SetCreateSingleHIDName",
                    FnSetCreateSingleHIDName
                ),
                set_cursor_clip_options: resolve_sym!(
                    clip_options,
                    "SetCursorClipOptions",
                    FnSetCursorClipOptions
                ),
                allow_fake_cursor_out_of_bounds: resolve_sym!(
                    cursor_bounds,
                    "AllowFakeCursorOutOfBounds",
                    FnAllowFakeCursorOutOfBounds
                ),
                set_toggle_fake_cursor_visibility_shortcut: resolve_sym!(
                    cursor_shortcut,
                    "SetToggleFakeCursorVisibilityShortcut",
                    FnSetToggleFakeCursorVisibilityShortcut
                ),
                set_raw_input_bypass: resolve_sym!(
                    raw_bypass,
                    "SetRawInputBypass",
                    FnSetRawInputBypass
                ),
                set_move_window_settings: resolve_sym!(
                    mw_settings,
                    "SetMoveWindowSettings",
                    FnSetMoveWindowSettings
                ),
                set_move_window_dont_resize: resolve_sym!(
                    mw_no_resize,
                    "SetMoveWindowDontResize",
                    FnSetMoveWindowDontResize
                ),
                set_move_window_dont_reposition: resolve_sym!(
                    mw_no_repos,
                    "SetMoveWindowDontReposition",
                    FnSetMoveWindowDontReposition
                ),
                set_adjust_window_rect_settings: resolve_sym!(
                    awr_settings,
                    "SetAdjustWindowRectSettings",
                    FnSetAdjustWindowRectSettings
                ),
            };

            Ok(Self {
                dll_path: path,
                bitness,
                funcs,
                module,
            })
        }

        #[cfg(not(windows))]
        {
            Err(NucleusError::Platform(
                "ProtoInput loader is only supported on Windows".to_string(),
            ))
        }
    }

    /// Searches standard directories for the appropriate loader DLL based on bitness.
    pub fn find_and_load(base_dir: Option<&Path>, bitness: ProcessBitness) -> NucleusResult<Self> {
        let dll_filename = match bitness {
            ProcessBitness::X86 => "ProtoInputLoader32.dll",
            ProcessBitness::X64 => "ProtoInputLoader64.dll",
        };

        let candidate_paths = [
            base_dir.map(|d| d.join(dll_filename)),
            base_dir.map(|d| d.join("utils").join("ProtoInput").join(dll_filename)),
            base_dir.map(|d| d.join("ProtoInput").join(dll_filename)),
            std::env::current_exe().ok().and_then(|p| {
                p.parent()
                    .map(|d| d.join("utils").join("ProtoInput").join(dll_filename))
            }),
            Some(PathBuf::from("utils").join("ProtoInput").join(dll_filename)),
            Some(PathBuf::from(dll_filename)),
        ];

        for candidate in candidate_paths.into_iter().flatten() {
            if candidate.exists() {
                return Self::load(&candidate, bitness);
            }
        }

        Err(NucleusError::Platform(format!(
            "ProtoInput loader DLL '{}' could not be found in candidate directories",
            dll_filename
        )))
    }

    pub fn bitness(&self) -> ProcessBitness {
        self.bitness
    }

    pub fn dll_path(&self) -> &Path {
        &self.dll_path
    }

    // Direct C-ABI invocations with safe ergonomic wrappers

    pub fn remote_load_library_inject_runtime(&self, pid: u32, dll_folder_path: &str) -> u32 {
        let wide = to_wide_null(dll_folder_path);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with valid null-terminated wide string.
        unsafe { (self.funcs.remote_load_library_inject_runtime)(pid, wide.as_ptr()) }
    }

    pub fn easy_hook_inject_runtime(&self, pid: u32, dll_folder_path: &str) -> u32 {
        let wide = to_wide_null(dll_folder_path);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with valid null-terminated wide string.
        unsafe { (self.funcs.easy_hook_inject_runtime)(pid, wide.as_ptr()) }
    }

    pub fn easy_hook_stealth_inject_runtime(&self, pid: u32, dll_folder_path: &str) -> u32 {
        let wide = to_wide_null(dll_folder_path);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with valid null-terminated wide string.
        unsafe { (self.funcs.easy_hook_stealth_inject_runtime)(pid, wide.as_ptr()) }
    }

    pub fn easy_hook_inject_startup(
        &self,
        exe_path: &str,
        cmd_line: &str,
        flags: u32,
        dll_folder_path: &str,
    ) -> (u32, u32) {
        let wide_exe = to_wide_null(exe_path);
        let wide_cmd = to_wide_null(cmd_line);
        let wide_dll = to_wide_null(dll_folder_path);
        let mut out_pid = 0u32;
        // SAFETY: Calling into loaded ProtoInputLoader DLL with valid pointers and environment null.
        let handle = unsafe {
            (self.funcs.easy_hook_inject_startup)(
                wide_exe.as_ptr(),
                wide_cmd.as_ptr(),
                flags,
                wide_dll.as_ptr(),
                &mut out_pid,
                std::ptr::null_mut(),
            )
        };
        (handle, out_pid)
    }

    pub fn install_hook(&self, instance_handle: u32, hook: ProtoHookIDs) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.install_hook)(instance_handle, hook as u32) }
    }

    pub fn uninstall_hook(&self, instance_handle: u32, hook: ProtoHookIDs) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.uninstall_hook)(instance_handle, hook as u32) }
    }

    pub fn enable_message_filter(&self, instance_handle: u32, filter: ProtoMessageFilterIDs) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.enable_message_filter)(instance_handle, filter as u32) }
    }

    pub fn disable_message_filter(&self, instance_handle: u32, filter: ProtoMessageFilterIDs) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.disable_message_filter)(instance_handle, filter as u32) }
    }

    pub fn enable_message_block(&self, instance_handle: u32, message_id: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.enable_message_block)(instance_handle, message_id) }
    }

    pub fn disable_message_block(&self, instance_handle: u32, message_id: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.disable_message_block)(instance_handle, message_id) }
    }

    pub fn wake_up_process(&self, instance_handle: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.wake_up_process)(instance_handle) }
    }

    pub fn update_main_window_handle(&self, instance_handle: u32, hwnd: u64) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.update_main_window_handle)(instance_handle, hwnd) }
    }

    pub fn setup_state(&self, instance_handle: u32, instance_index: i32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.setup_state)(instance_handle, instance_index) }
    }

    pub fn setup_messages_to_send(
        &self,
        instance_handle: u32,
        wheel: bool,
        button: bool,
        move_msg: bool,
        keyboard: bool,
        dblclk: bool,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.setup_messages_to_send)(
                instance_handle,
                wheel,
                button,
                move_msg,
                keyboard,
                dblclk,
            )
        }
    }

    pub fn start_focus_message_loop(
        &self,
        instance_handle: u32,
        ms: i32,
        activate: bool,
        activateapp: bool,
        ncactivate: bool,
        setfocus: bool,
        mouseactivate: bool,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.start_focus_message_loop)(
                instance_handle,
                ms,
                activate,
                activateapp,
                ncactivate,
                setfocus,
                mouseactivate,
            )
        }
    }

    pub fn stop_focus_message_loop(&self, instance_handle: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.stop_focus_message_loop)(instance_handle) }
    }

    pub fn set_draw_fake_cursor(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_draw_fake_cursor)(instance_handle, enable) }
    }

    pub fn set_draw_fake_cursor_fix(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_draw_fake_cursor_fix)(instance_handle, enable) }
    }

    pub fn set_external_freeze_fake_input(&self, instance_handle: u32, enable_freeze: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_external_freeze_fake_input)(instance_handle, enable_freeze) }
    }

    pub fn add_selected_mouse_handle(&self, instance_handle: u32, mouse_handle: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.add_selected_mouse_handle)(instance_handle, mouse_handle) }
    }

    pub fn add_selected_keyboard_handle(&self, instance_handle: u32, keyboard_handle: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.add_selected_keyboard_handle)(instance_handle, keyboard_handle) }
    }

    pub fn set_controller_index(&self, instance_handle: u32, c1: u32, c2: u32, c3: u32, c4: u32) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_controller_index)(instance_handle, c1, c2, c3, c4) }
    }

    pub fn set_use_dinput_redirection(&self, instance_handle: u32, use_redirection: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_use_dinput_redirection)(instance_handle, use_redirection) }
    }

    pub fn set_use_open_xinput(&self, instance_handle: u32, use_open_xinput: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_use_open_xinput)(instance_handle, use_open_xinput) }
    }

    pub fn set_show_cursor_when_image_updated(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_show_cursor_when_image_updated)(instance_handle, enable) }
    }

    pub fn set_put_mouse_inside_window(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_put_mouse_inside_window)(instance_handle, enable) }
    }

    pub fn set_default_top_left_mouse_bounds(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_default_top_left_mouse_bounds)(instance_handle, enable) }
    }

    pub fn set_default_bottom_right_mouse_bounds(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_default_bottom_right_mouse_bounds)(instance_handle, enable) }
    }

    pub fn set_dont_wait_window_border(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_dont_wait_window_border)(instance_handle, enable) }
    }

    pub fn add_handle_to_rename(&self, instance_handle: u32, name: &str) {
        let wide = to_wide_null(name);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with null-terminated wide string.
        unsafe { (self.funcs.add_handle_to_rename)(instance_handle, wide.as_ptr()) }
    }

    pub fn add_named_pipe_to_rename(&self, instance_handle: u32, name: &str) {
        let wide = to_wide_null(name);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with null-terminated wide string.
        unsafe { (self.funcs.add_named_pipe_to_rename)(instance_handle, wide.as_ptr()) }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_dinput_device_guid(
        &self,
        instance_handle: u32,
        data1: u32,
        data2: u16,
        data3: u16,
        data4a: u8,
        data4b: u8,
        data4c: u8,
        data4d: u8,
        data4e: u8,
        data4f: u8,
        data4g: u8,
        data4h: u8,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.set_dinput_device_guid)(
                instance_handle,
                data1,
                data2,
                data3,
                data4a,
                data4b,
                data4c,
                data4d,
                data4e,
                data4f,
                data4g,
                data4h,
            )
        }
    }

    pub fn dinput_hook_also_hooks_get_device_state(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.dinput_hook_also_hooks_get_device_state)(instance_handle, enable) }
    }

    pub fn set_set_window_pos_settings(
        &self,
        instance_handle: u32,
        posx: i32,
        posy: i32,
        width: i32,
        height: i32,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.set_set_window_pos_settings)(instance_handle, posx, posy, width, height)
        }
    }

    pub fn set_set_window_pos_dont_resize(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_set_window_pos_dont_resize)(instance_handle, enable) }
    }

    pub fn set_set_window_pos_dont_reposition(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_set_window_pos_dont_reposition)(instance_handle, enable) }
    }

    pub fn set_create_single_hid_name(&self, instance_handle: u32, name: &str) {
        let wide = to_wide_null(name);
        // SAFETY: Calling into loaded ProtoInputLoader DLL with null-terminated wide string.
        unsafe { (self.funcs.set_create_single_hid_name)(instance_handle, wide.as_ptr()) }
    }

    pub fn set_cursor_clip_options(&self, instance_handle: u32, use_fake_clip_cursor: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_cursor_clip_options)(instance_handle, use_fake_clip_cursor) }
    }

    pub fn allow_fake_cursor_out_of_bounds(
        &self,
        instance_handle: u32,
        allow_out_of_bounds: bool,
        extend_bounds: bool,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.allow_fake_cursor_out_of_bounds)(
                instance_handle,
                allow_out_of_bounds,
                extend_bounds,
            )
        }
    }

    pub fn set_toggle_fake_cursor_visibility_shortcut(
        &self,
        instance_handle: u32,
        enabled: bool,
        vkey: u32,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.set_toggle_fake_cursor_visibility_shortcut)(instance_handle, enabled, vkey)
        }
    }

    pub fn set_raw_input_bypass(&self, instance_handle: u32, enabled: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_raw_input_bypass)(instance_handle, enabled) }
    }

    pub fn set_move_window_settings(
        &self,
        instance_handle: u32,
        posx: i32,
        posy: i32,
        width: i32,
        height: i32,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_move_window_settings)(instance_handle, posx, posy, width, height) }
    }

    pub fn set_move_window_dont_resize(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_move_window_dont_resize)(instance_handle, enable) }
    }

    pub fn set_move_window_dont_reposition(&self, instance_handle: u32, enable: bool) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe { (self.funcs.set_move_window_dont_reposition)(instance_handle, enable) }
    }

    pub fn set_adjust_window_rect_settings(
        &self,
        instance_handle: u32,
        posx: i32,
        posy: i32,
        width: i32,
        height: i32,
    ) {
        // SAFETY: Calling into loaded ProtoInputLoader DLL.
        unsafe {
            (self.funcs.set_adjust_window_rect_settings)(instance_handle, posx, posy, width, height)
        }
    }
}

impl Drop for ProtoInputLoader {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            // SAFETY: Module was successfully loaded via LoadLibraryW.
            unsafe {
                let _ = windows::Win32::Foundation::FreeLibrary(self.module);
            }
        }
    }
}

/// Safe RAII wrapper around `ProtoInputUtilDynamic32.dll` or `ProtoInputUtilDynamic64.dll`.
pub struct ProtoInputUtil {
    dll_path: PathBuf,
    bitness: ProcessBitness,
    funcs: ProtoInputUtilFunctions,
    #[cfg(windows)]
    module: windows::Win32::Foundation::HMODULE,
}

unsafe impl Send for ProtoInputUtil {}
unsafe impl Sync for ProtoInputUtil {}

impl ProtoInputUtil {
    pub fn load(dll_path: impl AsRef<Path>, bitness: ProcessBitness) -> NucleusResult<Self> {
        let path = dll_path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(NucleusError::Platform(format!(
                "ProtoInput util DLL does not exist: {:?}",
                path
            )));
        }

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
            use windows::core::{PCSTR, PCWSTR};

            let wide_path: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // SAFETY: Loading DLL dynamically with absolute verified path.
            let module = unsafe { LoadLibraryW(PCWSTR(wide_path.as_ptr())) }.map_err(|e| {
                NucleusError::Platform(format!(
                    "Failed to load ProtoInput Util DLL '{:?}': {}",
                    path, e
                ))
            })?;

            macro_rules! resolve_sym {
                ($sym_str:literal, $ty:ty) => {{
                    let cstr = concat!($sym_str, "\0");
                    // SAFETY: module is valid; symbol name is null-terminated ASCII.
                    let proc = unsafe { GetProcAddress(module, PCSTR(cstr.as_ptr())) };
                    match proc {
                        Some(p) => unsafe { std::mem::transmute_copy::<_, $ty>(&p) },
                        None => {
                            unsafe {
                                let _ = windows::Win32::Foundation::FreeLibrary(module);
                            }
                            return Err(NucleusError::Platform(format!(
                                "Missing symbol '{}' in ProtoInput Util DLL '{:?}'",
                                $sym_str, path
                            )));
                        }
                    }
                }};
            }

            let funcs = ProtoInputUtilFunctions {
                lock_input: resolve_sym!("LockInput", FnLockInput),
                suspend_explorer: resolve_sym!("SuspendExplorer", FnSuspendExplorer),
                restart_explorer: resolve_sym!("RestartExplorer", FnRestartExplorer),
                set_taskbar_visibility: resolve_sym!(
                    "SetTaskbarVisibility",
                    FnSetTaskbarVisibility
                ),
                get_taskbar_visibility: resolve_sym!(
                    "GetTaskbarVisibility",
                    FnGetTaskbarVisibility
                ),
            };

            Ok(Self {
                dll_path: path,
                bitness,
                funcs,
                module,
            })
        }

        #[cfg(not(windows))]
        {
            Err(NucleusError::Platform(
                "ProtoInput util is only supported on Windows".to_string(),
            ))
        }
    }

    pub fn find_and_load(base_dir: Option<&Path>, bitness: ProcessBitness) -> NucleusResult<Self> {
        let dll_filename = match bitness {
            ProcessBitness::X86 => "ProtoInputUtilDynamic32.dll",
            ProcessBitness::X64 => "ProtoInputUtilDynamic64.dll",
        };

        let candidate_paths = [
            base_dir.map(|d| d.join(dll_filename)),
            base_dir.map(|d| d.join("utils").join("ProtoInput").join(dll_filename)),
            base_dir.map(|d| d.join("ProtoInput").join(dll_filename)),
            std::env::current_exe().ok().and_then(|p| {
                p.parent()
                    .map(|d| d.join("utils").join("ProtoInput").join(dll_filename))
            }),
            Some(PathBuf::from("utils").join("ProtoInput").join(dll_filename)),
            Some(PathBuf::from(dll_filename)),
        ];

        for candidate in candidate_paths.into_iter().flatten() {
            if candidate.exists() {
                return Self::load(&candidate, bitness);
            }
        }

        Err(NucleusError::Platform(format!(
            "ProtoInput util DLL '{}' could not be found in candidate directories",
            dll_filename
        )))
    }

    pub fn bitness(&self) -> ProcessBitness {
        self.bitness
    }

    pub fn dll_path(&self) -> &Path {
        &self.dll_path
    }

    pub fn lock_input(&self, lock: bool) -> u32 {
        // SAFETY: Calling into loaded ProtoInputUtil DLL.
        unsafe { (self.funcs.lock_input)(lock) }
    }

    pub fn suspend_explorer(&self) {
        // SAFETY: Calling into loaded ProtoInputUtil DLL.
        unsafe { (self.funcs.suspend_explorer)() }
    }

    pub fn restart_explorer(&self) {
        // SAFETY: Calling into loaded ProtoInputUtil DLL.
        unsafe { (self.funcs.restart_explorer)() }
    }

    pub fn set_taskbar_visibility(&self, auto_hide: bool, always_on_top: bool) {
        // SAFETY: Calling into loaded ProtoInputUtil DLL.
        unsafe { (self.funcs.set_taskbar_visibility)(auto_hide, always_on_top) }
    }

    pub fn get_taskbar_visibility(&self) -> (bool, bool) {
        let mut auto_hide = false;
        let mut always_on_top = false;
        // SAFETY: Calling into loaded ProtoInputUtil DLL with valid out pointers.
        unsafe { (self.funcs.get_taskbar_visibility)(&mut auto_hide, &mut always_on_top) };
        (auto_hide, always_on_top)
    }

    pub fn set_taskbar_autohide(&self, auto_hide: bool) {
        self.set_taskbar_visibility(auto_hide, false);
    }

    pub fn get_taskbar_autohide(&self) -> bool {
        self.get_taskbar_visibility().0
    }
}

impl Drop for ProtoInputUtil {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            // SAFETY: Module was successfully loaded via LoadLibraryW.
            unsafe {
                let _ = windows::Win32::Foundation::FreeLibrary(self.module);
            }
        }
    }
}

// ============================================================================
// 7. Bitness Detection
// ============================================================================

/// Determines process bitness on Windows using `IsWow64Process`.
pub fn detect_process_bitness(pid: u32) -> NucleusResult<ProcessBitness> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{BOOL, CloseHandle};
        use windows::Win32::System::Threading::{
            IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        // SAFETY: OpenProcess called with limited query rights.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| {
                NucleusError::Platform(format!("Failed to open process {}: {}", pid, e))
            })?;

        let mut is_wow64 = BOOL(0);
        // SAFETY: IsWow64Process called with valid process handle and out-parameter.
        let res = unsafe { IsWow64Process(handle, &mut is_wow64) };
        unsafe {
            let _ = CloseHandle(handle);
        }

        if res.is_err() {
            return Err(NucleusError::Platform(format!(
                "IsWow64Process failed for PID {}",
                pid
            )));
        }

        if is_wow64.as_bool() {
            Ok(ProcessBitness::X86)
        } else if std::mem::size_of::<usize>() == 8 {
            Ok(ProcessBitness::X64)
        } else {
            Ok(ProcessBitness::X86)
        }
    }

    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(ProcessBitness::X64)
    }
}

// ============================================================================
// 8. High-Level ProtoInput Runtime Orchestrator
// ============================================================================

/// High-level runtime coordinating ProtoInput lifecycle, DLL caching, and instance configuration.
pub struct ProtoInputRuntime {
    dll_folder: Option<PathBuf>,
    loader_x86: RwLock<Option<Arc<ProtoInputLoader>>>,
    loader_x64: RwLock<Option<Arc<ProtoInputLoader>>>,
    util_x86: RwLock<Option<Arc<ProtoInputUtil>>>,
    util_x64: RwLock<Option<Arc<ProtoInputUtil>>>,
    tracked_instance_handles: Mutex<Vec<u32>>,
}

impl ProtoInputRuntime {
    /// Creates a new ProtoInput runtime with an optional base DLL directory.
    pub fn new(dll_folder: Option<PathBuf>) -> Self {
        Self {
            dll_folder,
            loader_x86: RwLock::new(None),
            loader_x64: RwLock::new(None),
            util_x86: RwLock::new(None),
            util_x64: RwLock::new(None),
            tracked_instance_handles: Mutex::new(Vec::new()),
        }
    }

    /// Obtains or lazily initializes the `ProtoInputLoader` for the given bitness.
    pub fn loader_for_bitness(
        &self,
        bitness: ProcessBitness,
    ) -> NucleusResult<Arc<ProtoInputLoader>> {
        let lock = match bitness {
            ProcessBitness::X86 => &self.loader_x86,
            ProcessBitness::X64 => &self.loader_x64,
        };

        if let Some(loader) = lock
            .read()
            .map_err(|e| NucleusError::Platform(e.to_string()))?
            .as_ref()
        {
            return Ok(Arc::clone(loader));
        }

        let mut write_guard = lock
            .write()
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        if let Some(loader) = write_guard.as_ref() {
            return Ok(Arc::clone(loader));
        }

        let loaded = ProtoInputLoader::find_and_load(self.dll_folder.as_deref(), bitness)?;
        let arc = Arc::new(loaded);
        *write_guard = Some(Arc::clone(&arc));
        Ok(arc)
    }

    /// Obtains or lazily initializes the `ProtoInputUtil` for the given bitness.
    pub fn util_for_bitness(&self, bitness: ProcessBitness) -> NucleusResult<Arc<ProtoInputUtil>> {
        let lock = match bitness {
            ProcessBitness::X86 => &self.util_x86,
            ProcessBitness::X64 => &self.util_x64,
        };

        if let Some(util) = lock
            .read()
            .map_err(|e| NucleusError::Platform(e.to_string()))?
            .as_ref()
        {
            return Ok(Arc::clone(util));
        }

        let mut write_guard = lock
            .write()
            .map_err(|e| NucleusError::Platform(e.to_string()))?;
        if let Some(util) = write_guard.as_ref() {
            return Ok(Arc::clone(util));
        }

        let loaded = ProtoInputUtil::find_and_load(self.dll_folder.as_deref(), bitness)?;
        let arc = Arc::new(loaded);
        *write_guard = Some(Arc::clone(&arc));
        Ok(arc)
    }

    /// Injects ProtoInput at process startup, returning `(instance_handle, pid)`.
    pub fn inject_startup(
        &self,
        exe_path: &Path,
        cmd_line: &str,
        flags: u32,
        dll_folder_path: &Path,
    ) -> NucleusResult<(u32, u32)> {
        let exe_str = exe_path.to_str().ok_or_else(|| {
            NucleusError::Platform("Executable path contains invalid UTF-8".to_string())
        })?;
        let dll_str = dll_folder_path.to_str().ok_or_else(|| {
            NucleusError::Platform("DLL folder path contains invalid UTF-8".to_string())
        })?;

        // Format path with trailing backslash matching C# ProtoInput.cs
        let normalized_dll_str = if dll_str.ends_with('\\') {
            dll_str.to_string()
        } else {
            format!("{}\\", dll_str)
        };

        // Try 64-bit loader first, fallback to 32-bit if host is 64-bit.
        let loader = self
            .loader_for_bitness(ProcessBitness::X64)
            .or_else(|_| self.loader_for_bitness(ProcessBitness::X86))?;

        let (instance_handle, pid) =
            loader.easy_hook_inject_startup(exe_str, cmd_line, flags, &normalized_dll_str);

        if instance_handle == 0 {
            return Err(NucleusError::Platform(
                "ProtoInput startup injection failed (instance handle returned 0)".to_string(),
            ));
        }

        Ok((instance_handle, pid))
    }

    /// Injects ProtoInput into a running target process.
    pub fn inject_runtime(
        &self,
        pid: u32,
        method: InjectMethod,
        dll_folder_path: &Path,
    ) -> NucleusResult<u32> {
        let dll_str = dll_folder_path.to_str().ok_or_else(|| {
            NucleusError::Platform("DLL folder path contains invalid UTF-8".to_string())
        })?;

        let normalized_dll_str = if dll_str.ends_with('\\') {
            dll_str.to_string()
        } else {
            format!("{}\\", dll_str)
        };

        // The loader DLL must match the launcher's own bitness (it is loaded in-process);
        // it selects the matching hook DLL for the target process itself.
        let host_bitness = if cfg!(target_pointer_width = "64") {
            ProcessBitness::X64
        } else {
            ProcessBitness::X86
        };
        let loader = self.loader_for_bitness(host_bitness)?;

        let instance_handle = match method {
            InjectMethod::RemoteLoad => {
                loader.remote_load_library_inject_runtime(pid, &normalized_dll_str)
            }
            InjectMethod::EasyHook => loader.easy_hook_inject_runtime(pid, &normalized_dll_str),
            InjectMethod::EasyHookStealth => {
                loader.easy_hook_stealth_inject_runtime(pid, &normalized_dll_str)
            }
        };

        if instance_handle == 0 {
            return Err(NucleusError::Platform(
                "ProtoInput runtime injection failed (instance handle returned 0)".to_string(),
            ));
        }

        Ok(instance_handle)
    }

    /// Configures all ProtoInput hooks for a specific instance, reproducing
    /// C# `ProtoInputLauncher.SetupInstance` with 100% fidelity.
    pub fn setup_instance(
        &self,
        instance_handle: u32,
        instance_index: usize,
        config: &ProtoInputConfig,
        player: &PlayerProtoInputConfig,
    ) -> NucleusResult<()> {
        let loader = self
            .loader_for_bitness(ProcessBitness::X64)
            .or_else(|_| self.loader_for_bitness(ProcessBitness::X86))?;

        loader.setup_state(instance_handle, instance_index as i32);

        if config.register_raw_input_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::RegisterRawInput);
        }
        if config.get_raw_input_data_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::GetRawInputData);
        }
        if config.message_filter_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::MessageFilter);
        }
        if config.get_cursor_pos_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::GetCursorPos);
        }
        if config.set_cursor_pos_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::SetCursorPos);
        }
        if config.get_key_state_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::GetKeyState);
        }
        if config.get_async_key_state_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::GetAsyncKeyState);
        }
        if config.get_keyboard_state_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::GetKeyboardState);
        }

        loader.set_show_cursor_when_image_updated(
            instance_handle,
            !config.dont_show_cursor_when_image_updated,
        );
        if config.cursor_visibility_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::CursorVisibilityState);
        }

        loader.set_cursor_clip_options(instance_handle, config.clip_cursor_hook_creates_fake_clip);
        if config.clip_cursor_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::ClipCursor);
        }

        if config.focus_hooks {
            loader.install_hook(instance_handle, ProtoHookIDs::FocusHooks);
        }
        if config.rename_handles_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::RenameHandles);
        }
        if config.block_raw_input_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::BlockRawInput);
        }
        if config.find_window_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::FindWindow);
        }

        let hid_name = player.raw_hid.as_deref().unwrap_or("");
        loader.set_create_single_hid_name(instance_handle, hid_name);
        if config.create_single_hid_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::CreateSingleHID);
        }

        if config.set_window_style_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::WindowStyle);
        }

        loader.set_use_open_xinput(instance_handle, config.use_open_xinput);
        loader.set_use_dinput_redirection(instance_handle, config.use_dinput_redirection);
        if config.xinput_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::Xinput);
        }

        if player.is_dinput {
            if let Some(guid_str) = &player.gamepad_guid {
                if let Ok((d1, d2, d3, d4a, d4b, d4c, d4d, d4e, d4f, d4g, d4h)) =
                    guid_to_dinput_fields(guid_str)
                {
                    loader.set_dinput_device_guid(
                        instance_handle,
                        d1,
                        d2,
                        d3,
                        d4a,
                        d4b,
                        d4c,
                        d4d,
                        d4e,
                        d4f,
                        d4g,
                        d4h,
                    );
                }
            }
        }

        if config.dinput_hook_also_hooks_get_device_state {
            loader.dinput_hook_also_hooks_get_device_state(instance_handle, true);
        }
        if config.dinput_device_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::DinputOrder);
        }

        loader.set_set_window_pos_dont_resize(
            instance_handle,
            config.set_window_pos_hook == SetWindowPosHook::DontResize,
        );
        loader.set_set_window_pos_dont_reposition(
            instance_handle,
            config.set_window_pos_hook == SetWindowPosHook::DontReposition,
        );
        loader.set_set_window_pos_settings(
            instance_handle,
            player.monitor_bounds_x,
            player.monitor_bounds_y,
            player.monitor_bounds_width,
            player.monitor_bounds_height,
        );

        if config.set_window_pos_hook != SetWindowPosHook::None {
            loader.install_hook(instance_handle, ProtoHookIDs::SetWindowPos);
        }

        if config.raw_input_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::RawInput);
        }
        if config.mouse_move_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::MouseMove);
        }
        if config.mouse_activate_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::MouseActivate);
        }
        if config.window_activate_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::WindowActivate);
        }
        if config.window_activate_app_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::WindowActivateApp);
        }
        if config.mouse_wheel_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::MouseWheel);
        }
        if config.mouse_button_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::MouseButton);
        }
        if config.keyboard_button_filter {
            loader.enable_message_filter(instance_handle, ProtoMessageFilterIDs::KeyboardButton);
        }

        loader.set_move_window_dont_resize(
            instance_handle,
            config.move_window_hook == MoveWindowHook::DontResize,
        );
        loader.set_move_window_dont_reposition(
            instance_handle,
            config.move_window_hook == MoveWindowHook::DontReposition,
        );
        loader.set_move_window_settings(
            instance_handle,
            player.monitor_bounds_x,
            player.monitor_bounds_y,
            player.monitor_bounds_width,
            player.monitor_bounds_height,
        );

        if config.move_window_hook != MoveWindowHook::None {
            loader.install_hook(instance_handle, ProtoHookIDs::MoveWindow);
        }

        loader.set_adjust_window_rect_settings(
            instance_handle,
            player.monitor_bounds_x,
            player.monitor_bounds_y,
            player.monitor_bounds_width,
            player.monitor_bounds_height,
        );
        if config.adjust_window_rect_hook {
            loader.install_hook(instance_handle, ProtoHookIDs::AdjustWindowRect);
        }

        loader.set_dont_wait_window_border(
            instance_handle,
            config.set_remove_border_hook == SetRemoveBorderHook::DontWait,
        );
        if config.set_remove_border_hook != SetRemoveBorderHook::None {
            loader.install_hook(instance_handle, ProtoHookIDs::RemoveBorder);
        }

        for &blocked in &config.blocked_messages {
            loader.enable_message_block(instance_handle, blocked);
        }

        loader.setup_messages_to_send(
            instance_handle,
            config.send_mouse_wheel_messages,
            config.send_mouse_button_messages,
            config.send_mouse_movement_messages,
            config.send_keyboard_button_messages,
            config.send_mouse_dbl_clk_messages,
        );

        if config.enable_focus_message_loop {
            loader.start_focus_message_loop(
                instance_handle,
                config.focus_loop_interval_milliseconds,
                config.focus_loop_wm_activate,
                config.focus_loop_wm_activateapp,
                config.focus_loop_wm_ncactivate,
                config.focus_loop_wm_setfocus,
                config.focus_loop_wm_mouseactivate,
            );
        }

        loader.set_draw_fake_cursor_fix(
            instance_handle,
            config.draw_fake_cursor == DrawFakeCursor::Fix,
        );
        loader.set_draw_fake_cursor(
            instance_handle,
            config.draw_fake_cursor != DrawFakeCursor::None,
        );

        loader.set_default_top_left_mouse_bounds(
            instance_handle,
            config.put_mouse_inside_window == PutMouseInsideWindow::IgnoreTopLeft,
        );
        loader.set_default_bottom_right_mouse_bounds(
            instance_handle,
            config.put_mouse_inside_window == PutMouseInsideWindow::IgnoreBottomRight,
        );
        loader.set_put_mouse_inside_window(
            instance_handle,
            config.put_mouse_inside_window != PutMouseInsideWindow::None,
        );

        loader.allow_fake_cursor_out_of_bounds(
            instance_handle,
            config.allow_fake_cursor_out_of_bounds,
            config.extend_fake_cursor_bounds,
        );
        loader.set_toggle_fake_cursor_visibility_shortcut(
            instance_handle,
            config.enable_toggle_fake_cursor_visibility_shortcut,
            config.toggle_fake_cursor_visibility_shortcut_vkey,
        );

        for handle in &config.rename_handles {
            loader.add_handle_to_rename(instance_handle, handle);
        }
        for pipe in &config.rename_named_pipes {
            loader.add_named_pipe_to_rename(instance_handle, pipe);
        }

        if player.mouse_handle != -1 {
            loader.add_selected_mouse_handle(instance_handle, player.mouse_handle as u32);
        }
        if player.keyboard_handle != -1 {
            loader.add_selected_keyboard_handle(instance_handle, player.keyboard_handle as u32);
        }

        loader.set_controller_index(
            instance_handle,
            player.controller_index,
            player.controller_index2,
            player.controller_index3,
            player.controller_index4,
        );

        if let Ok(mut handles) = self.tracked_instance_handles.lock() {
            handles.push(instance_handle);
        }

        if config.freeze_external_input_when_input_not_locked {
            self.set_freeze_external_input_all(false)?;
        }

        Ok(())
    }

    /// Propagates input freeze state to all tracked instances matching `NotifyInputLockChange`.
    pub fn set_freeze_external_input_all(&self, is_locked: bool) -> NucleusResult<()> {
        let freeze_external = !is_locked;
        let loader = self
            .loader_for_bitness(ProcessBitness::X64)
            .or_else(|_| self.loader_for_bitness(ProcessBitness::X86))?;

        if let Ok(handles) = self.tracked_instance_handles.lock() {
            for &handle in handles.iter() {
                loader.set_external_freeze_fake_input(handle, freeze_external);
            }
        }

        Ok(())
    }

    /// Wakes up a suspended process instance after startup injection.
    pub fn wake_up_process(&self, instance_handle: u32) -> NucleusResult<()> {
        let loader = self
            .loader_for_bitness(ProcessBitness::X64)
            .or_else(|_| self.loader_for_bitness(ProcessBitness::X86))?;
        loader.wake_up_process(instance_handle);
        Ok(())
    }

    /// Returns a list of currently tracked ProtoInput instance handles.
    pub fn tracked_handles(&self) -> Vec<u32> {
        self.tracked_instance_handles
            .lock()
            .map(|h| h.clone())
            .unwrap_or_default()
    }

    /// Clears tracked handles (e.g. on session teardown).
    pub fn clear_tracked_handles(&self) {
        if let Ok(mut h) = self.tracked_instance_handles.lock() {
            h.clear();
        }
    }
}

// ============================================================================
// 9. Backward-Compatibility Static Facade
// ============================================================================

/// Integration layer with the ProtoInput runtime hook for multi-device input redirection.
pub struct ProtoInput;

impl ProtoInput {
    /// Initializes ProtoInput hooks in the target process.
    pub fn initialize(pid: u32) -> NucleusResult<()> {
        let bitness = detect_process_bitness(pid).unwrap_or(ProcessBitness::X64);
        // Attempt to locate and verify the loader DLL without panicking
        let _ = ProtoInputLoader::find_and_load(None, bitness)?;
        Ok(())
    }

    /// Configures input device routing and player assignment for the process.
    pub fn setup_input(pid: u32, player_index: usize) -> NucleusResult<()> {
        let bitness = detect_process_bitness(pid).unwrap_or(ProcessBitness::X64);
        let loader = ProtoInputLoader::find_and_load(None, bitness)?;
        loader.setup_state(pid, player_index as i32);
        Ok(())
    }

    /// Dispatches a simulated Windows input message directly to the hooked process.
    pub fn send_input_message(
        pid: u32,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> NucleusResult<()> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

            // In Windows, when targeting by HWND or broadcast:
            let hwnd = HWND(pid as usize as _);
            let res = unsafe { PostMessageW(hwnd, message, WPARAM(wparam), LPARAM(lparam)) };
            if res.is_ok() {
                Ok(())
            } else {
                Err(NucleusError::Platform(format!(
                    "Failed to post input message {:#x} to target HWND {}",
                    message, pid
                )))
            }
        }

        #[cfg(not(windows))]
        {
            let _ = (pid, message, wparam, lparam);
            Err(NucleusError::Platform(
                "Windows input message posting is only supported on Windows".to_string(),
            ))
        }
    }
}

// Helper: converts a string into a null-terminated UTF-16 wide string vector
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ============================================================================
// 10. Comprehensive Parity and Regression Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_hook_ids_values() {
        assert_eq!(ProtoHookIDs::RegisterRawInput as u32, 0);
        assert_eq!(ProtoHookIDs::GetRawInputData as u32, 1);
        assert_eq!(ProtoHookIDs::MessageFilter as u32, 2);
        assert_eq!(ProtoHookIDs::GetCursorPos as u32, 3);
        assert_eq!(ProtoHookIDs::SetCursorPos as u32, 4);
        assert_eq!(ProtoHookIDs::GetKeyState as u32, 5);
        assert_eq!(ProtoHookIDs::GetAsyncKeyState as u32, 6);
        assert_eq!(ProtoHookIDs::GetKeyboardState as u32, 7);
        assert_eq!(ProtoHookIDs::CursorVisibilityState as u32, 8);
        assert_eq!(ProtoHookIDs::ClipCursor as u32, 9);
        assert_eq!(ProtoHookIDs::FocusHooks as u32, 10);
        assert_eq!(ProtoHookIDs::RenameHandles as u32, 11);
        assert_eq!(ProtoHookIDs::Xinput as u32, 12);
        assert_eq!(ProtoHookIDs::DinputOrder as u32, 13);
        assert_eq!(ProtoHookIDs::SetWindowPos as u32, 14);
        assert_eq!(ProtoHookIDs::BlockRawInput as u32, 15);
        assert_eq!(ProtoHookIDs::FindWindow as u32, 16);
        assert_eq!(ProtoHookIDs::CreateSingleHID as u32, 17);
        assert_eq!(ProtoHookIDs::WindowStyle as u32, 18);
        assert_eq!(ProtoHookIDs::MoveWindow as u32, 19);
        assert_eq!(ProtoHookIDs::AdjustWindowRect as u32, 20);
        assert_eq!(ProtoHookIDs::RemoveBorder as u32, 21);
    }

    #[test]
    fn test_proto_message_filter_ids_values() {
        assert_eq!(ProtoMessageFilterIDs::RawInput as u32, 0);
        assert_eq!(ProtoMessageFilterIDs::MouseMove as u32, 1);
        assert_eq!(ProtoMessageFilterIDs::MouseActivate as u32, 2);
        assert_eq!(ProtoMessageFilterIDs::WindowActivate as u32, 3);
        assert_eq!(ProtoMessageFilterIDs::WindowActivateApp as u32, 4);
        assert_eq!(ProtoMessageFilterIDs::MouseWheel as u32, 5);
        assert_eq!(ProtoMessageFilterIDs::MouseButton as u32, 6);
        assert_eq!(ProtoMessageFilterIDs::KeyboardButton as u32, 7);
    }

    #[test]
    fn test_enum_options_values() {
        assert_eq!(SetWindowPosHook::None as u32, 0);
        assert_eq!(SetWindowPosHook::True as u32, 1);
        assert_eq!(SetWindowPosHook::DontResize as u32, 2);
        assert_eq!(SetWindowPosHook::DontReposition as u32, 3);

        assert_eq!(MoveWindowHook::None as u32, 0);
        assert_eq!(MoveWindowHook::True as u32, 1);
        assert_eq!(MoveWindowHook::DontResize as u32, 2);
        assert_eq!(MoveWindowHook::DontReposition as u32, 3);

        assert_eq!(DrawFakeCursor::None as u32, 0);
        assert_eq!(DrawFakeCursor::True as u32, 1);
        assert_eq!(DrawFakeCursor::Fix as u32, 2);

        assert_eq!(PutMouseInsideWindow::None as u32, 0);
        assert_eq!(PutMouseInsideWindow::True as u32, 1);
        assert_eq!(PutMouseInsideWindow::IgnoreTopLeft as u32, 2);
        assert_eq!(PutMouseInsideWindow::IgnoreBottomRight as u32, 3);

        assert_eq!(SetRemoveBorderHook::None as u32, 0);
        assert_eq!(SetRemoveBorderHook::True as u32, 1);
        assert_eq!(SetRemoveBorderHook::DontWait as u32, 2);
    }

    #[test]
    fn test_guid_conversion_from_bytes_and_string() {
        // Test with a known GUID: {00112233-4455-6677-8899-aabbccddeeff}
        let guid_str = "00112233-4455-6677-8899-aabbccddeeff";
        let (d1, d2, d3, d4a, d4b, d4c, d4d, d4e, d4f, d4g, d4h) =
            guid_to_dinput_fields(guid_str).expect("Valid GUID parse");

        assert_eq!(d1, 0x00112233);
        assert_eq!(d2, 0x4455);
        assert_eq!(d3, 0x6677);
        assert_eq!(d4a, 0x88);
        assert_eq!(d4b, 0x99);
        assert_eq!(d4c, 0xaa);
        assert_eq!(d4d, 0xbb);
        assert_eq!(d4e, 0xcc);
        assert_eq!(d4f, 0xdd);
        assert_eq!(d4g, 0xee);
        assert_eq!(d4h, 0xff);

        // Test with braces: {00112233-4455-6677-8899-aabbccddeeff}
        let with_braces = format!("{{{}}}", guid_str);
        let fields_braces = guid_to_dinput_fields(&with_braces).expect("Valid braced GUID");
        assert_eq!(fields_braces.0, d1);
        assert_eq!(fields_braces.1, d2);
        assert_eq!(fields_braces.2, d3);

        // Test with byte array layout: in memory, Data1, Data2, Data3 are little-endian
        let bytes: [u8; 16] = [
            0x33, 0x22, 0x11, 0x00, // Data1 = 0x00112233
            0x55, 0x44, // Data2 = 0x4455
            0x77, 0x66, // Data3 = 0x6677
            0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // Data4
        ];
        let byte_fields = guid_bytes_to_dinput_fields(&bytes);
        assert_eq!(byte_fields.0, d1);
        assert_eq!(byte_fields.1, d2);
        assert_eq!(byte_fields.2, d3);
        assert_eq!(byte_fields.3, d4a);
        assert_eq!(byte_fields.10, d4h);
    }

    #[test]
    fn test_proto_input_config_builder_and_defaults() {
        let config = ProtoInputConfig::default();
        assert_eq!(config.focus_loop_interval_milliseconds, 5);
        assert_eq!(config.toggle_fake_cursor_visibility_shortcut_vkey, 0x24);
        assert_eq!(config.draw_fake_cursor, DrawFakeCursor::None);
        assert!(!config.register_raw_input_hook);

        let customized = ProtoInputConfig::default()
            .with_raw_input()
            .with_fake_cursor(DrawFakeCursor::Fix)
            .with_focus_message_loop(10);

        assert!(customized.register_raw_input_hook);
        assert!(customized.get_raw_input_data_hook);
        assert!(customized.message_filter_hook);
        assert_eq!(customized.draw_fake_cursor, DrawFakeCursor::Fix);
        assert_eq!(customized.focus_loop_interval_milliseconds, 10);
        assert!(customized.focus_loop_wm_activate);
    }

    #[test]
    fn test_proto_input_config_json_serialization() {
        let config = ProtoInputConfig::default().with_raw_input();
        let json = serde_json::to_string(&config).expect("Serialize to JSON");
        let deserialized: ProtoInputConfig =
            serde_json::from_str(&json).expect("Deserialize from JSON");

        assert_eq!(config, deserialized);
        assert!(deserialized.register_raw_input_hook);
    }

    #[test]
    fn test_player_proto_input_config_builder() {
        let player = PlayerProtoInputConfig::new(42)
            .with_bounds(100, 200, 1280, 720)
            .with_raw_handles(10, 20)
            .with_controllers(1, 2, 3, 4);

        assert_eq!(player.instance_handle, 42);
        assert_eq!(player.monitor_bounds_x, 100);
        assert_eq!(player.monitor_bounds_y, 200);
        assert_eq!(player.monitor_bounds_width, 1280);
        assert_eq!(player.monitor_bounds_height, 720);
        assert_eq!(player.keyboard_handle, 10);
        assert_eq!(player.mouse_handle, 20);
        assert_eq!(player.controller_index, 1);
        assert_eq!(player.controller_index2, 2);
    }

    #[test]
    fn test_dynamic_loader_missing_dll_graceful_error() {
        let result_x86 = ProtoInputLoader::load(
            "C:\\Definitely\\NonExistent\\ProtoInputLoader32.dll",
            ProcessBitness::X86,
        );
        assert!(result_x86.is_err());
        match result_x86 {
            Err(NucleusError::Platform(msg)) => {
                assert!(msg.contains("does not exist"));
            }
            _ => panic!("Expected NucleusError::Platform for non-existent DLL"),
        }

        let util_result = ProtoInputUtil::load(
            "C:\\Definitely\\NonExistent\\ProtoInputUtilDynamic64.dll",
            ProcessBitness::X64,
        );
        assert!(util_result.is_err());
        match util_result {
            Err(NucleusError::Platform(msg)) => {
                assert!(msg.contains("does not exist"));
            }
            _ => panic!("Expected NucleusError::Platform for non-existent DLL"),
        }
    }

    #[test]
    fn test_proto_input_runtime_tracked_handles() {
        let runtime = ProtoInputRuntime::new(Some(PathBuf::from("fake_utils/ProtoInput")));
        assert!(runtime.tracked_handles().is_empty());

        if let Ok(mut handles) = runtime.tracked_instance_handles.lock() {
            handles.push(1001);
            handles.push(1002);
        }

        assert_eq!(runtime.tracked_handles(), vec![1001, 1002]);
        runtime.clear_tracked_handles();
        assert!(runtime.tracked_handles().is_empty());
    }
}
