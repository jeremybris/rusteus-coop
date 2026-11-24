//! Script-side ProtoInput API and the remaining .NET-style globals handlers use.
//!
//! Roughly half of the published handlers adjust ProtoInput when input is locked or
//! unlocked, from callbacks such as:
//!
//! ```js
//! Game.ProtoInput.OnInputLocked = function() {
//!   for (var i = 0; i < PlayerList.Count; i++) {
//!     var player = PlayerList[i];
//!     ProtoInput.InstallHook(player.ProtoInputInstanceHandle, ProtoInput.Values.GetCursorPosHookID);
//!   }
//! };
//! ```
//!
//! The script engine has no access to the running ProtoInput instances, so the global
//! `ProtoInput` object *records* what the callback asks for, in order (including
//! `System.Threading.Thread.Sleep` pauses), and the launcher replays the recording against
//! the real instances.

use crate::engine::ScriptEngine;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsResult, JsString, JsValue, Source};
use nucleus_common::error::{NucleusError, NucleusResult};

/// Lifecycle callbacks a handler may define to drive ProtoInput.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoCallback {
    /// `Game.ProtoInput.OnInputLocked`
    InputLocked,
    /// `Game.ProtoInput.OnInputUnlocked`
    InputUnlocked,
    /// `Game.OnFinishedSetup`
    FinishedSetup,
}

/// One entry of the `PlayerList` global seen by ProtoInput callbacks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProtoPlayer {
    pub player_index: usize,
    pub nickname: String,
    /// ProtoInput instance handle (0 when the instance has no ProtoInput).
    pub instance_handle: u32,
    pub process_id: u32,
    pub is_keyboard_player: bool,
    pub is_raw_keyboard: bool,
    pub is_raw_mouse: bool,
    /// 1-based controller index, 0 for none.
    pub gamepad_id: i32,
    pub screen_index: i32,
    pub proto_controllers: [i32; 4],
    pub bounds: (i32, i32, i32, i32),
}

/// An operation a callback requested on a ProtoInput instance.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtoCall {
    InstallHook {
        handle: u32,
        hook: u32,
    },
    UninstallHook {
        handle: u32,
        hook: u32,
    },
    EnableMessageFilter {
        handle: u32,
        filter: u32,
    },
    DisableMessageFilter {
        handle: u32,
        filter: u32,
    },
    EnableMessageBlock {
        handle: u32,
        message: u32,
    },
    DisableMessageBlock {
        handle: u32,
        message: u32,
    },
    SetDrawFakeCursor {
        handle: u32,
        enable: bool,
    },
    SetDrawFakeCursorFix {
        handle: u32,
        enable: bool,
    },
    SetRawInputBypass {
        handle: u32,
        enable: bool,
    },
    StartFocusMessageLoop {
        handle: u32,
        interval_ms: i32,
        messages: [bool; 5],
    },
    StopFocusMessageLoop {
        handle: u32,
    },
    /// `System.Threading.Thread.Sleep` issued between operations.
    Sleep {
        ms: u64,
    },
}

