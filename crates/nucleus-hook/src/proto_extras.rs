//! Optional ProtoInput features added by newer builds of the library: controller-to-mouse
//! and mouse-to-controller translation, input re-registration, sub-window messaging and
//! manual cursor scaling.
//!
//! Older ProtoInput releases do not export these functions. They are therefore resolved
//! individually and treated as optional, so the core loader keeps working with any release
//! and a handler asking for a missing feature gets a warning instead of a failed launch.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Hook ID of `GetCursorInfoHook` (newer builds only).
pub const HOOK_ID_GET_CURSOR_INFO: u32 = 22;
/// Hook ID of `SetWindowsHookHook` (newer builds only).
pub const HOOK_ID_SET_WINDOWS_HOOK: u32 = 23;

// Windows virtual-key codes used as translation defaults.
const VK_RETURN: i32 = 13;
const VK_CONTROL: i32 = 17;
const VK_ESCAPE: i32 = 27;
const VK_SPACE: i32 = 32;
const VK_LEFT: i32 = 37;
const VK_UP: i32 = 38;
const VK_RIGHT: i32 = 39;
const VK_DOWN: i32 = 40;

/// `Game.ProtoInput` options handled through the optional exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProtoInputExtraConfig {
    /// Drive the game's mouse and keyboard from a controller.
    pub translate_xinput_to_mkb: bool,
    /// Present a keyboard and mouse to the game as a controller.
    pub translate_mkb_to_xinput: bool,
    pub re_register_input: bool,
    pub pointer_in_mouse: bool,
    pub message_sub_windows: bool,
    pub get_cursor_info_hook: bool,
    pub set_windows_hook_hook: bool,
    /// Resolution the game believes it renders at (0 = no manual scaling).
    pub scale_from_x: i32,
    pub scale_from_y: i32,

    // Controller-to-keyboard bindings (virtual-key codes; 0 = use the default).
    pub button_a: i32,
    pub button_b: i32,
    pub button_x: i32,
    pub button_y: i32,
    pub button_rs: i32,
    pub button_ls: i32,
    pub button_right: i32,
    pub button_left: i32,
    pub button_up: i32,
    pub button_down: i32,
    pub stick_r: i32,
    pub stick_l: i32,
    pub stick_right: i32,
    pub stick_left: i32,
    pub stick_up: i32,
    pub stick_down: i32,
    pub button_option: i32,
    pub button_start: i32,
    pub sensitivity: i32,
    pub acceleration: i32,
    pub deadzone: i32,

    // Translation behaviour switches.
    pub swap_sticks: bool,
    pub scanner: bool,
    pub shoulders_next_point: bool,
    pub a_static: bool,
    pub a_point_click: bool,
    pub a_point_move: bool,
    pub b_static: bool,
    pub b_point_click: bool,
    pub b_point_move: bool,
    pub x_static: bool,
    pub x_point_click: bool,
    pub x_point_move: bool,
    pub y_static: bool,
    pub y_point_click: bool,
    pub y_point_move: bool,
}

impl ProtoInputExtraConfig {
    /// Whether the handler uses anything that needs the optional exports.
    pub fn is_used(&self) -> bool {
        self != &Self::default()
    }

    /// The 21 key/tuning values in the order `SetXinputtoMKBkeys` expects, with the
    /// launcher's defaults filled in for bindings the handler left at 0.
    pub fn key_values(&self) -> [i32; 21] {
        let or = |v: i32, default: i32| if v == 0 { default } else { v };
        [
            or(self.button_a, b'R' as i32),
            or(self.button_b, b'C' as i32),
            or(self.button_x, b'E' as i32),
            or(self.button_y, b'G' as i32),
            or(self.button_rs, VK_SPACE),
            or(self.button_ls, VK_CONTROL),
            or(self.button_right, VK_RIGHT),
            or(self.button_left, VK_LEFT),
            or(self.button_up, VK_UP),
            or(self.button_down, VK_DOWN),
            self.stick_r,
            self.stick_l,
            or(self.stick_right, b'D' as i32),
            or(self.stick_left, b'A' as i32),
            or(self.stick_up, b'W' as i32),
            or(self.stick_down, b'S' as i32),
            or(self.button_option, VK_ESCAPE),
            or(self.button_start, VK_RETURN),
            or(self.sensitivity, 12),
            or(self.acceleration, 4),
            self.deadzone,
        ]
    }

