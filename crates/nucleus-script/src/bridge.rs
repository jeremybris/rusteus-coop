//! Bridge utilities for bidirectional communication between Rust and the JavaScript runtime.

use crate::handler_context::HandlerContext;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::FunctionObjectBuilder;
use boa_engine::object::builtins::JsArray;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, JsValue};
use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_common::file_util;
use nucleus_common::types::SearchMode;
use nucleus_tools::HexEditTool;
use std::path::Path;

fn extract_target_pid(this: &JsValue, ctx: &mut Context) -> u32 {
    let pid_from_this = this
        .as_object()
        .and_then(|o| o.get(JsString::from("ProcessID"), ctx).ok())
        .and_then(|v| v.as_number())
        .unwrap_or(0.0) as u32;
    if pid_from_this != 0 {
        pid_from_this
    } else {
        std::process::id()
    }
}

fn js_err(msg: impl Into<String>) -> JsError {
    JsError::from(JsNativeError::typ().with_message(msg.into()))
}

/// Bridge helper for registering native Rust callbacks and converting types in the JS engine.
pub struct JsBridge;

impl JsBridge {
    /// Converts a Rust string slice into a Boa `JsValue`.
    pub fn string_to_js(val: &str) -> JsValue {
        JsValue::from(JsString::from(val))
    }

    /// Converts a Boa `JsValue` into a Rust string if compatible.
    pub fn js_to_string(val: &JsValue) -> Option<String> {
        val.as_string().map(|s| s.to_std_string_escaped())
    }

    /// Creates a native JS function from a function pointer.
    pub fn create_native_function(
        context: &mut Context,
        func: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    ) -> JsValue {
        let native = NativeFunction::from_fn_ptr(func);
        let js_fn = FunctionObjectBuilder::new(context.realm(), native).build();
        JsValue::from(js_fn)
    }

    /// Creates a constructible native JS function from a function pointer (callable with `new`).
    pub fn create_native_constructor(
        context: &mut Context,
        func: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    ) -> JsValue {
        let native = NativeFunction::from_fn_ptr(func);
        let js_fn = FunctionObjectBuilder::new(context.realm(), native)
            .constructor(true)
            .build();
        JsValue::from(js_fn)
    }