/// JavaScript half of the API. Kept in script so it behaves like ordinary handler-visible
/// objects (extensible, enumerable) and needs no per-function native glue.
const PRELUDE: &str = r#"
(function (global) {
  var native = global.__nucleus;
  var calls = [];
  function rec(name) {
    return function () {
      calls.push([name].concat(Array.prototype.slice.call(arguments)));
    };
  }
  global.ProtoInput = {
    _calls: calls,
    _recording: false,
    Values: {
      RegisterRawInputHookID: 0, GetRawInputDataHookID: 1, MessageFilterHookID: 2,
      GetCursorPosHookID: 3, SetCursorPosHookID: 4, GetKeyStateHookID: 5,
      GetAsyncKeyStateHookID: 6, GetKeyboardStateHookID: 7, CursorVisibilityStateHookID: 8,
      ClipCursorHookID: 9, FocusHooksHookID: 10, RenameHandlesHookID: 11, XinputHookID: 12,
      DinputOrderHookID: 13, SetWindowPosHookID: 14, BlockRawInputHookID: 15,
      FindWindowHookID: 16, CreateSingleHIDHookID: 17, WindowStyleHookID: 18,
      MoveWindowHookID: 19, AdjustWindowRectHookID: 20, RemoveBorderHookID: 21,
      RawInputFilterID: 0, MouseMoveFilterID: 1, MouseActivateFilterID: 2,
      WindowActivateFilterID: 3, WindowActivateAppFilterID: 4, WindowActvateAppFilterID: 4,
      MouseWheelFilterID: 5, MouseButtonFilterID: 6, KeyboardButtonFilterID: 7
    },
    InstallHook: rec("InstallHook"),
    UninstallHook: rec("UninstallHook"),
    EnableMessageFilter: rec("EnableMessageFilter"),
    DisableMessageFilter: rec("DisableMessageFilter"),
    EnableMessageBlock: rec("EnableMessageBlock"),
    DisableMessageBlock: rec("DisableMessageBlock"),
    SetDrawFakeCursor: rec("SetDrawFakeCursor"),
    SetDrawFakeCursorFix: rec("SetDrawFakeCursorFix"),
    SetRawInputBypass: rec("SetRawInputBypass"),
    StartFocusMessageLoop: rec("StartFocusMessageLoop"),
    StopFocusMessageLoop: rec("StopFocusMessageLoop")
  };

  var players = [];
  players.Count = 0;
  global.PlayerList = players;

  var System = global.System = global.System || {};
  System.Threading = {
    Thread: {
      Sleep: function (ms) {
        if (global.ProtoInput._recording) calls.push(["Sleep", ms]);
        else native.sleep(ms);
      }
    }
  };
  System.Environment = {
    UserName: native.env("USERNAME") || native.env("USER") || "",
    NewLine: "\r\n",
    GetEnvironmentVariable: function (name) { return native.env(name); }
  };
  var IO = System.IO = System.IO || {};
  var P = IO.Path = IO.Path || {};
  function lastSep(p) { return Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/")); }
  P.GetDirectoryName = function (p) { p = String(p); var i = lastSep(p); return i < 0 ? "" : p.substring(0, i); };
  P.GetFileName = function (p) { p = String(p); return p.substring(lastSep(p) + 1); };
  P.GetExtension = function (p) { var n = P.GetFileName(p); var i = n.lastIndexOf("."); return i < 0 ? "" : n.substring(i); };
  P.GetFileNameWithoutExtension = function (p) { var n = P.GetFileName(p); var i = n.lastIndexOf("."); return i < 0 ? n : n.substring(0, i); };
  var F = IO.File = IO.File || {};
  F.ReadAllText = function (p) { return native.readText(String(p)); };
  F.ReadAllLines = function (p) { var t = native.readText(String(p)); return t === null ? [] : t.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n"); };
  F.WriteAllText = function (p, t) { native.writeText(String(p), String(t), false); };
  F.AppendAllText = function (p, t) { native.writeText(String(p), String(t), true); };
  F.WriteAllLines = function (p, lines) { native.writeText(String(p), Array.prototype.join.call(lines, "\r\n") + "\r\n", false); };
  F.Move = function (a, b) { native.move(String(a), String(b)); };
  var D = IO.Directory = IO.Directory || {};
  D.GetFiles = function (p) { return native.listDir(String(p), false); };
  D.GetDirectories = function (p) { return native.listDir(String(p), true); };
  D.GetParent = function (p) { var d = P.GetDirectoryName(String(p).replace(/[\\\/]+$/, "")); return { FullName: d, toString: function () { return d; } }; };
})(this);
"#;

/// Applied after `Context`/`Player` are injected for an instance: fills in the members
/// handlers read from them that only make sense per player.
pub const PER_PLAYER_PRELUDE: &str = r#"
(function (global) {
  var C = global.Context, P = global.Player;
  if (typeof C !== "object" || C === null) return;
  if (!C.User) C.User = { GetLocalIP: function () { return C.LocalIP; } };
  var lists = ["CustomUserGeneralValues", "CustomUserPlayerValues", "CustomUserInstanceValues"];
  for (var i = 0; i < lists.length; i++) if (!C[lists[i]]) C[lists[i]] = [];
  if (typeof P === "object" && P !== null) {
    P.IsRawKeyboard = !!C.IsRawKeyboard;
    P.IsRawMouse = !!C.IsRawMouse;
    P.IsXInput = !C.IsKeyboardPlayer;
    P.IsController = !C.IsKeyboardPlayer;
    P.ProtoInputInstanceHandle = 0;
    P.MonitorBounds = { X: C.PosX, Y: C.PosY, Width: C.Width, Height: C.Height };
    P.ProcessID = C.ProcessID;
  }
})(this);
"#;

fn arg_string(args: &[JsValue], idx: usize, ctx: &mut Context) -> String {
    args.get(idx)
        .and_then(|v| v.to_string(ctx).ok())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default()
}

fn js_str(s: &str) -> JsValue {
    JsValue::from(JsString::from(s))
}

type NativeFn = fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>;

/// Registers the `ProtoInput`, `PlayerList` and extended `System.*` globals.
pub fn register(context: &mut Context) -> NucleusResult<()> {
    let native = JsObject::default();
    let fns: [(&str, NativeFn); 6] = [
        ("sleep", |_t, args, _c| {
            let ms = args.first().and_then(|v| v.as_number()).unwrap_or(0.0);
            std::thread::sleep(std::time::Duration::from_millis(
                ms.clamp(0.0, 600_000.0) as u64
            ));
            Ok(JsValue::undefined())
        }),
        ("env", |_t, args, ctx| {
            let name = arg_string(args, 0, ctx);
            Ok(match std::env::var(&name) {
                Ok(v) => js_str(&v),
                Err(_) => JsValue::null(),
            })
        }),
        ("readText", |_t, args, ctx| {
            let path = nucleus_common::file_util::expand_env_vars(&arg_string(args, 0, ctx));
            Ok(match std::fs::read(&path) {
                Ok(bytes) => js_str(&String::from_utf8_lossy(&bytes)),
                Err(_) => JsValue::null(),
            })
        }),
        ("writeText", |_t, args, ctx| {
            use std::io::Write;
            let path = nucleus_common::file_util::expand_env_vars(&arg_string(args, 0, ctx));
            let text = arg_string(args, 1, ctx);
            let append = args.get(2).map(|v| v.to_boolean()).unwrap_or(false);
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let ok = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .truncate(!append)
                .open(&path)
                .and_then(|mut f| f.write_all(text.as_bytes()))
                .is_ok();
            Ok(JsValue::from(ok))
        }),
        ("move", |_t, args, ctx| {
            let from = arg_string(args, 0, ctx);
            let to = arg_string(args, 1, ctx);
            Ok(JsValue::from(std::fs::rename(from, to).is_ok()))
        }),
        ("listDir", |_t, args, ctx| {
            let path = arg_string(args, 0, ctx);
            let want_dirs = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);
            let arr = boa_engine::object::builtins::JsArray::new(ctx);
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir() == want_dirs)
                    .map(|e| e.path().to_string_lossy().to_string())
                    .collect();
                names.sort();
                for n in names {
                    arr.push(js_str(&n), ctx)?;
                }
            }
            Ok(JsValue::from(arr))
        }),
    ];
    for (name, f) in fns {
        let v = crate::bridge::JsBridge::create_native_function(context, f);
        native
            .set(JsString::from(name), v, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
    }
    context
        .global_object()
        .set(
            JsString::from("__nucleus"),
            JsValue::from(native),
            false,
            context,
        )
        .map_err(|e| NucleusError::Script(e.to_string()))?;
    context
        .eval(Source::from_bytes(PRELUDE))
        .map_err(|e| NucleusError::Script(format!("ProtoInput script API failed to load: {e}")))?;
    Ok(())
}