    /// The 15 switches in the order `SetXinputtoMKBCFG` expects.
    pub fn cfg_values(&self) -> [bool; 15] {
        [
            self.swap_sticks,
            self.scanner,
            self.shoulders_next_point,
            self.a_static,
            self.a_point_click,
            self.a_point_move,
            self.b_static,
            self.b_point_click,
            self.b_point_move,
            self.x_static,
            self.x_point_click,
            self.x_point_move,
            self.y_static,
            self.y_point_click,
            self.y_point_move,
        ]
    }
}

type FnHandleBool = unsafe extern "C" fn(u32, bool);
type FnManualScaling = unsafe extern "C" fn(u32, i32, i32, i32, i32);
type FnInstallHookRaw = unsafe extern "C" fn(u32, u32);
#[rustfmt::skip]
type FnMkbKeys = unsafe extern "C" fn(
    u32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32,
    i32, i32, i32, i32,
);
#[rustfmt::skip]
type FnMkbCfg = unsafe extern "C" fn(
    u32, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool,
);

/// The optional exports found in the loaded ProtoInput loader DLL.
#[derive(Clone, Copy, Default)]
pub struct ProtoInputExtras {
    set_translate_xinput_to_mkb: Option<FnHandleBool>,
    set_translate_mkb_to_xinput: Option<FnHandleBool>,
    set_reregister_input: Option<FnHandleBool>,
    set_pointer_in_mouse: Option<FnHandleBool>,
    set_send_messages_to_sub_windows: Option<FnHandleBool>,
    set_manual_scaling: Option<FnManualScaling>,
    set_xinput_to_mkb_keys: Option<FnMkbKeys>,
    set_xinput_to_mkb_cfg: Option<FnMkbCfg>,
    install_hook: Option<FnInstallHookRaw>,
}

