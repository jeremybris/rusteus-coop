//! Tests for `Game.InjectedDlls` parsing and bridge in `nucleus-script`.

use nucleus_common::dll_injection::{InjectionTiming, TargetArchitecture};
use nucleus_script::ScriptEngine;

#[test]
fn test_parse_game_injected_dlls_from_js() {
    let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
    let script = r#"
        Game.ExecutableName = "game.exe";
        Game.InjectedDlls = [
            {
                Path: "mods/fov_fix.dll",
                Timing: "OnWindowCreated",
                DelayMs: 500,
                Required: true,
                ExportName: "SetFov",
                ExportArgument: "90",
                Architecture: "X64"
            }
        ];
    "#;

    let config = engine
        .load_handler_script(script)
        .expect("Failed to load handler script");
    assert_eq!(config.injected_dlls.len(), 1);
    let dll = &config.injected_dlls[0];
    assert_eq!(dll.path, "mods/fov_fix.dll");
    assert_eq!(dll.timing, InjectionTiming::OnWindowCreated);
    assert_eq!(dll.delay_ms, 500);
    assert!(dll.required);
    assert_eq!(dll.export_name.as_deref(), Some("SetFov"));
    assert_eq!(dll.export_argument.as_deref(), Some("90"));
    assert_eq!(dll.architecture, TargetArchitecture::X64);
}

#[test]
fn test_game_add_injected_dll_helper() {
    let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
    let script = r#"
        Game.ExecutableName = "game.exe";
        Game.AddInjectedDll("tools/hook.dll", {
            Timing: "AtStartupSuspended",
            DelayMs: 0,
            Required: false
        });
    "#;

    let config = engine
        .load_handler_script(script)
        .expect("Failed to load handler script");
    assert_eq!(config.injected_dlls.len(), 1);
    let dll = &config.injected_dlls[0];
    assert_eq!(dll.path, "tools/hook.dll");
    assert_eq!(dll.timing, InjectionTiming::AtStartupSuspended);
    assert!(!dll.required);
}