fn player_to_js(p: &ProtoPlayer) -> String {
    let nick = serde_json::to_string(&p.nickname).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "{{PlayerID:{},Nickname:{},ProtoInputInstanceHandle:{},ProcessID:{},IsKeyboardPlayer:{},\
         IsRawKeyboard:{},IsRawMouse:{},IsXInput:{},IsController:{},GamepadId:{},ScreenIndex:{},\
         ProtoController1:{},ProtoController2:{},ProtoController3:{},ProtoController4:{},\
         MonitorBounds:{{X:{},Y:{},Width:{},Height:{}}}}}",
        p.player_index,
        nick,
        p.instance_handle,
        p.process_id,
        p.is_keyboard_player,
        p.is_raw_keyboard,
        p.is_raw_mouse,
        !p.is_keyboard_player,
        !p.is_keyboard_player,
        (p.gamepad_id - 1).max(-1),
        p.screen_index,
        p.proto_controllers[0],
        p.proto_controllers[1],
        p.proto_controllers[2],
        p.proto_controllers[3],
        p.bounds.0,
        p.bounds.1,
        p.bounds.2,
        p.bounds.3,
    )
}

fn parse_calls(json: &str) -> Vec<ProtoCall> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Some(parts) = item.as_array() else {
            continue;
        };
        let name = parts.first().and_then(|v| v.as_str()).unwrap_or_default();
        let num = |i: usize| -> f64 {
            match parts.get(i) {
                Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
                Some(serde_json::Value::Bool(b)) => *b as u8 as f64,
                Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(0.0),
                _ => 0.0,
            }
        };
        let flag = |i: usize| -> bool {
            match parts.get(i) {
                Some(serde_json::Value::Bool(b)) => *b,
                Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
                _ => false,
            }
        };
        let handle = num(1) as u32;
        let call = match name {
            "InstallHook" => ProtoCall::InstallHook {
                handle,
                hook: num(2) as u32,
            },
            "UninstallHook" => ProtoCall::UninstallHook {
                handle,
                hook: num(2) as u32,
            },
            "EnableMessageFilter" => ProtoCall::EnableMessageFilter {
                handle,
                filter: num(2) as u32,
            },
            "DisableMessageFilter" => ProtoCall::DisableMessageFilter {
                handle,
                filter: num(2) as u32,
            },
            "EnableMessageBlock" => ProtoCall::EnableMessageBlock {
                handle,
                message: num(2) as u32,
            },
            "DisableMessageBlock" => ProtoCall::DisableMessageBlock {
                handle,
                message: num(2) as u32,
            },
            "SetDrawFakeCursor" => ProtoCall::SetDrawFakeCursor {
                handle,
                enable: flag(2),
            },
            "SetDrawFakeCursorFix" => ProtoCall::SetDrawFakeCursorFix {
                handle,
                enable: flag(2),
            },
            "SetRawInputBypass" => ProtoCall::SetRawInputBypass {
                handle,
                enable: flag(2),
            },
            "StartFocusMessageLoop" => ProtoCall::StartFocusMessageLoop {
                handle,
                interval_ms: num(2) as i32,
                messages: [flag(3), flag(4), flag(5), flag(6), flag(7)],
            },
            "StopFocusMessageLoop" => ProtoCall::StopFocusMessageLoop { handle },
            "Sleep" => ProtoCall::Sleep {
                ms: num(1).clamp(0.0, 60_000.0) as u64,
            },
            _ => continue,
        };
        out.push(call);
    }
    out
}