impl ProtoInputExtras {
    /// Resolves the optional exports from the already loaded loader DLL at `dll_path`.
    #[cfg(windows)]
    pub fn resolve(dll_path: &Path) -> Self {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
        use windows::core::{PCSTR, PCWSTR};

        let wide: Vec<u16> = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` is a valid null-terminated path. The module stays loaded for as long
        // as the `ProtoInputLoader` that loaded it is alive.
        let Ok(module) = (unsafe { GetModuleHandleW(PCWSTR(wide.as_ptr())) }) else {
            return Self::default();
        };
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                // SAFETY: `module` is valid and the name is null-terminated ASCII. The
                // transmute gives the export the C signature documented by ProtoInput.
                unsafe { GetProcAddress(module, PCSTR(concat!($name, "\0").as_ptr())) }
                    .map(|p| unsafe { std::mem::transmute_copy::<_, $ty>(&p) })
            }};
        }
        Self {
            set_translate_xinput_to_mkb: sym!("SetTranslateXinputtoMKB", FnHandleBool),
            set_translate_mkb_to_xinput: sym!("SetTranslateMKBtoXinput", FnHandleBool),
            set_reregister_input: sym!("SetReregisterinput", FnHandleBool),
            set_pointer_in_mouse: sym!("SetPointerInMouse", FnHandleBool),
            set_send_messages_to_sub_windows: sym!("SetSendMessagesToSubWindows", FnHandleBool),
            set_manual_scaling: sym!("SetManualScaling", FnManualScaling),
            set_xinput_to_mkb_keys: sym!("SetXinputtoMKBkeys", FnMkbKeys),
            set_xinput_to_mkb_cfg: sym!("SetXinputtoMKBCFG", FnMkbCfg),
            install_hook: sym!("InstallHook", FnInstallHookRaw),
        }
    }

    /// Resolves the optional exports from the already loaded loader DLL at `dll_path`.
    #[cfg(not(windows))]
    pub fn resolve(_dll_path: &Path) -> Self {
        Self::default()
    }

    /// Whether this ProtoInput build supports controller-to-mouse translation.
    pub fn supports_translation(&self) -> bool {
        self.set_translate_xinput_to_mkb.is_some()
            && self.set_xinput_to_mkb_keys.is_some()
            && self.set_xinput_to_mkb_cfg.is_some()
    }

    /// Applies `cfg` to an instance. `bounds` is the instance's width and height.
    ///
    /// Returns the names of requested features this ProtoInput build does not provide.
    pub fn apply(
        &self,
        instance_handle: u32,
        cfg: &ProtoInputExtraConfig,
        bounds: (i32, i32),
    ) -> Vec<&'static str> {
        let mut missing = Vec::new();
        let mut flag = |f: Option<FnHandleBool>, wanted: bool, always: bool, name: &'static str| {
            match f {
                // SAFETY: the export was resolved from the loaded ProtoInput DLL and takes
                // (instance handle, bool).
                Some(func) if wanted || always => unsafe { func(instance_handle, wanted) },
                Some(_) => {}
                None if wanted => missing.push(name),
                None => {}
            }
        };
        // The launcher always states both translation modes, like the original.
        flag(
            self.set_translate_xinput_to_mkb,
            cfg.translate_xinput_to_mkb,
            true,
            "TranslateXinputtoMKB",
        );
        flag(
            self.set_translate_mkb_to_xinput,
            cfg.translate_mkb_to_xinput,
            true,
            "TranslateMKBtoXinput",
        );
        flag(
            self.set_reregister_input,
            cfg.re_register_input,
            false,
            "ReRegisterInput",
        );
        flag(
            self.set_pointer_in_mouse,
            cfg.pointer_in_mouse,
            false,
            "PointerInMouse",
        );
        flag(
            self.set_send_messages_to_sub_windows,
            cfg.message_sub_windows,
            false,
            "MessageSubWindows",
        );

        for (wanted, id, name) in [
            (cfg.get_cursor_info_hook, HOOK_ID_GET_CURSOR_INFO, "GetCursorInfoHook"),
            (cfg.set_windows_hook_hook, HOOK_ID_SET_WINDOWS_HOOK, "SetWindowsHookHook"),
        ] {
            if !wanted {
                continue;
            }
            // These hook IDs only exist in builds that also export the translation API.
            match (self.install_hook, self.set_translate_xinput_to_mkb) {
                // SAFETY: resolved export taking (instance handle, hook id).
                (Some(install), Some(_)) => unsafe { install(instance_handle, id) },
                _ => missing.push(name),
            }
        }

        if cfg.scale_from_x != 0 || cfg.scale_from_y != 0 {
            match self.set_manual_scaling {
                // SAFETY: resolved export taking (handle, oldX, oldY, newX, newY).
                Some(func) => unsafe {
                    func(
                        instance_handle,
                        cfg.scale_from_x,
                        cfg.scale_from_y,
                        bounds.0,
                        bounds.1,
                    )
                },
                None => missing.push("ScaleFromX/ScaleFromY"),
            }
        }

        if cfg.translate_xinput_to_mkb {
            let k = cfg.key_values();
            let c = cfg.cfg_values();
            if let (Some(keys), Some(switches)) =
                (self.set_xinput_to_mkb_keys, self.set_xinput_to_mkb_cfg)
            {
                // SAFETY: resolved exports with the documented 21-int / 15-bool signatures.
                unsafe {
                    keys(
                        instance_handle,
                        k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7], k[8], k[9], k[10],
                        k[11], k[12], k[13], k[14], k[15], k[16], k[17], k[18], k[19], k[20],
                    );
                    switches(
                        instance_handle,
                        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8], c[9], c[10],
                        c[11], c[12], c[13], c[14],
                    );
                }
            }
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn defaults_fill_unset_bindings_only() {
        let mut cfg = ProtoInputExtraConfig::default();
        assert!(!cfg.is_used());
        cfg.translate_xinput_to_mkb = true;
        cfg.button_a = 70;
        cfg.deadzone = 9000;
        assert!(cfg.is_used());
        let k = cfg.key_values();
        assert_eq!(k[0], 70, "handler binding is kept");
        assert_eq!(k[1], b'C' as i32);
        assert_eq!(k[4], VK_SPACE);
        assert_eq!(&k[12..16], &[b'D' as i32, b'A' as i32, b'W' as i32, b'S' as i32]);
        assert_eq!((k[16], k[17]), (VK_ESCAPE, VK_RETURN));
        assert_eq!((k[18], k[19], k[20]), (12, 4, 9000));
        assert_eq!((k[10], k[11]), (0, 0), "stick clicks have no default binding");
    }

    #[test]
    fn missing_exports_are_reported_not_called() {
        let extras = ProtoInputExtras::default();
        assert!(!extras.supports_translation());
        let cfg = ProtoInputExtraConfig {
            translate_xinput_to_mkb: true,
            pointer_in_mouse: true,
            get_cursor_info_hook: true,
            scale_from_x: 1280,
            scale_from_y: 720,
            ..Default::default()
        };
        let missing = extras.apply(1, &cfg, (960, 540));
        assert_eq!(
            missing,
            vec![
                "TranslateXinputtoMKB",
                "PointerInMouse",
                "GetCursorInfoHook",
                "ScaleFromX/ScaleFromY"
            ]
        );
        assert!(extras.apply(1, &ProtoInputExtraConfig::default(), (1, 1)).is_empty());
    }

    static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn rec_x2m(h: u32, v: bool) {
        CALLS.lock().unwrap().push(format!("x2m {h} {v}"));
    }
    unsafe extern "C" fn rec_m2x(h: u32, v: bool) {
        CALLS.lock().unwrap().push(format!("m2x {h} {v}"));
    }
    unsafe extern "C" fn rec_scale(h: u32, ox: i32, oy: i32, nx: i32, ny: i32) {
        CALLS.lock().unwrap().push(format!("scale {h} {ox}x{oy}->{nx}x{ny}"));
    }
    unsafe extern "C" fn rec_hook(h: u32, id: u32) {
        CALLS.lock().unwrap().push(format!("hook {h} {id}"));
    }
    #[rustfmt::skip]
    unsafe extern "C" fn rec_keys(
        h: u32, a: i32, _b: i32, _x: i32, _y: i32, _rs: i32, _ls: i32, _r: i32, _l: i32, _u: i32,
        _d: i32, _sr: i32, _sl: i32, _sri: i32, _sle: i32, _su: i32, _sd: i32, _o: i32, start: i32,
        sens: i32, _acc: i32, dead: i32,
    ) {
        CALLS.lock().unwrap().push(format!("keys {h} a={a} start={start} sens={sens} dead={dead}"));
    }
    #[rustfmt::skip]
    unsafe extern "C" fn rec_cfg(
        h: u32, swap: bool, _s: bool, _n: bool, _a1: bool, _a2: bool, _a3: bool, _b1: bool,
        _b2: bool, _b3: bool, _x1: bool, _x2: bool, _x3: bool, _y1: bool, _y2: bool, ymove: bool,
    ) {
        CALLS.lock().unwrap().push(format!("cfg {h} swap={swap} ymove={ymove}"));
    }

    #[test]
    fn present_exports_receive_arguments_in_order() {
        let extras = ProtoInputExtras {
            set_translate_xinput_to_mkb: Some(rec_x2m),
            set_translate_mkb_to_xinput: Some(rec_m2x),
            set_manual_scaling: Some(rec_scale),
            set_xinput_to_mkb_keys: Some(rec_keys),
            set_xinput_to_mkb_cfg: Some(rec_cfg),
            install_hook: Some(rec_hook),
            ..Default::default()
        };
        assert!(extras.supports_translation());
        let cfg = ProtoInputExtraConfig {
            translate_xinput_to_mkb: true,
            set_windows_hook_hook: true,
            scale_from_x: 1920,
            scale_from_y: 1080,
            deadzone: 7000,
            swap_sticks: true,
            y_point_move: true,
            ..Default::default()
        };
        CALLS.lock().unwrap().clear();
        let missing = extras.apply(5, &cfg, (960, 540));
        assert!(missing.is_empty());
        assert_eq!(
            *CALLS.lock().unwrap(),
            vec![
                "x2m 5 true",
                "m2x 5 false",
                "hook 5 23",
                "scale 5 1920x1080->960x540",
                "keys 5 a=82 start=13 sens=12 dead=7000",
                "cfg 5 swap=true ymove=true",
            ]
        );
    }
}