    /// Registers the System.IO namespace and Nucleus helper constructors into global scope.
    pub fn register_globals(context: &mut Context) -> NucleusResult<()> {
        // System.IO.Path & File & Directory
        let system_obj = boa_engine::object::JsObject::default();
        let io_obj = boa_engine::object::JsObject::default();

        // Path.Combine
        let path_obj = boa_engine::object::JsObject::default();
        let path_combine = Self::create_native_function(context, |_this, args, _ctx| {
            let mut parts = Vec::new();
            for arg in args {
                if let Some(s) = arg.as_string() {
                    parts.push(s.to_std_string_escaped());
                }
            }
            let combined = parts.join("\\");
            Ok(JsValue::from(JsString::from(combined)))
        });
        path_obj
            .set(JsString::from("Combine"), path_combine, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        io_obj
            .set(
                JsString::from("Path"),
                JsValue::from(path_obj),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // File.Copy & File.Exists
        let file_obj = boa_engine::object::JsObject::default();
        let file_copy = Self::create_native_function(context, |_this, args, _ctx| {
            if let (Some(src), Some(dst)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let src_path = src.to_std_string_escaped();
                let dst_path = dst.to_std_string_escaped();
                if let Some(parent) = Path::new(&dst_path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(&src_path, &dst_path);
            }
            Ok(JsValue::undefined())
        });
        let file_exists = Self::create_native_function(context, |_this, args, _ctx| {
            let exists = args
                .get(0)
                .and_then(|v| v.as_string())
                .map(|s| Path::new(&s.to_std_string_escaped()).exists())
                .unwrap_or(false);
            Ok(JsValue::from(exists))
        });
        let file_delete = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                let p = s.to_std_string_escaped();
                let _ = std::fs::remove_file(Path::new(&p));
            }
            Ok(JsValue::undefined())
        });
        file_obj
            .set(JsString::from("Copy"), file_copy, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        file_obj
            .set(JsString::from("Exists"), file_exists, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        file_obj
            .set(JsString::from("Delete"), file_delete, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        io_obj
            .set(
                JsString::from("File"),
                JsValue::from(file_obj),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Directory.Exists & Directory.CreateDirectory
        let dir_obj = boa_engine::object::JsObject::default();
        let dir_exists = Self::create_native_function(context, |_this, args, _ctx| {
            let exists = args
                .get(0)
                .and_then(|v| v.as_string())
                .map(|s| Path::new(&s.to_std_string_escaped()).is_dir())
                .unwrap_or(false);
            Ok(JsValue::from(exists))
        });
        let dir_create = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                let p = s.to_std_string_escaped();
                let _ = std::fs::create_dir_all(Path::new(&p));
            }
            Ok(JsValue::undefined())
        });
        dir_obj
            .set(JsString::from("Exists"), dir_exists, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        dir_obj
            .set(
                JsString::from("CreateDirectory"),
                dir_create,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        io_obj
            .set(
                JsString::from("Directory"),
                JsValue::from(dir_obj),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        system_obj
            .set(JsString::from("IO"), JsValue::from(io_obj), false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        context
            .register_global_property(
                JsString::from("System"),
                JsValue::from(system_obj),
                Attribute::all(),
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Also register Nucleus helper constructors: IniSaveInfo and CfgSaveInfo
        let ini_save_info = Self::create_native_constructor(context, |_this, args, ctx| {
            let obj = boa_engine::object::JsObject::default();
            let section = args
                .get(0)
                .and_then(|v| v.as_string())
                .cloned()
                .unwrap_or_default();
            let key = args
                .get(1)
                .and_then(|v| v.as_string())
                .cloned()
                .unwrap_or_default();
            let val = args
                .get(2)
                .and_then(|v| v.as_string())
                .cloned()
                .unwrap_or_default();
            let _ = obj.set(
                JsString::from("Section"),
                JsValue::from(section),
                false,
                ctx,
            );
            let _ = obj.set(JsString::from("Key"), JsValue::from(key), false, ctx);
            let _ = obj.set(JsString::from("Value"), JsValue::from(val), false, ctx);
            Ok(JsValue::from(obj))
        });

        let cfg_save_info = Self::create_native_constructor(context, |_this, args, ctx| {
            let obj = boa_engine::object::JsObject::default();
            let (section, key, val) = if args.len() == 2 {
                (
                    JsString::default(),
                    args.get(0)
                        .and_then(|v| v.as_string())
                        .cloned()
                        .unwrap_or_default(),
                    args.get(1)
                        .and_then(|v| v.as_string())
                        .cloned()
                        .unwrap_or_default(),
                )
            } else {
                (
                    args.get(0)
                        .and_then(|v| v.as_string())
                        .cloned()
                        .unwrap_or_default(),
                    args.get(1)
                        .and_then(|v| v.as_string())
                        .cloned()
                        .unwrap_or_default(),
                    args.get(2)
                        .and_then(|v| v.as_string())
                        .cloned()
                        .unwrap_or_default(),
                )
            };
            let _ = obj.set(
                JsString::from("Section"),
                JsValue::from(section),
                false,
                ctx,
            );
            let _ = obj.set(JsString::from("Key"), JsValue::from(key), false, ctx);
            let _ = obj.set(JsString::from("Value"), JsValue::from(val), false, ctx);
            Ok(JsValue::from(obj))
        });

        // Add to global Nucleus object
        let global = context.global_object();
        if let Ok(nucleus_val) = global.get(JsString::from("Nucleus"), context) {
            if let Some(nucleus_obj) = nucleus_val.as_object() {
                let _ =
                    nucleus_obj.set(JsString::from("IniSaveInfo"), ini_save_info, false, context);
                let _ =
                    nucleus_obj.set(JsString::from("CfgSaveInfo"), cfg_save_info, false, context);

                // Nucleus.SearchType (C# SearchType: Contains=0, Full=1, StartsWith=2, EndsWith=3)
                let search_type_obj = boa_engine::object::JsObject::default();
                let _ = search_type_obj.set(
                    JsString::from("Contains"),
                    JsValue::from(0),
                    false,
                    context,
                );
                let _ =
                    search_type_obj.set(JsString::from("Full"), JsValue::from(1), false, context);
                let _ =
                    search_type_obj.set(JsString::from("Exact"), JsValue::from(1), false, context); // alias
                let _ = search_type_obj.set(
                    JsString::from("StartsWith"),
                    JsValue::from(2),
                    false,
                    context,
                );
                let _ = search_type_obj.set(
                    JsString::from("EndsWith"),
                    JsValue::from(3),
                    false,
                    context,
                );
                let _ = nucleus_obj.set(
                    JsString::from("SearchType"),
                    JsValue::from(search_type_obj),
                    false,
                    context,
                );

                // Nucleus.DPIHandling (C# DPIHandling: True=0, Scaled=1, InvScaled=2)
                let dpi_obj = boa_engine::object::JsObject::default();
                let _ = dpi_obj.set(JsString::from("True"), JsValue::from(0), false, context);
                let _ = dpi_obj.set(JsString::from("None"), JsValue::from(0), false, context); // backwards-compatible alias
                let _ = dpi_obj.set(JsString::from("Scaled"), JsValue::from(1), false, context);
                let _ = dpi_obj.set(
                    JsString::from("InvScaled"),
                    JsValue::from(2),
                    false,
                    context,
                );
                let _ = nucleus_obj.set(
                    JsString::from("DPIHandling"),
                    JsValue::from(dpi_obj),
                    false,
                    context,
                );

                // Nucleus.SaveType (C# SaveType: None=0, INI=1, CFG=2, SCR=3, RegKey=4)
                let save_type_obj = boa_engine::object::JsObject::default();
                let _ = save_type_obj.set(JsString::from("None"), JsValue::from(0), false, context);
                let _ = save_type_obj.set(JsString::from("INI"), JsValue::from(1), false, context);
                let _ = save_type_obj.set(JsString::from("CFG"), JsValue::from(2), false, context);
                let _ = save_type_obj.set(JsString::from("SCR"), JsValue::from(3), false, context);
                let _ =
                    save_type_obj.set(JsString::from("RegKey"), JsValue::from(4), false, context);
                let _ = nucleus_obj.set(
                    JsString::from("SaveType"),
                    JsValue::from(save_type_obj),
                    false,
                    context,
                );

                // Nucleus.Folder
                let folder_obj = boa_engine::object::JsObject::default();
                let _ = folder_obj.set(
                    JsString::from("Documents"),
                    JsValue::from(0),
                    false,
                    context,
                );
                let _ = folder_obj.set(
                    JsString::from("MainGameFolder"),
                    JsValue::from(1),
                    false,
                    context,
                );
                let _ = folder_obj.set(
                    JsString::from("InstancedGameFolder"),
                    JsValue::from(2),
                    false,
                    context,
                );
                let _ = nucleus_obj.set(
                    JsString::from("Folder"),
                    JsValue::from(folder_obj),
                    false,
                    context,
                );
            }
        }

        // Global Plugins namespace
        let plugins_obj = boa_engine::object::JsObject::default();
        let _ = context.register_global_property(
            JsString::from("Plugins"),
            JsValue::from(plugins_obj),
            Attribute::all(),
        );

        Ok(())
    }

    /// Injects the `Context` object configured for `handler_ctx` into the global JS scope.
    pub fn inject_context(
        context: &mut Context,
        handler_ctx: &HandlerContext,
    ) -> NucleusResult<()> {
        let ctx_obj = boa_engine::object::JsObject::default();

        macro_rules! set_str {
            ($key:expr, $val:expr) => {
                ctx_obj
                    .set(
                        JsString::from($key),
                        JsValue::from(JsString::from($val)),
                        false,
                        context,
                    )
                    .map_err(|e| NucleusError::Script(e.to_string()))?;
            };
        }

        macro_rules! set_num {
            ($key:expr, $val:expr) => {
                ctx_obj
                    .set(JsString::from($key), JsValue::from($val), false, context)
                    .map_err(|e| NucleusError::Script(e.to_string()))?;
            };
        }

        macro_rules! set_bool {
            ($key:expr, $val:expr) => {
                ctx_obj
                    .set(JsString::from($key), JsValue::from($val), false, context)
                    .map_err(|e| NucleusError::Script(e.to_string()))?;
            };
        }

        set_num!("Width", handler_ctx.width);
        set_num!("Height", handler_ctx.height);
        set_num!("PosX", handler_ctx.pos_x);
        set_num!("PosY", handler_ctx.pos_y);
        set_num!("MonitorWidth", handler_ctx.monitor_width);
        set_num!("MonitorHeight", handler_ctx.monitor_height);
        set_num!("PlayerID", handler_ctx.player_index as i32);
        set_num!("PlayerId", handler_ctx.player_index as i32);
        set_num!("ProcessID", handler_ctx.process_id.unwrap_or(0) as i32);
        set_num!("DPIHandling", handler_ctx.dpi_handling as i32);
        set_bool!("IsKeyboardPlayer", handler_ctx.is_keyboard_player);
        set_bool!("IsRawKeyboard", handler_ctx.is_raw_keyboard);
        set_str!(
            "RawKeyboardGuid",
            handler_ctx.raw_keyboard_guid.as_deref().unwrap_or_default()
        );
        set_bool!("IsRawMouse", handler_ctx.is_raw_mouse);
        set_str!(
            "RawMouseGuid",
            handler_ctx.raw_mouse_guid.as_deref().unwrap_or_default()
        );
        set_num!("NumberOfPlayers", handler_ctx.number_of_players as i32);
        set_num!("GamepadId", handler_ctx.gamepad_id);
        set_bool!("UsePeerParty", handler_ctx.use_peerparty);
        set_str!(
            "PeerPartyRoom",
            handler_ctx.peerparty_room.as_deref().unwrap_or_default()
        );

        set_str!("Nickname", handler_ctx.nickname.as_str());
        set_str!("HandlerGUID", handler_ctx.handler_guid.as_str());
        set_str!("ExePath", handler_ctx.exe_path.as_str());
        set_str!(
            "RootInstallFolder",
            handler_ctx.root_install_folder.as_str()
        );
        set_str!("RootFolder", handler_ctx.root_folder.as_str());
        set_str!("OrigRootFolder", handler_ctx.orig_root_folder.as_str());
        set_str!(
            "SavePath",
            handler_ctx.save_path.as_deref().unwrap_or_default()
        );
        set_str!("StartArguments", handler_ctx.start_arguments.as_str());
        set_str!("AspectRatio", handler_ctx.aspect_ratio().as_str());
        set_num!("AspectRatioDecimal", handler_ctx.aspect_ratio_decimal());

        let nucleus_folder = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        set_str!("NucleusFolder", nucleus_folder.as_str());

        let local_ip = if handler_ctx.local_ip.is_empty() {
            "127.0.0.1".to_string()
        } else {
            handler_ctx.local_ip.clone()
        };
        set_str!("LocalIP", local_ip.as_str());

        // Context.Options map
        let options_obj = boa_engine::object::JsObject::default();
        for (k, v) in &handler_ctx.options {
            let _ = options_obj.set(
                JsString::from(k.as_str()),
                JsValue::from(JsString::from(v.as_str())),
                false,
                context,
            );
        }
        if !handler_ctx.options.contains_key("Native") {
            let _ = options_obj.set(
                JsString::from("Native"),
                JsValue::from(JsString::from("No")),
                false,
                context,
            );
        }
        if !handler_ctx.options.contains_key("Scale") {
            let _ = options_obj.set(
                JsString::from("Scale"),
                JsValue::from(JsString::from("NoScaled")),
                false,
                context,
            );
        }
        if !handler_ctx.options.contains_key("Hooks") {
            let _ = options_obj.set(
                JsString::from("Hooks"),
                JsValue::from(JsString::from("No")),
                false,
                context,
            );
        }
        ctx_obj
            .set(
                JsString::from("Options"),
                JsValue::from(options_obj),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        let doc_root = dirs::home_dir()
            .map(|h| {
                h.join("NucleusCoop")
                    .join(&handler_ctx.nickname)
                    .join("Documents")
            })
            .map(|p| {
                if !p.exists() {
                    let _ = std::fs::create_dir_all(&p);
                }
                p
            })
            .or_else(dirs::document_dir)
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        set_str!("DocumentsFolder", doc_root.as_str());

        // Methods on Context
        // GetFolder(folderEnum)
        let get_folder_fn = Self::create_native_function(context, |this, args, ctx| {
            let folder_type = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
            let this_obj = this.as_object();
            let key = match folder_type {
                0 => "DocumentsFolder",
                1 => "RootInstallFolder",
                _ => "RootFolder",
            };
            let val = this_obj
                .and_then(|o| o.get(JsString::from(key), ctx).ok())
                .and_then(|v| v.as_string().cloned())
                .unwrap_or_default();
            Ok(JsValue::from(val))
        });
        ctx_obj
            .set(JsString::from("GetFolder"), get_folder_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // FindLineNumberInTextFile(path, pattern, searchType)
        let find_line_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let (Some(path_val), Some(pattern_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let path = path_val.to_std_string_escaped();
                let pattern = pattern_val.to_std_string_escaped();
                let mode_num = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                let mode = match mode_num {
                    0 => SearchMode::Contains,
                    1 => SearchMode::Exact, // Full
                    2 => SearchMode::StartsWith,
                    3 => SearchMode::EndsWith,
                    _ => SearchMode::Contains,
                };
                let line_num =
                    file_util::find_line_number_in_file(Path::new(&path), &pattern, mode)
                        .unwrap_or(-1);
                Ok(JsValue::from(line_num))
            } else {
                Ok(JsValue::from(-1))
            }
        });
        ctx_obj
            .set(
                JsString::from("FindLineNumberInTextFile"),
                find_line_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ReplaceLinesInTextFile(path, linesArray)
        let replace_lines_fn = Self::create_native_function(context, |_this, args, ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let path = path_val.to_std_string_escaped();
                if let Some(arr_obj) = args.get(1).and_then(|v| v.as_object()) {
                    let len = arr_obj
                        .get(JsString::from("length"), ctx)
                        .ok()
                        .and_then(|v| v.as_number())
                        .unwrap_or(0.0) as usize;
                    let mut replacements = Vec::new();
                    for i in 0..len {
                        if let Ok(item) = arr_obj.get(i, ctx) {
                            if let Some(s) = item.as_string() {
                                let entry = s.to_std_string_escaped();
                                if let Some((num_str, val)) = entry.split_once('|') {
                                    if let Ok(num) = num_str.trim().parse::<usize>() {
                                        replacements.push((num, val.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    let _ = file_util::replace_lines_in_file(Path::new(&path), &replacements);
                }
            }
            Ok(JsValue::undefined())
        });
        ctx_obj
            .set(
                JsString::from("ReplaceLinesInTextFile"),
                replace_lines_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // RemoveLineInTextFile(path, lineNum) OR RemoveLineInTextFile(path, searchValue, searchType)
        let remove_line_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let path = path_val.to_std_string_escaped();
                if let Some(line_num) = args.get(1).and_then(|v| v.as_number()) {
                    let _ = file_util::remove_line_in_file(Path::new(&path), line_num as usize);
                    return Ok(JsValue::from(true));
                } else if let Some(search_val) = args.get(1).and_then(|v| v.as_string()) {
                    let pattern = search_val.to_std_string_escaped();
                    let mode_num = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                    let mode = match mode_num {
                        0 => SearchMode::Contains,
                        1 => SearchMode::Exact, // Full
                        2 => SearchMode::StartsWith,
                        3 => SearchMode::EndsWith,
                        _ => SearchMode::Contains,
                    };
                    let changed =
                        file_util::remove_lines_matching_pattern(Path::new(&path), &pattern, mode)
                            .unwrap_or(false);
                    return Ok(JsValue::from(changed));
                }
            }
            Ok(JsValue::from(false))
        });
        ctx_obj
            .set(
                JsString::from("RemoveLineInTextFile"),
                remove_line_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ModifySaveFile(installSavePath, saveFullPath, saveType, saveInfos)
        let modify_save_fn = Self::create_native_function(context, |_this, args, ctx| {
            if let (Some(orig_val), Some(dest_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let orig_path = orig_val.to_std_string_escaped();
                let dest_path = dest_val.to_std_string_escaped();

                // If source != dest and source exists, ALWAYS overwrite dest file with fresh source template
                if orig_path != dest_path && Path::new(&orig_path).exists() {
                    if let Some(parent) = Path::new(&dest_path).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(&orig_path, &dest_path);
                }

                let save_type_num = args.get(2).and_then(|v| v.as_number()).unwrap_or(1.0) as i32;
                // SaveType: None = 0, INI = 1, CFG = 2, SCR = 3, RegKey = 4
                let is_cfg = save_type_num == 2 || dest_path.to_ascii_lowercase().ends_with(".cfg");

                // Support both array at args[3] and variadic arguments args[3..]
                let mut info_objects = Vec::new();
                if let Some(arr_obj) = args.get(3).and_then(|v| v.as_object()) {
                    if arr_obj.is_array() {
                        let len = arr_obj
                            .get(JsString::from("length"), ctx)
                            .ok()
                            .and_then(|v| v.as_number())
                            .unwrap_or(0.0) as usize;
                        for i in 0..len {
                            if let Ok(info_val) = arr_obj.get(i, ctx) {
                                if let Some(info_obj) = info_val.as_object() {
                                    info_objects.push(info_obj.clone());
                                }
                            }
                        }
                    } else {
                        info_objects.push(arr_obj.clone());
                    }
                }
                for arg in args.iter().skip(4) {
                    if let Some(info_obj) = arg.as_object() {
                        info_objects.push(info_obj.clone());
                    }
                }

                for info_obj in info_objects {
                    let section = info_obj
                        .get(JsString::from("Section"), ctx)
                        .ok()
                        .as_ref()
                        .and_then(|v| v.as_string())
                        .map(|s| s.to_std_string_escaped())
                        .or_else(|| {
                            info_obj
                                .get(JsString::from("section"), ctx)
                                .ok()
                                .as_ref()
                                .and_then(|v| v.as_string())
                                .map(|s| s.to_std_string_escaped())
                        })
                        .unwrap_or_default();
                    let key = info_obj
                        .get(JsString::from("Key"), ctx)
                        .ok()
                        .as_ref()
                        .and_then(|v| v.as_string())
                        .map(|s| s.to_std_string_escaped())
                        .or_else(|| {
                            info_obj
                                .get(JsString::from("key"), ctx)
                                .ok()
                                .as_ref()
                                .and_then(|v| v.as_string())
                                .map(|s| s.to_std_string_escaped())
                        })
                        .unwrap_or_default();
                    let val = info_obj
                        .get(JsString::from("Value"), ctx)
                        .ok()
                        .as_ref()
                        .and_then(|v| v.as_string())
                        .map(|s| s.to_std_string_escaped())
                        .or_else(|| {
                            info_obj
                                .get(JsString::from("value"), ctx)
                                .ok()
                                .as_ref()
                                .and_then(|v| v.as_string())
                                .map(|s| s.to_std_string_escaped())
                        })
                        .unwrap_or_default();

                    if is_cfg {
                        let _ = file_util::modify_cfg_value(
                            Path::new(&dest_path),
                            &section,
                            &key,
                            &val,
                        );
                    } else {
                        let _ = file_util::modify_ini_value(
                            Path::new(&dest_path),
                            &section,
                            &key,
                            &val,
                        );
                    }
                }
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("ModifySaveFile"),
                modify_save_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // PatchFile(src, dst, findBytes/str, replaceBytes/str)
        let patch_file_fn = Self::create_native_function(context, |_this, args, ctx| {
            if let (Some(src_val), Some(dst_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let src_path = src_val.to_std_string_escaped();
                let dst_path = dst_val.to_std_string_escaped();
                let find_bytes = Self::extract_bytes_from_arg(args.get(2), ctx);
                let replace_bytes = Self::extract_bytes_from_arg(args.get(3), ctx);

                if !find_bytes.is_empty() && find_bytes.len() == replace_bytes.len() {
                    let _ = HexEditTool::patch_file(
                        Path::new(&src_path),
                        Path::new(&dst_path),
                        &find_bytes,
                        &replace_bytes,
                        false,
                    );
                }
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(JsString::from("PatchFile"), patch_file_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // PatchFileFindAll(src, dst, findBytes/str, replaceBytes/str)
        let patch_file_all_fn = Self::create_native_function(context, |_this, args, ctx| {
            if let (Some(src_val), Some(dst_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let src_path = src_val.to_std_string_escaped();
                let dst_path = dst_val.to_std_string_escaped();
                let find_bytes = Self::extract_bytes_from_arg(args.get(2), ctx);
                let replace_bytes = Self::extract_bytes_from_arg(args.get(3), ctx);

                if !find_bytes.is_empty() && find_bytes.len() == replace_bytes.len() {
                    let _ = HexEditTool::patch_file(
                        Path::new(&src_path),
                        Path::new(&dst_path),
                        &find_bytes,
                        &replace_bytes,
                        true,
                    );
                }
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("PatchFileFindAll"),
                patch_file_all_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // PatchFileFindPattern(src, dst, origPattern, patchPattern, patchall)
        // OR PatchFileFindPattern(src, dst, origPattern, insert, offset, patchall)
        let patch_pattern_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let (Some(src_val), Some(dst_val), Some(orig_pat)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
                args.get(2).and_then(|v| v.as_string()),
            ) {
                let src_path = src_val.to_std_string_escaped();
                let dst_path = dst_val.to_std_string_escaped();
                let orig_pattern = orig_pat.to_std_string_escaped();

                if let Some(patch_pat) = args.get(3).and_then(|v| v.as_string()) {
                    let patch_pattern = patch_pat.to_std_string_escaped();
                    let patch_all = args.get(4).and_then(|v| v.as_boolean()).unwrap_or(true);
                    let _ = HexEditTool::patch_file_find_pattern(
                        Path::new(&src_path),
                        Path::new(&dst_path),
                        &orig_pattern,
                        &patch_pattern,
                        patch_all,
                    );
                } else if let Some(insert_val) = args.get(3).and_then(|v| v.as_number()) {
                    let insert = insert_val as i32;
                    let offset = args.get(4).and_then(|v| v.as_number()).unwrap_or(1.0) as usize;
                    let patch_all = args.get(5).and_then(|v| v.as_boolean()).unwrap_or(true);
                    let _ = HexEditTool::patch_file_find_pattern_insert(
                        Path::new(&src_path),
                        Path::new(&dst_path),
                        &orig_pattern,
                        insert,
                        offset,
                        patch_all,
                    );
                }
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("PatchFileFindPattern"),
                patch_pattern_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ConvertToBytes(num)
        let convert_bytes_fn = Self::create_native_function(context, |_this, args, ctx| {
            let bytes = if let Some(n) = args.get(0).and_then(|v| v.as_number()) {
                // If fractional, treat as float; otherwise check range
                if n.fract() != 0.0 {
                    (n as f32).to_le_bytes().to_vec()
                } else {
                    (n as i32).to_le_bytes().to_vec()
                }
            } else if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                s.to_std_string_escaped().into_bytes()
            } else {
                Vec::new()
            };

            let arr = JsArray::new(ctx);
            for b in bytes {
                let _ = arr.push(JsValue::from(b), ctx);
            }
            Ok(JsValue::from(arr))
        });
        ctx_obj
            .set(
                JsString::from("ConvertToBytes"),
                convert_bytes_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Log(msg)
        let log_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(msg) = args.get(0).and_then(|v| v.as_string()) {
                tracing::info!("[JS Context]: {}", msg.to_std_string_escaped());
            }
            Ok(JsValue::undefined())
        });
        ctx_obj
            .set(JsString::from("Log"), log_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ChangeXmlAttributeValue(path, xpath, attrName, attrValue)
        let change_xml_attr_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let (Some(path_val), Some(xpath_val), Some(name_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
                args.get(2).and_then(|v| v.as_string()),
            ) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);
                let xpath = xpath_val.to_std_string_escaped();
                let attr_name = name_val.to_std_string_escaped();

                let attr_val = if let Some(s) = args.get(3).and_then(|v| v.as_string()) {
                    s.to_std_string_escaped()
                } else if let Some(n) = args.get(3).and_then(|v| v.as_number()) {
                    n.to_string()
                } else if let Some(b) = args.get(3).and_then(|v| v.as_boolean()) {
                    b.to_string()
                } else {
                    String::new()
                };

                let _ = file_util::change_xml_attribute_value(
                    Path::new(&path_str),
                    &xpath,
                    &attr_name,
                    &attr_val,
                );
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("ChangeXmlAttributeValue"),
                change_xml_attr_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ChangeXmlNodeValue / ChangeXmlNodeInnerTextValue(path, xpath, value)
        let change_xml_node_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let (Some(path_val), Some(xpath_val)) = (
                args.get(0).and_then(|v| v.as_string()),
                args.get(1).and_then(|v| v.as_string()),
            ) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);
                let xpath = xpath_val.to_std_string_escaped();

                let node_val = if let Some(s) = args.get(2).and_then(|v| v.as_string()) {
                    s.to_std_string_escaped()
                } else if let Some(n) = args.get(2).and_then(|v| v.as_number()) {
                    n.to_string()
                } else if let Some(b) = args.get(2).and_then(|v| v.as_boolean()) {
                    b.to_string()
                } else {
                    String::new()
                };

                let _ = file_util::change_xml_node_value(Path::new(&path_str), &xpath, &node_val);
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("ChangeXmlNodeValue"),
                change_xml_node_fn.clone(),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        ctx_obj
            .set(
                JsString::from("ChangeXmlNodeInnerTextValue"),
                change_xml_node_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // WriteTextFile(path, ...lines) OR WriteTextFile(path, linesArray)
        let write_text_fn = Self::create_native_function(context, |_this, args, ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);

                let content = if let Some(arr_obj) = args.get(1).and_then(|v| v.as_object()) {
                    if arr_obj.is_array() {
                        let len = arr_obj
                            .get(JsString::from("length"), ctx)
                            .ok()
                            .and_then(|v| v.as_number())
                            .unwrap_or(0.0) as usize;
                        let mut lines = Vec::with_capacity(len);
                        for i in 0..len {
                            if let Ok(item) = arr_obj.get(i, ctx) {
                                if let Some(s) = item.as_string() {
                                    lines.push(s.to_std_string_escaped());
                                } else if let Some(n) = item.as_number() {
                                    lines.push(n.to_string());
                                }
                            }
                        }
                        lines.join("\r\n")
                    } else {
                        String::new()
                    }
                } else if args.len() > 2 {
                    let mut lines = Vec::new();
                    for arg in args.iter().skip(1) {
                        if let Some(s) = arg.as_string() {
                            lines.push(s.to_std_string_escaped());
                        } else if let Some(n) = arg.as_number() {
                            lines.push(n.to_string());
                        }
                    }
                    lines.join("\r\n")
                } else if let Some(s) = args.get(1).and_then(|v| v.as_string()) {
                    s.to_std_string_escaped()
                } else {
                    String::new()
                };

                let _ = file_util::write_text_file(Path::new(&path_str), &content);
            }
            Ok(JsValue::from(true))
        });
        ctx_obj
            .set(
                JsString::from("WriteTextFile"),
                write_text_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ReadTextFile(path)
        let read_text_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);
                if let Ok(content) = file_util::read_text_file(Path::new(&path_str)) {
                    return Ok(JsValue::from(JsString::from(content)));
                }
            }
            Ok(JsValue::from(JsString::from("")))
        });
        ctx_obj
            .set(JsString::from("ReadTextFile"), read_text_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // FileExists(path)
        let file_exists_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);
                return Ok(JsValue::from(Path::new(&path_str).is_file()));
            }
            Ok(JsValue::from(false))
        });
        ctx_obj
            .set(JsString::from("FileExists"), file_exists_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // DirectoryExists(path)
        let dir_exists_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let raw_path = path_val.to_std_string_escaped();
                let path_str = file_util::expand_env_vars(&raw_path);
                return Ok(JsValue::from(Path::new(&path_str).is_dir()));
            }
            Ok(JsValue::from(false))
        });
        ctx_obj
            .set(
                JsString::from("DirectoryExists"),
                dir_exists_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // RandomInt(min, max)
        let random_int_fn = Self::create_native_function(context, |_this, args, _ctx| {
            let min = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i64;
            let max = args.get(1).and_then(|v| v.as_number()).unwrap_or(100.0) as i64;
            let delta = (max - min).max(1);
            let pseudo = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                % delta as u128) as i64;
            Ok(JsValue::from(min + pseudo))
        });
        ctx_obj
            .set(JsString::from("RandomInt"), random_int_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // ConvertToInt(num)
        let convert_to_int_fn = Self::create_native_function(context, |_this, args, _ctx| {
            if let Some(n) = args.get(0).and_then(|v| v.as_number()) {
                Ok(JsValue::from(n as i32))
            } else if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                let parsed = s.to_std_string_escaped().trim().parse::<i32>().unwrap_or(0);
                Ok(JsValue::from(parsed))
            } else {
                Ok(JsValue::from(0))
            }
        });
        ctx_obj
            .set(
                JsString::from("ConvertToInt"),
                convert_to_int_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // HideTaskBar / HideTaskbar
        let hide_taskbar_fn =
            Self::create_native_function(context, |_this, _args, _ctx| Ok(JsValue::undefined()));
        ctx_obj
            .set(
                JsString::from("HideTaskBar"),
                hide_taskbar_fn.clone(),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        ctx_obj
            .set(
                JsString::from("HideTaskbar"),
                hide_taskbar_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Initialize _ahkDirectives tracking array
        let ahk_directives_arr = JsArray::new(context);
        for directive in &handler_ctx.ahk_directives {
            let d_obj = boa_engine::object::JsObject::default();
            let _ = d_obj.set(
                JsString::from("id"),
                JsValue::from(JsString::from(directive.id.as_str())),
                false,
                context,
            );
            let _ = d_obj.set(
                JsString::from("is_inline"),
                JsValue::from(directive.is_inline),
                false,
                context,
            );
            let _ = d_obj.set(
                JsString::from("content"),
                JsValue::from(JsString::from(directive.content.as_str())),
                false,
                context,
            );
            if let Some(ref ver) = directive.version {
                let _ = d_obj.set(
                    JsString::from("version"),
                    JsValue::from(JsString::from(ver.as_str())),
                    false,
                    context,
                );
            }
            if let Some(ref tim) = directive.timing {
                let _ = d_obj.set(
                    JsString::from("timing"),
                    JsValue::from(JsString::from(tim.as_str())),
                    false,
                    context,
                );
            }
            let args_arr = JsArray::new(context);
            for arg in &directive.args {
                let _ = args_arr.push(JsValue::from(JsString::from(arg.as_str())), context);
            }
            let _ = d_obj.set(
                JsString::from("args"),
                JsValue::from(args_arr),
                false,
                context,
            );
            let _ = ahk_directives_arr.push(JsValue::from(d_obj), context);
        }
        ctx_obj
            .set(
                JsString::from("_ahkDirectives"),
                JsValue::from(ahk_directives_arr),
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ImportAhkFile(path, [options_or_version], [timing], [args])
        let import_ahk_fn = Self::create_native_function(context, |this, args, ctx| {
            if let Some(path_val) = args.get(0).and_then(|v| v.as_string()) {
                let path_str = path_val.to_std_string_escaped();
                let mut version = None;
                let mut timing = None;
                let mut script_args = Vec::new();

                if let Some(opt_obj) = args.get(1).and_then(|v| v.as_object()) {
                    if !opt_obj.is_array() {
                        if let Ok(v) = opt_obj.get(JsString::from("version"), ctx) {
                            if let Some(s) = v.as_string() {
                                version = Some(s.to_std_string_escaped());
                            }
                        }
                        if let Ok(v) = opt_obj.get(JsString::from("timing"), ctx) {
                            if let Some(s) = v.as_string() {
                                timing = Some(s.to_std_string_escaped());
                            }
                        }
                        if let Ok(v) = opt_obj.get(JsString::from("args"), ctx) {
                            if let Some(arr) = v.as_object() {
                                let len = arr
                                    .get(JsString::from("length"), ctx)
                                    .ok()
                                    .and_then(|l| l.as_number())
                                    .unwrap_or(0.0)
                                    as usize;
                                for i in 0..len {
                                    if let Ok(elem) = arr.get(i, ctx) {
                                        if let Some(s) = elem.as_string() {
                                            script_args.push(s.to_std_string_escaped());
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        let len = opt_obj
                            .get(JsString::from("length"), ctx)
                            .ok()
                            .and_then(|l| l.as_number())
                            .unwrap_or(0.0) as usize;
                        for i in 0..len {
                            if let Ok(elem) = opt_obj.get(i, ctx) {
                                if let Some(s) = elem.as_string() {
                                    script_args.push(s.to_std_string_escaped());
                                }
                            }
                        }
                    }
                } else if let Some(ver_str) = args.get(1).and_then(|v| v.as_string()) {
                    version = Some(ver_str.to_std_string_escaped());
                }

                if timing.is_none() {
                    if let Some(tim_str) = args.get(2).and_then(|v| v.as_string()) {
                        timing = Some(tim_str.to_std_string_escaped());
                    }
                }

                if script_args.is_empty() {
                    if let Some(arr) = args.get(3).and_then(|v| v.as_object()) {
                        let len = arr
                            .get(JsString::from("length"), ctx)
                            .ok()
                            .and_then(|l| l.as_number())
                            .unwrap_or(0.0) as usize;
                        for i in 0..len {
                            if let Ok(elem) = arr.get(i, ctx) {
                                if let Some(s) = elem.as_string() {
                                    script_args.push(s.to_std_string_escaped());
                                }
                            }
                        }
                    }
                }

                if let Some(this_obj) = this.as_object() {
                    if let Ok(dir_val) = this_obj.get(JsString::from("_ahkDirectives"), ctx) {
                        if let Some(dir_arr) = dir_val.as_object() {
                            let len = dir_arr
                                .get(JsString::from("length"), ctx)
                                .ok()
                                .and_then(|l| l.as_number())
                                .unwrap_or(0.0) as u32;
                            let d_obj = boa_engine::object::JsObject::default();
                            let _ = d_obj.set(
                                JsString::from("id"),
                                JsValue::from(JsString::from(format!("file_{len}"))),
                                false,
                                ctx,
                            );
                            let _ = d_obj.set(
                                JsString::from("is_inline"),
                                JsValue::from(false),
                                false,
                                ctx,
                            );
                            let _ = d_obj.set(
                                JsString::from("content"),
                                JsValue::from(JsString::from(path_str.as_str())),
                                false,
                                ctx,
                            );
                            if let Some(ver) = version {
                                let _ = d_obj.set(
                                    JsString::from("version"),
                                    JsValue::from(JsString::from(ver.as_str())),
                                    false,
                                    ctx,
                                );
                            }
                            if let Some(tim) = timing {
                                let _ = d_obj.set(
                                    JsString::from("timing"),
                                    JsValue::from(JsString::from(tim.as_str())),
                                    false,
                                    ctx,
                                );
                            }
                            let args_arr = JsArray::new(ctx);
                            for a in script_args {
                                let _ =
                                    args_arr.push(JsValue::from(JsString::from(a.as_str())), ctx);
                            }
                            let _ = d_obj.set(
                                JsString::from("args"),
                                JsValue::from(args_arr),
                                false,
                                ctx,
                            );
                            let _ = dir_arr.set(len, JsValue::from(d_obj), false, ctx);
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        });
        ctx_obj
            .set(
                JsString::from("ImportAhkFile"),
                import_ahk_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.RunAhkCode(code, [options_or_version], [timing], [args])
        let run_ahk_fn = Self::create_native_function(context, |this, args, ctx| {
            if let Some(code_val) = args.get(0).and_then(|v| v.as_string()) {
                let code_str = code_val.to_std_string_escaped();
                let mut version = None;
                let mut timing = None;
                let mut script_args = Vec::new();

                if let Some(opt_obj) = args.get(1).and_then(|v| v.as_object()) {
                    if !opt_obj.is_array() {
                        if let Ok(v) = opt_obj.get(JsString::from("version"), ctx) {
                            if let Some(s) = v.as_string() {
                                version = Some(s.to_std_string_escaped());
                            }
                        }
                        if let Ok(v) = opt_obj.get(JsString::from("timing"), ctx) {
                            if let Some(s) = v.as_string() {
                                timing = Some(s.to_std_string_escaped());
                            }
                        }
                        if let Ok(v) = opt_obj.get(JsString::from("args"), ctx) {
                            if let Some(arr) = v.as_object() {
                                let len = arr
                                    .get(JsString::from("length"), ctx)
                                    .ok()
                                    .and_then(|l| l.as_number())
                                    .unwrap_or(0.0)
                                    as usize;
                                for i in 0..len {
                                    if let Ok(elem) = arr.get(i, ctx) {
                                        if let Some(s) = elem.as_string() {
                                            script_args.push(s.to_std_string_escaped());
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(ver_str) = args.get(1).and_then(|v| v.as_string()) {
                    version = Some(ver_str.to_std_string_escaped());
                }

                if timing.is_none() {
                    if let Some(tim_str) = args.get(2).and_then(|v| v.as_string()) {
                        timing = Some(tim_str.to_std_string_escaped());
                    }
                }

                if let Some(this_obj) = this.as_object() {
                    if let Ok(dir_val) = this_obj.get(JsString::from("_ahkDirectives"), ctx) {
                        if let Some(dir_arr) = dir_val.as_object() {
                            let len = dir_arr
                                .get(JsString::from("length"), ctx)
                                .ok()
                                .and_then(|l| l.as_number())
                                .unwrap_or(0.0) as u32;
                            let d_obj = boa_engine::object::JsObject::default();
                            let _ = d_obj.set(
                                JsString::from("id"),
                                JsValue::from(JsString::from(format!("inline_{len}"))),
                                false,
                                ctx,
                            );
                            let _ = d_obj.set(
                                JsString::from("is_inline"),
                                JsValue::from(true),
                                false,
                                ctx,
                            );
                            let _ = d_obj.set(
                                JsString::from("content"),
                                JsValue::from(JsString::from(code_str.as_str())),
                                false,
                                ctx,
                            );
                            if let Some(ver) = version {
                                let _ = d_obj.set(
                                    JsString::from("version"),
                                    JsValue::from(JsString::from(ver.as_str())),
                                    false,
                                    ctx,
                                );
                            }
                            if let Some(tim) = timing {
                                let _ = d_obj.set(
                                    JsString::from("timing"),
                                    JsValue::from(JsString::from(tim.as_str())),
                                    false,
                                    ctx,
                                );
                            }
                            let args_arr = JsArray::new(ctx);
                            for a in script_args {
                                let _ =
                                    args_arr.push(JsValue::from(JsString::from(a.as_str())), ctx);
                            }
                            let _ = d_obj.set(
                                JsString::from("args"),
                                JsValue::from(args_arr),
                                false,
                                ctx,
                            );
                            let _ = dir_arr.set(len, JsValue::from(d_obj), false, ctx);
                        }
                    }
                }
            }
            Ok(JsValue::undefined())
        });
        ctx_obj
            .set(JsString::from("RunAhkCode"), run_ahk_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // DLL Injection & Memory Engine APIs on Context
        // Context.InjectDll(path, [options])
        let inject_dll_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let path = args
                .get(0)
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let mut config = nucleus_common::dll_injection::DllInjectionConfig::new(path);
            if let Some(opts) = args.get(1).and_then(|v| v.as_object()) {
                if let Ok(v) = opts.get(JsString::from("Timing"), ctx) {
                    if let Some(s) = v.as_string() {
                        let t_str = s.to_std_string_escaped().to_lowercase();
                        if t_str.contains("startupsuspended") || t_str.contains("suspended") {
                            config.timing =
                                nucleus_common::dll_injection::InjectionTiming::AtStartupSuspended;
                        } else if t_str.contains("window") {
                            config.timing =
                                nucleus_common::dll_injection::InjectionTiming::OnWindowCreated;
                        } else if t_str.contains("delay") {
                            config.timing =
                                nucleus_common::dll_injection::InjectionTiming::AfterDelayMs(
                                    config.delay_ms,
                                );
                        } else if t_str.contains("manual") {
                            config.timing = nucleus_common::dll_injection::InjectionTiming::Manual;
                        } else {
                            config.timing =
                                nucleus_common::dll_injection::InjectionTiming::PostStartup;
                        }
                    }
                }
                if let Ok(v) = opts.get(JsString::from("DelayMs"), ctx) {
                    if let Some(n) = v.as_number() {
                        config.delay_ms = n.max(0.0) as u64;
                    }
                }
                if let Ok(v) = opts.get(JsString::from("Required"), ctx) {
                    if let Some(b) = v.as_boolean() {
                        config.required = b;
                    }
                }
                if let Ok(v) = opts.get(JsString::from("ExportName"), ctx) {
                    if let Some(s) = v.as_string() {
                        let export_str = s.to_std_string_escaped();
                        if !export_str.is_empty() {
                            config.export_name = Some(export_str);
                        }
                    }
                }
                if let Ok(v) = opts.get(JsString::from("ExportArgument"), ctx) {
                    if let Some(s) = v.as_string() {
                        config.export_argument = Some(s.to_std_string_escaped());
                    }
                }
            }
            match nucleus_hook::injector::DllInjector::inject_advanced(pid, &config) {
                Ok(handle) => Ok(JsValue::from(handle as f64)),
                Err(e) => Err(js_err(format!("Context.InjectDll failed: {e}"))),
            }
        });
        ctx_obj
            .set(JsString::from("InjectDll"), inject_dll_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.CallDllExport(dll_path, module_handle, export_name, [arg_str])
        let call_export_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let dll_path_str = args
                .get(0)
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let module_handle = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let export_name = args
                .get(2)
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let arg_str = args
                .get(3)
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped());

            match nucleus_hook::remote_call::RemoteRpc::call_remote_export(
                pid,
                Path::new(&dll_path_str),
                module_handle,
                &export_name,
                arg_str.as_deref(),
            ) {
                Ok(code) => Ok(JsValue::from(code as f64)),
                Err(e) => Err(js_err(format!("Context.CallDllExport failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("CallDllExport"),
                call_export_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.SendDllMessage(msg)
        let send_dll_msg_fn = Self::create_native_function(context, |this, args, ctx| {
            let player_id = this
                .as_object()
                .and_then(|o| o.get(JsString::from("PlayerID"), ctx).ok())
                .and_then(|v| v.as_number())
                .unwrap_or(0.0) as u32;

            let payload = if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                s.to_std_string_escaped()
            } else {
                String::new()
            };

            let pipe_name = format!(r"\\.\pipe\nucleus_instance_{player_id}");
            match nucleus_hook::ipc::send_pipe_message_sync(&pipe_name, &payload) {
                Ok(resp) => Ok(JsValue::from(JsString::from(resp.as_str()))),
                Err(e) => Err(js_err(format!("Context.SendDllMessage failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("SendDllMessage"),
                send_dll_msg_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ReadMemoryInt(address)
        let read_mem_int_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            match nucleus_hook::memory_engine::MemoryEngine::read_value::<i32>(pid, address) {
                Ok(val) => Ok(JsValue::from(val)),
                Err(e) => Err(js_err(format!("Context.ReadMemoryInt failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("ReadMemoryInt"),
                read_mem_int_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ReadMemoryFloat(address)
        let read_mem_float_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            match nucleus_hook::memory_engine::MemoryEngine::read_value::<f32>(pid, address) {
                Ok(val) => Ok(JsValue::from(val as f64)),
                Err(e) => Err(js_err(format!("Context.ReadMemoryFloat failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("ReadMemoryFloat"),
                read_mem_float_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ReadMemoryBytes(address, length)
        let read_mem_bytes_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let length = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let mut buf = vec![0u8; length];
            match nucleus_hook::memory_engine::MemoryEngine::read_bytes(pid, address, &mut buf) {
                Ok(read) => {
                    let arr = JsArray::new(ctx);
                    for b in &buf[..read] {
                        let _ = arr.push(JsValue::from(*b), ctx);
                    }
                    Ok(JsValue::from(arr))
                }
                Err(e) => Err(js_err(format!("Context.ReadMemoryBytes failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("ReadMemoryBytes"),
                read_mem_bytes_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.WriteMemoryInt(address, value)
        let write_mem_int_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let value = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
            match nucleus_hook::memory_engine::MemoryEngine::write_value::<i32>(
                pid, address, &value,
            ) {
                Ok(()) => Ok(JsValue::from(true)),
                Err(e) => Err(js_err(format!("Context.WriteMemoryInt failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("WriteMemoryInt"),
                write_mem_int_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.WriteMemoryFloat(address, value)
        let write_mem_float_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let value = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as f32;
            match nucleus_hook::memory_engine::MemoryEngine::write_value::<f32>(
                pid, address, &value,
            ) {
                Ok(()) => Ok(JsValue::from(true)),
                Err(e) => Err(js_err(format!("Context.WriteMemoryFloat failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("WriteMemoryFloat"),
                write_mem_float_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.WriteMemoryBytes(address, bytes)
        let write_mem_bytes_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let mut bytes = Vec::new();
            if let Some(arr_obj) = args.get(1).and_then(|v| v.as_object()) {
                let len = arr_obj
                    .get(JsString::from("length"), ctx)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;
                for i in 0..len {
                    if let Ok(item) = arr_obj.get(i, ctx) {
                        if let Some(b) = item.as_number() {
                            bytes.push(b as u8);
                        }
                    }
                }
            }
            match nucleus_hook::memory_engine::MemoryEngine::write_bytes(pid, address, &bytes) {
                Ok(_) => Ok(JsValue::from(true)),
                Err(e) => Err(js_err(format!("Context.WriteMemoryBytes failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("WriteMemoryBytes"),
                write_mem_bytes_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ScanMemory(pattern, mask)
        let scan_mem_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let mut pattern = Vec::new();
            if let Some(s) = args.get(0).and_then(|v| v.as_string()) {
                let hex_str = s.to_std_string_escaped();
                for part in hex_str.split_whitespace() {
                    if part == "?" || part == "??" {
                        pattern.push(0x00);
                    } else if let Ok(b) = u8::from_str_radix(part, 16) {
                        pattern.push(b);
                    }
                }
            } else if let Some(arr_obj) = args.get(0).and_then(|v| v.as_object()) {
                let len = arr_obj
                    .get(JsString::from("length"), ctx)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;
                for i in 0..len {
                    if let Ok(item) = arr_obj.get(i, ctx) {
                        if let Some(b) = item.as_number() {
                            pattern.push(b as u8);
                        }
                    }
                }
            }
            let mask = args
                .get(1)
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            match nucleus_hook::memory_engine::MemoryEngine::aob_scan(pid, &pattern, &mask) {
                Ok(addrs) => {
                    let arr = JsArray::new(ctx);
                    for addr in addrs {
                        let _ = arr.push(JsValue::from(addr as f64), ctx);
                    }
                    Ok(JsValue::from(arr))
                }
                Err(e) => Err(js_err(format!("Context.ScanMemory failed: {e}"))),
            }
        });
        ctx_obj
            .set(JsString::from("ScanMemory"), scan_mem_fn, false, context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.ResolvePointer(base, offsets_arr)
        let resolve_ptr_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let base = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let mut offsets = Vec::new();
            if let Some(arr_obj) = args.get(1).and_then(|v| v.as_object()) {
                let len = arr_obj
                    .get(JsString::from("length"), ctx)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;
                for i in 0..len {
                    if let Ok(item) = arr_obj.get(i, ctx) {
                        if let Some(offset) = item.as_number() {
                            offsets.push(offset as usize);
                        }
                    }
                }
            }
            match nucleus_hook::memory_engine::MemoryEngine::resolve_pointer_chain(
                pid,
                base,
                &offsets,
                cfg!(target_pointer_width = "64"),
            ) {
                Ok(addr) => Ok(JsValue::from(addr as f64)),
                Err(e) => Err(js_err(format!("Context.ResolvePointer failed: {e}"))),
            }
        });
        ctx_obj
            .set(
                JsString::from("ResolvePointer"),
                resolve_ptr_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.FreezeMemoryInt(address, value)
        let freeze_mem_int_fn = Self::create_native_function(context, |this, args, ctx| {
            let pid = extract_target_pid(this, ctx);
            let address = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as usize;
            let value = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
            let id = nucleus_hook::memory_engine::MemoryEngine::freeze_value(
                pid,
                address,
                value.to_le_bytes().to_vec(),
            );
            Ok(JsValue::from(id as f64))
        });
        ctx_obj
            .set(
                JsString::from("FreezeMemoryInt"),
                freeze_mem_int_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Context.UnfreezeMemory(id)
        let unfreeze_mem_fn = Self::create_native_function(context, |_this, args, _ctx| {
            let id = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
            let result = nucleus_hook::memory_engine::MemoryEngine::unfreeze_value(id);
            Ok(JsValue::from(result))
        });
        ctx_obj
            .set(
                JsString::from("UnfreezeMemory"),
                unfreeze_mem_fn,
                false,
                context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        crate::context_ext::install(context, &ctx_obj, handler_ctx)?;

        // Register global Context
        context
            .register_global_property(
                JsString::from("Context"),
                JsValue::from(ctx_obj),
                Attribute::all(),
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Register global Player
        let player_obj = boa_engine::object::JsObject::default();
        let _ = player_obj.set(
            JsString::from("PlayerID"),
            JsValue::from(handler_ctx.player_index as i32),
            false,
            context,
        );
        let _ = player_obj.set(
            JsString::from("PlayerId"),
            JsValue::from(handler_ctx.player_index as i32),
            false,
            context,
        );
        let _ = player_obj.set(
            JsString::from("Nickname"),
            JsValue::from(JsString::from(handler_ctx.nickname.as_str())),
            false,
            context,
        );
        let _ = player_obj.set(
            JsString::from("IsKeyboardPlayer"),
            JsValue::from(handler_ctx.is_keyboard_player),
            false,
            context,
        );
        let _ = player_obj.set(
            JsString::from("GamepadId"),
            JsValue::from(handler_ctx.gamepad_id),
            false,
            context,
        );

        // Player.Owner object representing the layout/screen partition
        let owner_obj = boa_engine::object::JsObject::default();
        let _ = owner_obj.set(
            JsString::from("Width"),
            JsValue::from(handler_ctx.monitor_width),
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("Height"),
            JsValue::from(handler_ctx.monitor_height),
            false,
            context,
        );

        let n_players = handler_ctx.number_of_players;
        let layout_lower = handler_ctx.layout.to_lowercase();

        let is_dual_horiz = n_players == 2
            && (layout_lower.contains("1x2")
                || layout_lower.contains("horizontal")
                || layout_lower.contains("stacked"));
        let is_dual_vert = n_players == 2
            && (layout_lower.contains("2x1")
                || layout_lower.contains("vertical")
                || layout_lower.contains("side-by-side"));
        let is_fullscreen = n_players == 1 || layout_lower.contains("fullscreen");
        let is_four =
            n_players == 4 || layout_lower.contains("2x2") || layout_lower.contains("four");
        let p_count = n_players as i32;

        let _ = owner_obj.set(
            JsString::from("_is_dual_horizontal"),
            JsValue::from(is_dual_horiz),
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("_is_dual_vertical"),
            JsValue::from(is_dual_vert),
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("_is_fullscreen"),
            JsValue::from(is_fullscreen),
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("_is_four_players"),
            JsValue::from(is_four),
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("_player_count"),
            JsValue::from(p_count),
            false,
            context,
        );

        let dual_horiz_fn = Self::create_native_function(context, |this, _args, ctx| {
            let val = this
                .as_object()
                .and_then(|o| o.get(JsString::from("_is_dual_horizontal"), ctx).ok())
                .and_then(|v| v.as_boolean())
                .unwrap_or(false);
            Ok(JsValue::from(val))
        });
        let dual_vert_fn = Self::create_native_function(context, |this, _args, ctx| {
            let val = this
                .as_object()
                .and_then(|o| o.get(JsString::from("_is_dual_vertical"), ctx).ok())
                .and_then(|v| v.as_boolean())
                .unwrap_or(false);
            Ok(JsValue::from(val))
        });
        let fullscreen_fn = Self::create_native_function(context, |this, _args, ctx| {
            let val = this
                .as_object()
                .and_then(|o| o.get(JsString::from("_is_fullscreen"), ctx).ok())
                .and_then(|v| v.as_boolean())
                .unwrap_or(false);
            Ok(JsValue::from(val))
        });
        let four_fn = Self::create_native_function(context, |this, _args, ctx| {
            let val = this
                .as_object()
                .and_then(|o| o.get(JsString::from("_is_four_players"), ctx).ok())
                .and_then(|v| v.as_boolean())
                .unwrap_or(false);
            Ok(JsValue::from(val))
        });
        let count_fn = Self::create_native_function(context, |this, _args, ctx| {
            let val = this
                .as_object()
                .and_then(|o| o.get(JsString::from("_player_count"), ctx).ok())
                .and_then(|v| v.as_number())
                .unwrap_or(1.0) as i32;
            Ok(JsValue::from(val))
        });

        let _ = owner_obj.set(
            JsString::from("IsDualHorizontal"),
            dual_horiz_fn,
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("IsDualVertical"),
            dual_vert_fn,
            false,
            context,
        );
        let _ = owner_obj.set(
            JsString::from("IsFullscreen"),
            fullscreen_fn,
            false,
            context,
        );
        let _ = owner_obj.set(JsString::from("IsFourPlayers"), four_fn, false, context);
        let _ = owner_obj.set(JsString::from("GetPlayerCount"), count_fn, false, context);

        let _ = player_obj.set(
            JsString::from("Owner"),
            JsValue::from(owner_obj),
            false,
            context,
        );

        context
            .register_global_property(
                JsString::from("Player"),
                JsValue::from(player_obj),
                Attribute::all(),
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        Ok(())
    }

    /// Helper to extract byte vector from a JS array of numbers or a JS string.
    fn extract_bytes_from_arg(val: Option<&JsValue>, context: &mut Context) -> Vec<u8> {
        if let Some(v) = val {
            if let Some(s) = v.as_string() {
                return s.to_std_string_escaped().into_bytes();
            }
            if let Some(arr_obj) = v.as_object() {
                let len = arr_obj
                    .get(JsString::from("length"), context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as usize;
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    if let Ok(item) = arr_obj.get(i, context) {
                        if let Some(n) = item.as_number() {
                            bytes.push(n as u8);
                        }
                    }
                }
                return bytes;
            }
        }
        Vec::new()
    }
}