impl ScriptEngine {
    /// Runs one of the handler's ProtoInput lifecycle callbacks with `players` exposed as
    /// `PlayerList`, and returns the operations it requested, in order.
    ///
    /// Returns `Ok(None)` when the handler does not define the callback.
    pub fn run_proto_callback(
        &mut self,
        which: ProtoCallback,
        players: &[ProtoPlayer],
    ) -> NucleusResult<Option<Vec<ProtoCall>>> {
        let target = match which {
            ProtoCallback::InputLocked => "Game.ProtoInput && Game.ProtoInput.OnInputLocked",
            ProtoCallback::InputUnlocked => "Game.ProtoInput && Game.ProtoInput.OnInputUnlocked",
            ProtoCallback::FinishedSetup => "Game.OnFinishedSetup",
        };
        let list = players
            .iter()
            .map(player_to_js)
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "(function(){{\
               if (typeof Game !== 'object' || Game === null) return null;\
               var cb = {target};\
               if (typeof cb !== 'function') return null;\
               var list = [{list}]; list.Count = list.length; PlayerList = list;\
               ProtoInput._calls.length = 0; ProtoInput._recording = true;\
               try {{ cb.call(Game); }} finally {{ ProtoInput._recording = false; }}\
               var out = JSON.stringify(ProtoInput._calls); ProtoInput._calls.length = 0;\
               return out;\
             }})()"
        );
        let result = self
            .context_mut()
            .eval(Source::from_bytes(&script))
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        Ok(result
            .as_string()
            .map(|s| parse_calls(&s.to_std_string_escaped())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn players() -> Vec<ProtoPlayer> {
        vec![
            ProtoPlayer {
                player_index: 0,
                nickname: "Player \"One\"".into(),
                instance_handle: 11,
                is_keyboard_player: true,
                is_raw_keyboard: true,
                is_raw_mouse: true,
                ..Default::default()
            },
            ProtoPlayer {
                player_index: 1,
                nickname: "Player2".into(),
                instance_handle: 22,
                gamepad_id: 1,
                proto_controllers: [1, 0, 0, 0],
                ..Default::default()
            },
        ]
    }

    const HANDLER: &str = r#"
        Game.GUID = "proto-cb";
        Game.GameName = "Proto CB";
        Game.ExecutableName = "p.exe";
        Game.ProtoInput.OnInputLocked = function() {
          for (var i = 0; i < PlayerList.Count; i++) {
            var player = PlayerList[i];
            ProtoInput.InstallHook(player.ProtoInputInstanceHandle, ProtoInput.Values.GetCursorPosHookID);
            ProtoInput.EnableMessageFilter(player.ProtoInputInstanceHandle, ProtoInput.Values.MouseButtonFilterID);
            if (player.IsKeyboardPlayer) ProtoInput.SetDrawFakeCursor(player.ProtoInputInstanceHandle, true);
          }
        };
        Game.ProtoInput.OnInputUnlocked = function() {
          for (var i = 0; i < PlayerList.Count; i++) {
            var player = PlayerList[i];
            ProtoInput.UninstallHook(player.ProtoInputInstanceHandle, ProtoInput.Values.GetCursorPosHookID);
            ProtoInput.StartFocusMessageLoop(player.ProtoInputInstanceHandle, 25, true, true, true, true, false);
            System.Threading.Thread.Sleep(1000);
            ProtoInput.StopFocusMessageLoop(player.ProtoInputInstanceHandle);
          }
        };
    "#;

    #[test]
    fn callbacks_are_recorded_in_order_for_every_player() {
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(HANDLER).unwrap();

        let locked = engine
            .run_proto_callback(ProtoCallback::InputLocked, &players())
            .unwrap()
            .expect("OnInputLocked is defined");
        assert_eq!(
            locked,
            vec![
                ProtoCall::InstallHook {
                    handle: 11,
                    hook: 3
                },
                ProtoCall::EnableMessageFilter {
                    handle: 11,
                    filter: 6
                },
                ProtoCall::SetDrawFakeCursor {
                    handle: 11,
                    enable: true
                },
                ProtoCall::InstallHook {
                    handle: 22,
                    hook: 3
                },
                ProtoCall::EnableMessageFilter {
                    handle: 22,
                    filter: 6
                },
            ]
        );

        let unlocked = engine
            .run_proto_callback(ProtoCallback::InputUnlocked, &players()[..1])
            .unwrap()
            .unwrap();
        assert_eq!(
            unlocked,
            vec![
                ProtoCall::UninstallHook {
                    handle: 11,
                    hook: 3
                },
                ProtoCall::StartFocusMessageLoop {
                    handle: 11,
                    interval_ms: 25,
                    messages: [true, true, true, true, false],
                },
                ProtoCall::Sleep { ms: 1000 },
                ProtoCall::StopFocusMessageLoop { handle: 11 },
            ],
            "the pause is recorded in sequence instead of blocking the script"
        );

        // A callback the handler does not define is reported as absent.
        assert_eq!(
            engine
                .run_proto_callback(ProtoCallback::FinishedSetup, &players())
                .unwrap(),
            None
        );
        // A second run starts from an empty recording.
        assert_eq!(
            engine
                .run_proto_callback(ProtoCallback::InputLocked, &[])
                .unwrap(),
            Some(Vec::new())
        );
    }

    #[test]
    fn dotnet_style_globals_work_from_scripts() {
        let dir = std::env::temp_dir().join(format!("nucleus_proto_script_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let base = dir.to_string_lossy().replace('\\', "\\\\");
        let mut engine = ScriptEngine::new().unwrap();
        let v = engine
            .execute(&format!(
                r#"
                var base = "{base}";
                var f = base + "\\notes.txt";
                System.IO.File.WriteAllText(f, "a");
                System.IO.File.AppendAllText(f, "b");
                [
                  System.IO.File.ReadAllText(f),
                  System.IO.Path.GetDirectoryName(f) === base,
                  System.IO.Path.GetFileName(f),
                  System.IO.Path.GetExtension(f),
                  System.IO.Path.GetFileNameWithoutExtension(f),
                  System.IO.Directory.GetFiles(base).length,
                  System.IO.Directory.GetDirectories(base).length,
                  String(System.IO.Directory.GetParent(base + "\\sub")) === base,
                  typeof System.Environment.UserName,
                  System.Environment.GetEnvironmentVariable("NUCLEUS_DEFINITELY_UNSET") === null,
                  System.IO.File.ReadAllText(base + "\\missing.txt") === null,
                  typeof System.IO.Path.Combine
                ].join("|")
                "#
            ))
            .unwrap();
        assert_eq!(
            v.as_string().unwrap().to_std_string_escaped(),
            "ab|true|notes.txt|.txt|notes|1|1|true|string|true|true|function"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_recordings_are_ignored() {
        assert!(parse_calls("not json").is_empty());
        assert_eq!(
            parse_calls(r#"[["Unknown",1],["InstallHook","7",2],["Sleep",999999],42]"#),
            vec![
                ProtoCall::InstallHook { handle: 7, hook: 2 },
                ProtoCall::Sleep { ms: 60_000 },
            ]
        );
    }
}
