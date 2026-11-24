//! Boa JavaScript engine wrapper for executing game handler scripts.

use crate::handler_config::HandlerConfig;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsString, JsValue, Source};
use nucleus_common::error::{NucleusError, NucleusResult};
use std::path::Path;

/// Wrapper around the Boa JavaScript execution engine for running game handlers.
pub struct ScriptEngine {
    /// Internal Boa JavaScript context.
    context: Context,
}

impl ScriptEngine {
    /// Creates a new `ScriptEngine` instance initialized with default runtime globals.
    pub fn new() -> NucleusResult<Self> {
        let context = Context::default();
        let mut engine = Self { context };
        engine.register_nucleus_namespace()?;
        crate::context_ext::register_reg_type(&mut engine.context)?;
        crate::bridge::JsBridge::register_globals(&mut engine.context)?;
        crate::proto_script::register(&mut engine.context)?;
        Ok(engine)
    }

    /// Injects a per-instance `Context` object into the JS runtime scope.
    pub fn inject_context(
        &mut self,
        ctx: &crate::handler_context::HandlerContext,
    ) -> NucleusResult<()> {
        crate::bridge::JsBridge::inject_context(&mut self.context, ctx)
    }

    /// Executes the `Game.Play()` lifecycle function for a player instance and reflects mutations back.
    pub fn execute_play(
        &mut self,
        ctx: &mut crate::handler_context::HandlerContext,
    ) -> NucleusResult<()> {
        self.inject_context(ctx)?;

        let _ = self.context.eval(Source::from_bytes(
            crate::proto_script::PER_PLAYER_PRELUDE,
        ));

        // The original Context mirrors every Game setting (Context.SymlinkExe, Context.Hook, ...).
        let _ = self.context.eval(Source::from_bytes(
            "(function(){ if (typeof Game !== 'object' || typeof Context !== 'object') return;              for (var k in Game) { if (!(k in Context) && typeof Game[k] !== 'function') Context[k] = Game[k]; } })();",
        ));

        let global = self.context.global_object();
        if let Ok(game_val) = global.get(JsString::from("Game"), &mut self.context) {
            if let Some(game_obj) = game_val.as_object() {
                if let Ok(play_val) = game_obj.get(JsString::from("Play"), &mut self.context) {
                    if let Some(callable) = play_val.as_callable() {
                        callable
                            .call(&JsValue::from(game_obj.clone()), &[], &mut self.context)
                            .map_err(|e| NucleusError::Script(e.to_string()))?;
                    }
                }
            }
        }

        // Reflect mutated properties from Context back into ctx
        if let Ok(ctx_val) = global.get(JsString::from("Context"), &mut self.context) {
            if let Some(ctx_obj) = ctx_val.as_object() {
                crate::context_ext::reflect_requests(&mut self.context, ctx_obj, ctx);
                if let Ok(v) = ctx_obj.get(JsString::from("StartArguments"), &mut self.context) {
                    if let Some(s) = v.as_string() {
                        ctx.start_arguments = s.to_std_string_escaped();
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("ExePath"), &mut self.context) {
                    if let Some(s) = v.as_string() {
                        ctx.exe_path = s.to_std_string_escaped();
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("SavePath"), &mut self.context) {
                    if let Some(s) = v.as_string() {
                        let path_str = s.to_std_string_escaped();
                        if !path_str.is_empty() {
                            ctx.save_path = Some(path_str);
                        }
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("DPIHandling"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        ctx.dpi_handling = (n as i32).into();
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("PosX"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        ctx.pos_x = n as i32;
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("PosY"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        ctx.pos_y = n as i32;
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("Width"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        ctx.width = n as i32;
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("Height"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        ctx.height = n as i32;
                    }
                }
                if let Ok(v) = ctx_obj.get(JsString::from("_ahkDirectives"), &mut self.context) {
                    if let Some(arr) = v.as_object() {
                        let len = arr
                            .get(JsString::from("length"), &mut self.context)
                            .ok()
                            .and_then(|n| n.as_number())
                            .unwrap_or(0.0) as usize;
                        let mut dirs = Vec::new();
                        for i in 0..len {
                            if let Ok(item) = arr.get(i, &mut self.context) {
                                if let Some(item_obj) = item.as_object() {
                                    let id = item_obj
                                        .get(JsString::from("id"), &mut self.context)
                                        .ok()
                                        .and_then(|x| {
                                            x.as_string().map(|s| s.to_std_string_escaped())
                                        })
                                        .unwrap_or_else(|| format!("dir_{i}"));
                                    let is_inline = item_obj
                                        .get(JsString::from("is_inline"), &mut self.context)
                                        .ok()
                                        .and_then(|x| x.as_boolean())
                                        .unwrap_or(false);
                                    let content = item_obj
                                        .get(JsString::from("content"), &mut self.context)
                                        .ok()
                                        .and_then(|x| {
                                            x.as_string().map(|s| s.to_std_string_escaped())
                                        })
                                        .unwrap_or_default();
                                    let version = item_obj
                                        .get(JsString::from("version"), &mut self.context)
                                        .ok()
                                        .and_then(|x| {
                                            x.as_string().map(|s| s.to_std_string_escaped())
                                        });
                                    let timing = item_obj
                                        .get(JsString::from("timing"), &mut self.context)
                                        .ok()
                                        .and_then(|x| {
                                            x.as_string().map(|s| s.to_std_string_escaped())
                                        });
                                    let mut args = Vec::new();
                                    if let Ok(args_val) =
                                        item_obj.get(JsString::from("args"), &mut self.context)
                                    {
                                        if let Some(args_arr) = args_val.as_object() {
                                            let a_len = args_arr
                                                .get(JsString::from("length"), &mut self.context)
                                                .ok()
                                                .and_then(|n| n.as_number())
                                                .unwrap_or(0.0)
                                                as usize;
                                            for j in 0..a_len {
                                                if let Ok(elem) = args_arr.get(j, &mut self.context)
                                                {
                                                    if let Some(s) = elem.as_string() {
                                                        args.push(s.to_std_string_escaped());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    dirs.push(crate::handler_context::AhkScriptDirective {
                                        id,
                                        is_inline,
                                        content,
                                        version,
                                        timing,
                                        args,
                                    });
                                }
                            }
                        }
                        if !dirs.is_empty() {
                            ctx.ahk_directives = dirs;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Invokes an optional parameterless `Game.<name>` callback (`OnStop`, `OnCustomHotKey_1`, ...).
    ///
    /// Returns `Ok(false)` when the handler does not define the callback.
    pub fn call_game_callback(&mut self, name: &str) -> NucleusResult<bool> {
        let global = self.context.global_object();
        let Ok(game_val) = global.get(JsString::from("Game"), &mut self.context) else {
            return Ok(false);
        };
        let Some(game_obj) = game_val.as_object().cloned() else {
            return Ok(false);
        };
        let Ok(cb) = game_obj.get(JsString::from(name), &mut self.context) else {
            return Ok(false);
        };
        let Some(callable) = cb.as_callable() else {
            return Ok(false);
        };
        callable
            .call(&JsValue::from(game_obj.clone()), &[], &mut self.context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        Ok(true)
    }

    /// Executes the `Game.Play()` lifecycle function with debugger instrumentation and breakpoints.
    pub fn execute_play_with_debugger(
        &mut self,
        file_id: &str,
        script_source: &str,
        handler_folder: Option<&Path>,
        ctx: &mut crate::handler_context::HandlerContext,
        controller: &nucleus_debug::DebugController,
    ) -> NucleusResult<()> {
        crate::debugger::register_debug_probes(&mut self.context, controller.clone())?;
        let instrumented = nucleus_debug::JsInstrumentor::instrument(file_id, script_source)
            .map_err(|e| NucleusError::Script(format!("Debugger instrumentation failed: {e}")))?;
        self.load_handler_script_with_folder(&instrumented.instrumented_source, handler_folder)?;
        self.execute_play(ctx)
    }

    /// Registers the Nucleus namespace and enums in global JS scope.
    fn register_nucleus_namespace(&mut self) -> NucleusResult<()> {
        let nucleus_obj = boa_engine::object::JsObject::default();

        // Register Folder enum
        let folder_obj = boa_engine::object::JsObject::default();
        folder_obj
            .set(
                JsString::from("Documents"),
                JsValue::from(0),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        folder_obj
            .set(
                JsString::from("MainGameFolder"),
                JsValue::from(1),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        folder_obj
            .set(
                JsString::from("InstancedGameFolder"),
                JsValue::from(2),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        nucleus_obj
            .set(
                JsString::from("Folder"),
                JsValue::from(folder_obj),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Register SaveType enum (matching C# SaveType: None=0, INI=1, CFG=2, SCR=3, RegKey=4)
        let save_type_obj = boa_engine::object::JsObject::default();
        save_type_obj
            .set(
                JsString::from("None"),
                JsValue::from(0),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        save_type_obj
            .set(
                JsString::from("INI"),
                JsValue::from(1),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        save_type_obj
            .set(
                JsString::from("CFG"),
                JsValue::from(2),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        save_type_obj
            .set(
                JsString::from("SCR"),
                JsValue::from(3),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        save_type_obj
            .set(
                JsString::from("RegKey"),
                JsValue::from(4),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        nucleus_obj
            .set(
                JsString::from("SaveType"),
                JsValue::from(save_type_obj),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Register DPIHandling enum (matching C# DPIHandling: True=0, Scaled=1, InvScaled=2)
        let dpi_obj = boa_engine::object::JsObject::default();
        dpi_obj
            .set(
                JsString::from("True"),
                JsValue::from(0),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        dpi_obj
            .set(
                JsString::from("None"),
                JsValue::from(0),
                false,
                &mut self.context,
            ) // backwards-compatible alias
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        dpi_obj
            .set(
                JsString::from("Scaled"),
                JsValue::from(1),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        dpi_obj
            .set(
                JsString::from("InvScaled"),
                JsValue::from(2),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        nucleus_obj
            .set(
                JsString::from("DPIHandling"),
                JsValue::from(dpi_obj),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Register SearchType enum (matching C# SearchType: Contains=0, Full=1, StartsWith=2, EndsWith=3)
        let search_type_obj = boa_engine::object::JsObject::default();
        search_type_obj
            .set(
                JsString::from("Contains"),
                JsValue::from(0),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        search_type_obj
            .set(
                JsString::from("Full"),
                JsValue::from(1),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        search_type_obj
            .set(
                JsString::from("Exact"),
                JsValue::from(1),
                false,
                &mut self.context,
            ) // alias
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        search_type_obj
            .set(
                JsString::from("StartsWith"),
                JsValue::from(2),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        search_type_obj
            .set(
                JsString::from("EndsWith"),
                JsValue::from(3),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        nucleus_obj
            .set(
                JsString::from("SearchType"),
                JsValue::from(search_type_obj),
                false,
                &mut self.context,
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Expose Nucleus object globally
        self.context
            .register_global_property(
                JsString::from("Nucleus"),
                JsValue::from(nucleus_obj),
                Attribute::all(),
            )
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        // Register importNamespace global function (returns Nucleus)
        let import_ns = crate::bridge::JsBridge::create_native_function(
            &mut self.context,
            |_this, _args, ctx| {
                let global = ctx.global_object();
                if let Ok(n_val) = global.get(JsString::from("Nucleus"), ctx) {
                    Ok(n_val)
                } else {
                    Ok(JsValue::undefined())
                }
            },
        );
        self.set_global("importNamespace", import_ns)?;

        Ok(())
    }

    /// Sets a global variable in the JavaScript execution context.
    pub fn set_global(&mut self, name: &str, value: JsValue) -> NucleusResult<()> {
        self.context
            .register_global_property(JsString::from(name), value, Attribute::all())
            .map_err(|e| NucleusError::Script(e.to_string()))?;
        Ok(())
    }

    /// Executes JavaScript source code in the context.
    pub fn execute(&mut self, source: &str) -> NucleusResult<JsValue> {
        let src = Source::from_bytes(source.as_bytes());
        self.context
            .eval(src)
            .map_err(|e| NucleusError::Script(e.to_string()))
    }

    /// Calls a global JavaScript function by name without arguments.
    pub fn call_function(&mut self, name: &str) -> NucleusResult<JsValue> {
        let global = self.context.global_object();
        let func = global
            .get(JsString::from(name), &mut self.context)
            .map_err(|e| NucleusError::Script(e.to_string()))?;

        if let Some(callable) = func.as_callable() {
            callable
                .call(&JsValue::undefined(), &[], &mut self.context)
                .map_err(|e| NucleusError::Script(e.to_string()))
        } else {
            Err(NucleusError::Script(format!(
                "'{}' is not a callable function",
                name
            )))
        }
    }

    /// Returns a mutable reference to the underlying Boa execution context.
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Initializes global `Game` and `Hub` objects, executes the handler script, and parses back the `HandlerConfig`.
    pub fn load_handler_script(&mut self, script: &str) -> NucleusResult<HandlerConfig> {
        self.load_handler_script_with_folder(script, None)
    }

    /// Initializes global `Game` and `Hub` objects with an optional handler directory path,
    /// executes the handler script, and parses back the `HandlerConfig`.
    pub fn load_handler_script_with_folder(
        &mut self,
        script: &str,
        handler_folder: Option<&Path>,
    ) -> NucleusResult<HandlerConfig> {
        // Register global Hub object expected by Community Hub scripts
        let hub_obj = boa_engine::object::JsObject::default();
        let hub_handler = boa_engine::object::JsObject::default();
        let _ = hub_handler.set(
            JsString::from("Version"),
            JsValue::from(-1),
            false,
            &mut self.context,
        );
        let _ = hub_handler.set(
            JsString::from("Id"),
            JsValue::from(JsString::from("")),
            false,
            &mut self.context,
        );
        let _ = hub_obj.set(
            JsString::from("Handler"),
            JsValue::from(hub_handler.clone()),
            false,
            &mut self.context,
        );

        let hub_maintainer = boa_engine::object::JsObject::default();
        let _ = hub_maintainer.set(
            JsString::from("Name"),
            JsValue::from(JsString::from("")),
            false,
            &mut self.context,
        );
        let _ = hub_maintainer.set(
            JsString::from("Id"),
            JsValue::from(JsString::from("")),
            false,
            &mut self.context,
        );
        let _ = hub_obj.set(
            JsString::from("Maintainer"),
            JsValue::from(hub_maintainer.clone()),
            false,
            &mut self.context,
        );

        let _ = hub_obj.set(
            JsString::from("Connected"),
            JsValue::from(true),
            false,
            &mut self.context,
        );

        let is_update_avail = crate::bridge::JsBridge::create_native_function(
            &mut self.context,
            |_this, _args, _ctx| Ok(JsValue::from(false)),
        );
        let _ = hub_obj.set(
            JsString::from("IsUpdateAvailable"),
            is_update_avail,
            false,
            &mut self.context,
        );
        self.set_global("Hub", JsValue::from(hub_obj.clone()))?;

        let game_obj = boa_engine::object::JsObject::default();

        // Pre-initialize sub-objects expected by standard handlers
        let hook_obj = boa_engine::object::JsObject::default();
        let _ = hook_obj.set(
            JsString::from("ForceFocus"),
            JsValue::from(true),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("ForceFocusWindowName"),
            JsValue::from(JsString::from("")),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("DInputEnabled"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("DInputForceDisable"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("XInputEnabled"),
            JsValue::from(true),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("XInputReroute"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("BlockKeyboardEvents"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("BlockMouseEvents"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("BlockInputEvents"),
            JsValue::from(false),
            false,
            &mut self.context,
        );
        let _ = hook_obj.set(
            JsString::from("CustomDllEnabled"),
            JsValue::from(false),
            false,
            &mut self.context,
        );

        let _ = game_obj.set(
            JsString::from("Hook"),
            JsValue::from(hook_obj.clone()),
            false,
            &mut self.context,
        );
        let proto_input_obj = boa_engine::object::JsObject::default();
        let _ = game_obj.set(
            JsString::from("ProtoInput"),
            JsValue::from(proto_input_obj),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("Options"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("ExecutableContext"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("DirSymlinkExclusions"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("DirExclusions"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("DirSymlinkCopyInstead"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("CopyFiles"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("FileSymlinkExclusions"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("FileSymlinkCopyInstead"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("KillMutex"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("XInputPlusDll"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("XInputNames"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("BackupFiles"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("BackupFolders"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("ExecutableNames"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("KillProcessesOnClose"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("CMDBatchClose"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("DeleteOnClose"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("DevReorderDlls"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("SDLPaths"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("X360ceDll"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("HexEditExe"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("HexEditDllFiles"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("CustomSteps"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("AutoHotkeyFiles"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("LudusaviPaths"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("LudusaviRegistry"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("RequiredPlugins"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("RequiredAddons"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );
        let _ = game_obj.set(
            JsString::from("InjectedDlls"),
            JsValue::from(boa_engine::object::builtins::JsArray::new(
                &mut self.context,
            )),
            false,
            &mut self.context,
        );

        let folder_str = handler_folder
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let _ = game_obj.set(
            JsString::from("Folder"),
            JsValue::from(JsString::from(folder_str.as_str())),
            false,
            &mut self.context,
        );

        // Native methods on Game object
        let add_option_fn = crate::bridge::JsBridge::create_native_function(
            &mut self.context,
            |this, args, ctx| {
                if let Some(this_obj) = this.as_object() {
                    if let Ok(opts_val) = this_obj.get(JsString::from("Options"), ctx) {
                        if let Some(opts_arr) = opts_val.as_object() {
                            let opt_obj = boa_engine::object::JsObject::default();
                            if let Some(name) = args.get(0).cloned() {
                                let _ = opt_obj.set(JsString::from("Name"), name, false, ctx);
                            }
                            if let Some(desc) = args.get(1).cloned() {
                                let _ =
                                    opt_obj.set(JsString::from("Description"), desc, false, ctx);
                            }
                            if let Some(key) = args.get(2).cloned() {
                                let _ = opt_obj.set(JsString::from("Key"), key, false, ctx);
                            }
                            if let Some(choices) = args.get(3).cloned() {
                                let _ = opt_obj.set(JsString::from("Values"), choices, false, ctx);
                            }
                            let _ = opt_obj.set(
                                JsString::from("Hidden"),
                                JsValue::from(false),
                                false,
                                ctx,
                            );
                            if let Ok(len_val) = opts_arr.get(JsString::from("length"), ctx) {
                                let len = len_val.as_number().unwrap_or(0.0) as u32;
                                let _ = opts_arr.set(len, JsValue::from(opt_obj), false, ctx);
                            }
                        }
                    }
                }
                Ok(JsValue::undefined())
            },
        );
        let _ = game_obj.set(
            JsString::from("AddOption"),
            add_option_fn,
            false,
            &mut self.context,
        );

        let add_injected_dll_fn = crate::bridge::JsBridge::create_native_function(
            &mut self.context,
            |this, args, ctx| {
                if let Some(this_obj) = this.as_object() {
                    if let Ok(dlls_val) = this_obj.get(JsString::from("InjectedDlls"), ctx) {
                        if let Some(dlls_arr) = dlls_val.as_object() {
                            let dll_obj = boa_engine::object::JsObject::default();
                            if let Some(path) = args.get(0).cloned() {
                                let _ = dll_obj.set(JsString::from("Path"), path, false, ctx);
                            }
                            if let Some(opts) = args.get(1).and_then(|v| v.as_object()) {
                                for prop in [
                                    "Timing",
                                    "DelayMs",
                                    "Required",
                                    "ExportName",
                                    "ExportArgument",
                                    "IpcChannel",
                                    "PipeName",
                                    "Architecture",
                                    "TargetInstances",
                                ] {
                                    if let Ok(val) = opts.get(JsString::from(prop), ctx) {
                                        if !val.is_undefined() {
                                            let _ = dll_obj.set(JsString::from(prop), val, false, ctx);
                                        }
                                    }
                                }
                            }
                            if let Ok(len_val) = dlls_arr.get(JsString::from("length"), ctx) {
                                let len = len_val.as_number().unwrap_or(0.0) as u32;
                                let _ = dlls_arr.set(len, JsValue::from(dll_obj), false, ctx);
                            }
                        }
                    }
                }
                Ok(JsValue::undefined())
            },
        );
        let _ = game_obj.set(
            JsString::from("AddInjectedDll"),
            add_injected_dll_fn,
            false,
            &mut self.context,
        );

        let show_option_fn = crate::bridge::JsBridge::create_native_function(
            &mut self.context,
            |this, args, ctx| {
                let opt_name_or_key = args
                    .get(0)
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                let required = args.get(1).and_then(|v| v.as_boolean()).unwrap_or(false);
                let update_player_info = args.get(2).and_then(|v| v.as_boolean()).unwrap_or(false);

                let mut matched_option = JsValue::undefined();
                let mut resolved_key = opt_name_or_key.clone();
                let mut resolved_title = opt_name_or_key.clone();

                if let Some(game) = this.as_object() {
                    if let Ok(opts_val) = game.get(JsString::from("Options"), ctx) {
                        if let Some(opts_arr) = opts_val.as_object() {
                            if let Ok(len_val) = opts_arr.get(JsString::from("length"), ctx) {
                                let len = len_val.as_number().unwrap_or(0.0) as u32;
                                for i in 0..len {
                                    if let Ok(opt_val) = opts_arr.get(i, ctx) {
                                        if let Some(opt_obj) = opt_val.as_object() {
                                            let name_match = opt_obj
                                                .get(JsString::from("Name"), ctx)
                                                .ok()
                                                .as_ref()
                                                .and_then(|v| v.as_string())
                                                .map(|s| {
                                                    s.to_std_string_escaped() == opt_name_or_key
                                                })
                                                .unwrap_or(false);
                                            let key_match = opt_obj
                                                .get(JsString::from("Key"), ctx)
                                                .ok()
                                                .as_ref()
                                                .and_then(|v| v.as_string())
                                                .map(|s| {
                                                    s.to_std_string_escaped() == opt_name_or_key
                                                })
                                                .unwrap_or(false);

                                            if name_match || key_match {
                                                let _ = opt_obj.set(
                                                    JsString::from("Hidden"),
                                                    JsValue::from(true),
                                                    false,
                                                    ctx,
                                                );
                                                matched_option = opt_val.clone();
                                                if let Ok(k) =
                                                    opt_obj.get(JsString::from("Key"), ctx)
                                                {
                                                    if let Some(ks) = k.as_string() {
                                                        resolved_key = ks.to_std_string_escaped();
                                                    }
                                                }
                                                if let Ok(n) =
                                                    opt_obj.get(JsString::from("Name"), ctx)
                                                {
                                                    if let Some(ns) = n.as_string() {
                                                        resolved_title = ns.to_std_string_escaped();
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let step_obj = boa_engine::object::JsObject::default();
                    let _ = step_obj.set(JsString::from("Option"), matched_option, false, ctx);
                    let _ = step_obj.set(
                        JsString::from("OptionKey"),
                        JsValue::from(JsString::from(resolved_key.clone())),
                        false,
                        ctx,
                    );
                    let _ = step_obj.set(
                        JsString::from("Required"),
                        JsValue::from(required),
                        false,
                        ctx,
                    );
                    let _ = step_obj.set(
                        JsString::from("Title"),
                        JsValue::from(JsString::from(resolved_title.clone())),
                        false,
                        ctx,
                    );
                    let _ = step_obj.set(
                        JsString::from("UpdatePlayerInfo"),
                        JsValue::from(update_player_info),
                        false,
                        ctx,
                    );

                    if let Ok(steps_val) = game.get(JsString::from("CustomSteps"), ctx) {
                        if let Some(steps_arr) = steps_val.as_object() {
                            if let Ok(len_val) = steps_arr.get(JsString::from("length"), ctx) {
                                let len = len_val.as_number().unwrap_or(0.0) as u32;
                                let _ =
                                    steps_arr.set(len, JsValue::from(step_obj.clone()), false, ctx);
                            }
                        }
                    }

                    Ok(JsValue::from(step_obj))
                } else {
                    Ok(JsValue::undefined())
                }
            },
        );
        let _ = game_obj.set(
            JsString::from("ShowOptionAsStep"),
            show_option_fn,
            false,
            &mut self.context,
        );

        self.set_global("Game", JsValue::from(game_obj.clone()))?;

        // Execute handler
        self.execute(script)?;

        // Extract properties from Game object
        let mut config = HandlerConfig::default();

        macro_rules! extract_string {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(s) = val.as_string() {
                        config.$field = s.to_std_string_escaped();
                    }
                }
            };
        }

        macro_rules! extract_bool {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(b) = val.as_boolean() {
                        config.$field = b;
                    }
                }
            };
        }

        macro_rules! extract_i32 {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(n) = val.as_number() {
                        config.$field = n as i32;
                    }
                }
            };
        }

        macro_rules! extract_f64 {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(n) = val.as_number() {
                        config.$field = n;
                    }
                }
            };
        }

        extract_string!(guid, "GUID");
        extract_string!(game_name, "GameName");
        extract_string!(steam_id, "SteamID");
        extract_string!(executable_name, "ExecutableName");
        extract_string!(start_arguments, "StartArguments");
        extract_string!(binaries_folder, "BinariesFolder");
        extract_string!(working_folder, "WorkingFolder");
        extract_string!(launcher_exe, "LauncherExe");
        extract_string!(launcher_title, "LauncherTitle");
        extract_string!(goldberg_language, "GoldbergLanguage");
        extract_string!(description, "Description");
        extract_string!(kill_mutex_type, "KillMutexType");
        extract_string!(save_path, "SavePath");
        extract_string!(game_data_dir, "GameDataDir");

        extract_bool!(symlink_exe, "SymlinkExe");
        extract_bool!(symlink_game, "SymlinkGame");
        extract_bool!(hardcopy_game, "HardcopyGame");
        extract_bool!(hardlink_game, "HardlinkGame");
        extract_bool!(keep_symlink_on_exit, "KeepSymLinkOnExit");
        extract_bool!(supports_keyboard, "SupportsKeyboard");
        extract_bool!(keyboard_player_first, "KeyboardPlayerFirst");
        extract_bool!(
            supports_multiple_keyboards_and_mice,
            "SupportsMultipleKeyboardsAndMice"
        );
        extract_bool!(hook_focus, "HookFocus");
        extract_bool!(fake_focus, "FakeFocus");
        extract_bool!(hide_taskbar, "HideTaskbar");
        extract_bool!(lock_mouse, "LockMouse");
        extract_bool!(use_goldberg, "UseGoldberg");
        extract_bool!(use_x360ce, "UseX360ce");
        extract_bool!(cmd_launch, "CMDLaunch");
        extract_bool!(use_force_bind_ip, "UseForceBindIP");
        extract_bool!(use_nucleus_environment, "UseNucleusEnvironment");
        extract_bool!(supports_positioning, "SupportsPositioning");
        extract_bool!(keep_aspect_ratio, "KeepAspectRatio");
        extract_bool!(keep_monitor_aspect_ratio, "KeepMonitorAspectRatio");
        extract_bool!(needs_steam_emulation, "NeedsSteamEmulation");
        extract_bool!(debug_mode, "Debug");
        extract_bool!(use_proto_input, "UseProtoInput");
        extract_bool!(symlink_folders, "SymlinkFolders");
        extract_bool!(user_profile_save_path_no_copy, "UserProfileSavePathNoCopy");

        extract_bool!(use_dev_reorder, "UseDevReorder");
        extract_bool!(use_sdl, "UseSDL");
        extract_bool!(use_xinput_plus, "UseXInputPlus");
        extract_bool!(use_xinput_index, "UseXInputIndex");
        extract_bool!(use_x360ce_on_steam, "UseX360ceOnSteam");
        extract_bool!(xbox_one_controller_fix, "XboxOneControllerFix");
        extract_bool!(goldberg_need_steam_interface, "GoldbergNeedSteamInterface");
        extract_bool!(
            goldberg_write_account_name,
            "GoldbergWriteSteamIDAndAccount"
        );
        if !config.goldberg_write_account_name {
            extract_bool!(goldberg_write_account_name, "GoldbergWriteAccountName");
        }
        extract_bool!(goldberg_lan_only, "GoldbergLanOnly");
        extract_bool!(goldberg_no_local_save, "GoldbergNoLocalSave");
        extract_bool!(create_steam_appid_by_exe, "CreateSteamAppIdByExe");
        extract_bool!(goldberg_experimental, "GoldbergExperimental");
        extract_bool!(goldberg_ignore_steam_appid, "GoldbergIgnoreSteamAppId");
        extract_bool!(process_changes_at_end, "ProcessChangesAtEnd");
        extract_bool!(create_single_device_file, "CreateSingleDeviceFile");
        extract_bool!(launch_as_admin, "LaunchAsAdmin");
        extract_bool!(resume_game_on_lost_focus, "ResumeGameOnLostFocus");
        extract_bool!(use_api_index, "UseApiIndex");
        extract_bool!(hide_desktop, "HideDesktop");
        extract_bool!(set_thread_affinity, "SetThreadAffinity");
        extract_bool!(forward_raw_keyboard_input, "ForwardRawKeyboardInput");
        extract_bool!(forward_raw_mouse_input, "ForwardRawMouseInput");
        extract_bool!(allow_fake_battrey, "AllowFakeBattrey");
        extract_bool!(use_input_emulation, "UseInputEmulation");
        extract_bool!(use_realnpad, "UseRealnpad");
        extract_bool!(not_top_most, "NotTopMost");
        extract_bool!(set_top_most_at_end, "SetTopMostAtEnd");

        extract_bool!(hide_cursor, "HideCursor");
        extract_bool!(dont_resize, "DontResize");
        extract_bool!(dont_reposition, "DontReposition");

        extract_bool!(partial_mutex_search_launcher, "PartialMutexSearchLauncher");
        extract_bool!(partial_mutex_search_process, "PartialMutexSearchProcess");
        extract_bool!(kill_last_instance_mutex, "KillLastInstanceMutex");
        extract_bool!(kill_mutex_at_end, "KillMutexAtEnd");
        extract_bool!(rename_not_kill_mutex, "RenameNotKillMutex");
        extract_bool!(partial_mutex_search, "PartialMutexSearch");

        extract_bool!(dont_remove_borders, "DontRemoveBorders");
        extract_bool!(ignore_window_border_check, "IgnoreWindowBorderCheck");
        extract_bool!(refresh_window_after_start, "RefreshWindowAfterStart");
        extract_bool!(reset_windows, "ResetWindows");
        extract_bool!(
            set_foreground_window_elsewhere,
            "SetForegroundWindowElsewhere"
        );
        extract_bool!(prevent_window_deactivation, "PreventWindowDeactivation");
        extract_bool!(
            keyboard_player_skip_prevent_window_deactivate,
            "KeyboardPlayerSkipPreventWindowDeactivate"
        );
        extract_bool!(force_window_title, "ForceWindowTitle");
        extract_bool!(has_dynamic_window_title, "HasDynamicWindowTitle");
        extract_bool!(id_in_window_title, "IdInWindowTitle");
        extract_bool!(enable_windows, "EnableWindows");

        extract_bool!(use_nemirtingas_epic_emu, "UseNemirtingasEpicEmu");
        extract_bool!(use_nemirtingas_galaxy_emu, "UseNemirtingasGalaxyEmu");
        extract_bool!(epic_emu_args, "EpicEmuArgs");
        extract_bool!(alt_epic_emu_args, "AltEpicEmuArgs");
        extract_bool!(
            flawless_widescreen_override_display,
            "FlawlessWidescreenOverrideDisplay"
        );
        extract_bool!(use_steamless, "UseSteamless");
        extract_bool!(use_steam_stub_drm_patcher, "UseSteamStubDRMPatcher");
        extract_bool!(use_eac_bypass, "UseEACBypass");
        extract_bool!(use_directx9_wrapper, "UseDirectX9Wrapper");
        extract_bool!(goldberg_no_warning, "GoldbergNoWarning");
        extract_bool!(
            goldberg_experimental_steam_client,
            "GoldbergExperimentalSteamClient"
        );
        extract_bool!(goldberg_experimental_rename, "GoldbergExperimentalRename");
        extract_bool!(goldberg_lobby_connect, "GoldbergLobbyConnect");
        extract_bool!(requires_admin, "RequiresAdmin");
        extract_bool!(use_peerparty, "UsePeerParty");
        extract_bool!(peerparty_public, "PeerPartyPublic");
        extract_bool!(peerparty_voice, "PeerPartyVoice");
        extract_bool!(peerparty_overlay, "PeerPartyOverlay");
        extract_bool!(peerparty_copy_saves, "PeerPartyCopySaves");

        extract_bool!(cmd_start_args_inside, "CMDStartArgsInside");
        extract_bool!(launcher_exe_ignore_file_check, "LauncherExeIgnoreFileCheck");
        extract_bool!(
            force_launcher_exe_ignore_file_check,
            "ForceLauncherExeIgnoreFileCheck"
        );
        extract_bool!(run_launcher_and_exe, "RunLauncherAndExe");
        extract_bool!(third_party_launch, "ThirdPartyLaunch");
        extract_bool!(ignore_third_party_prompt, "IgnoreThirdPartyPrompt");
        extract_bool!(force_process_pick, "ForceProcessPick");
        extract_bool!(force_process_search, "ForceProcessSearch");
        extract_bool!(prompt_before_process_grab, "PromptBeforeProcessGrab");
        extract_bool!(prompt_between_instances, "PromptBetweenInstances");
        extract_bool!(prompt_between_instances_end, "PromptBetweenInstancesEnd");
        extract_bool!(prompt_after_first_instance, "PromptAfterFirstInstance");
        extract_bool!(prompt_process_changes_at_end, "PromptProcessChangesAtEnd");
        extract_bool!(ignore_delete_files_prompt, "IgnoreDeleteFilesPrompt");
        extract_bool!(change_exe, "ChangeExe");

        extract_i32!(max_players, "MaxPlayers");
        extract_i32!(max_players_one_monitor, "MaxPlayersOneMonitor");
        extract_i32!(pause_between_starts, "PauseBetweenStarts");
        extract_i32!(kill_mutex_delay, "KillMutexDelay");
        extract_i32!(thread_affinity, "ThreadAffinity");
        extract_i32!(fake_focus_interval, "FakeFocusInterval");
        extract_i32!(players_per_instance, "PlayersPerInstance");
        extract_i32!(kill_mutex_delay_launcher, "KillMutexDelayLauncher");
        extract_i32!(kill_mutex_delay_process, "KillMutexDelayProcess");
        extract_i32!(pause_before_mutex_killing, "PauseBeforeMutexKilling");
        extract_i32!(steamless_timing, "SteamlessTiming");
        extract_i32!(lock_input_toggle_key, "LockInputToggleKey");
        extract_i32!(pause_cmd_batch_before, "PauseCMDBatchBefore");
        extract_i32!(pause_cmd_batch_after, "PauseCMDBatchAfter");
        extract_i32!(pause_between_process_grab, "PauseBetweenProcessGrab");
        extract_i32!(
            pause_between_context_and_launch,
            "PauseBetweenContextAndLaunch"
        );
        extract_i32!(ideal_processor, "IdealProcessor");

        extract_f64!(handler_interval, "HandlerInterval");

        macro_rules! extract_opt_string {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(s) = val.as_string() {
                        let text = s.to_std_string_escaped();
                        if !text.is_empty() {
                            config.$field = Some(text);
                        }
                    }
                }
            };
        }

        extract_opt_string!(user_profile_config_path, "UserProfileConfigPath");
        extract_opt_string!(user_profile_save_path, "UserProfileSavePath");
        extract_opt_string!(documents_config_path, "DocumentsConfigPath");
        extract_opt_string!(documents_save_path, "DocumentsSavePath");
        extract_opt_string!(handler_folder, "Folder");
        if config.handler_folder.is_none() && !folder_str.is_empty() {
            config.handler_folder = Some(folder_str);
        }

        extract_string!(kill_mutex_type_launcher, "KillMutexTypeLauncher");
        extract_string!(kill_mutex_type_process, "KillMutexTypeProcess");
        extract_opt_string!(mutex_process_exe, "MutexProcessExe");
        extract_opt_string!(flawless_widescreen, "FlawlessWidescreen");
        extract_opt_string!(
            flawless_widescreen_plugin_path,
            "FlawlessWidescreenPluginPath"
        );
        extract_opt_string!(steamless_args, "SteamlessArgs");
        extract_opt_string!(steam_stub_drm_patcher_arch, "SteamStubDRMPatcherArch");
        extract_opt_string!(executable_to_launch, "ExecutableToLaunch");
        extract_opt_string!(force_game_arch, "ForceGameArch");
        extract_opt_string!(write_to_process_memory, "WriteToProcessMemory");
        extract_opt_string!(use_processor, "UseProcessor");
        extract_opt_string!(processor_priority_class, "ProcessorPriorityClass");
        extract_opt_string!(peerparty_room, "PeerPartyRoom");

        // Hook sub-object properties
        if let Ok(hook_val) = game_obj.get(JsString::from("Hook"), &mut self.context) {
            if let Some(h) = hook_val.as_object() {
                if let Ok(ff) = h.get(JsString::from("ForceFocus"), &mut self.context) {
                    if let Some(b) = ff.as_boolean() {
                        config.hook_focus = b;
                    }
                }
                if let Ok(xi) = h.get(JsString::from("XInputEnabled"), &mut self.context) {
                    if let Some(b) = xi.as_boolean() {
                        config.hook_xinput_enabled = b;
                    }
                }
                if let Ok(di) = h.get(JsString::from("DInputEnabled"), &mut self.context) {
                    if let Some(b) = di.as_boolean() {
                        config.hook_dinput_enabled = b;
                    }
                }
                if let Ok(xr) = h.get(JsString::from("XInputReroute"), &mut self.context) {
                    if let Some(b) = xr.as_boolean() {
                        config.hook_xinput_reroute = b;
                    }
                }
                if let Ok(sdl) = h.get(JsString::from("SDL2Enabled"), &mut self.context) {
                    if let Some(b) = sdl.as_boolean() {
                        config.hook_sdl2_enabled = b;
                    }
                }
                if let Ok(cde) = h.get(JsString::from("CustomDllEnabled"), &mut self.context) {
                    if let Some(b) = cde.as_boolean() {
                        config.hook_custom_dll_enabled = b;
                    }
                }
            }
        }
        if let Ok(cde) = game_obj.get(JsString::from("CustomDllEnabled"), &mut self.context) {
            if let Some(b) = cde.as_boolean() {
                config.hook_custom_dll_enabled = b;
            }
        }

        // ProtoInput sub-object properties
        if let Ok(pi_val) = game_obj.get(JsString::from("ProtoInput"), &mut self.context) {
            if let Some(pi) = pi_val.as_object() {
                if let Ok(xi) = pi.get(JsString::from("XinputHook"), &mut self.context) {
                    if let Some(b) = xi.as_boolean() {
                        config.proto_input_xinput_hook = b;
                    }
                }
                if let Ok(di) = pi.get(JsString::from("DinputDeviceHook"), &mut self.context) {
                    if let Some(b) = di.as_boolean() {
                        config.proto_input_dinput_hook = b;
                    }
                }
                if let Ok(di2) = pi.get(JsString::from("DinputHook"), &mut self.context) {
                    if let Some(b) = di2.as_boolean() {
                        config.proto_input_dinput_hook = config.proto_input_dinput_hook || b;
                    }
                }
            }
        }

        // Compute exact input support flags based on C# parity
        config.supports_xinput = (config.hook_xinput_enabled
            && !config.hook_xinput_reroute
            && !config.proto_input_dinput_hook)
            || config.proto_input_xinput_hook
            || config.hook_sdl2_enabled
            || config.use_x360ce
            || config.use_xinput_plus;

        config.supports_dinput = config.hook_dinput_enabled
            || config.hook_xinput_reroute
            || config.proto_input_dinput_hook
            || config.hook_sdl2_enabled
            || config.use_dev_reorder;

        config.supports_raw_input =
            config.supports_multiple_keyboards_and_mice || config.use_proto_input;

        // Helper macro for string arrays
        macro_rules! extract_string_array {
            ($field:ident, $js_key:expr) => {
                if let Ok(val) = game_obj.get(JsString::from($js_key), &mut self.context) {
                    if let Some(obj) = val.as_object() {
                        if obj.is_array() {
                            let mut list = Vec::new();
                            let len = obj
                                .get(JsString::from("length"), &mut self.context)
                                .ok()
                                .and_then(|v| v.as_number())
                                .unwrap_or(0.0) as usize;
                            for i in 0..len {
                                if let Ok(item) = obj.get(i, &mut self.context) {
                                    if let Some(s) = item.as_string() {
                                        list.push(s.to_std_string_escaped());
                                    }
                                }
                            }
                            if !list.is_empty() {
                                config.$field = Some(list);
                            }
                        }
                    }
                }
            };
        }

        extract_string_array!(kill_mutex, "KillMutex");
        extract_string_array!(executable_context, "ExecutableContext");
        extract_string_array!(executable_names, "ExecutableNames");
        extract_string_array!(cmd_options, "CMDOptions");
        extract_string_array!(kill_processes_on_close, "KillProcessesOnClose");
        extract_string_array!(backup_files, "BackupFiles");
        extract_string_array!(backup_folders, "BackupFolders");
        extract_string_array!(kill_mutex_launcher, "KillMutexLauncher");
        extract_string_array!(kill_mutex_process, "KillMutexProcess");
        extract_string_array!(window_style_values, "WindowStyleValues");
        extract_string_array!(ext_window_style_values, "ExtWindowStyleValues");
        extract_string_array!(window_style_end_changes, "WindowStyleEndChanges");
        extract_string_array!(ext_window_style_end_changes, "ExtWindowStyleEndChanges");
        extract_string_array!(steamless_patch, "SteamlessPatch");
        extract_string_array!(cmd_batch_before, "CMDBatchBefore");
        extract_string_array!(cmd_batch_after, "CMDBatchAfter");
        extract_string_array!(use_processors_per_instance, "UseProcessorsPerInstance");
        extract_string_array!(dir_symlink_exclusions, "DirSymlinkExclusions");
        extract_string_array!(dir_exclusions, "DirExclusions");
        extract_string_array!(dir_symlink_copy_instead, "DirSymlinkCopyInstead");
        extract_bool!(
            dir_symlink_copy_instead_include_sub_folders,
            "DirSymlinkCopyInsteadIncludeSubFolders"
        );
        extract_string_array!(copy_files, "CopyFiles");
        extract_string_array!(file_symlink_exclusions, "FileSymlinkExclusions");
        extract_string_array!(file_symlink_copy_instead, "FileSymlinkCopyInstead");
        extract_string_array!(dev_reorder_dlls, "DevReorderDlls");
        extract_string_array!(sdl_paths, "SDLPaths");
        extract_string_array!(xinput_plus_dll, "XInputPlusDll");
        extract_string_array!(x360ce_dll, "X360ceDll");
        extract_string_array!(hex_edit_exe, "HexEditExe");
        extract_string_array!(hex_edit_dll_files, "HexEditDllFiles");
        extract_string_array!(custom_steam_api_dll_path, "CustomSteamApiDllPath");
        extract_string_array!(cmd_batch_close, "CMDBatchClose");
        extract_string_array!(delete_on_close, "DeleteOnClose");

        // AutoHotkey extraction
        extract_bool!(use_autohotkey, "UseAutoHotkey");
        if let Ok(val) = game_obj.get(JsString::from("AutoHotkeyVersion"), &mut self.context) {
            if let Some(s) = val.as_string() {
                let v = s.to_std_string_escaped().to_lowercase();
                if v.contains("v1") || v == "1" {
                    config.autohotkey_version = nucleus_tools::autohotkey::AhkVersion::V1;
                } else if v.contains("v2") || v == "2" {
                    config.autohotkey_version = nucleus_tools::autohotkey::AhkVersion::V2;
                } else {
                    config.autohotkey_version = nucleus_tools::autohotkey::AhkVersion::Auto;
                }
            }
        }
        extract_string_array!(autohotkey_files, "AutoHotkeyFiles");
        extract_opt_string!(autohotkey_code, "AutoHotkeyCode");
        if let Ok(val) = game_obj.get(JsString::from("AutoHotkeyTiming"), &mut self.context) {
            if let Some(s) = val.as_string() {
                let t = s.to_std_string_escaped().to_lowercase();
                if t.contains("immediate") {
                    config.autohotkey_timing = nucleus_tools::autohotkey::AhkTiming::Immediate;
                } else if t.contains("pre") || t.contains("before") {
                    config.autohotkey_timing = nucleus_tools::autohotkey::AhkTiming::PreLaunch;
                } else {
                    config.autohotkey_timing = nucleus_tools::autohotkey::AhkTiming::PostWindow;
                }
            }
        }

        // Ludusavi extraction
        extract_opt_string!(ludusavi_title, "LudusaviTitle");
        extract_string_array!(ludusavi_paths, "LudusaviPaths");
        extract_string_array!(ludusavi_registry, "LudusaviRegistry");
        extract_bool!(ludusavi_disabled, "LudusaviDisabled");

        // Extensions & Plugins extraction
        extract_string_array!(required_plugins, "RequiredPlugins");
        extract_string_array!(required_addons, "RequiredAddons");
        extract_string_array!(peerparty_services, "PeerPartyServices");

        if config.use_peerparty {
            let mut pp_dll = nucleus_common::dll_injection::DllInjectionConfig::new("utils/PeerParty/x64/peerparty_hook.dll");
            pp_dll.timing = nucleus_common::dll_injection::InjectionTiming::AtStartupSuspended;
            pp_dll.method = nucleus_common::dll_injection::InjectionMethod::QueueUserApc;
            pp_dll.architecture = nucleus_common::dll_injection::TargetArchitecture::Auto;
            pp_dll.required = true;
            if !config.injected_dlls.iter().any(|d| d.path.contains("peerparty_hook")) {
                config.injected_dlls.push(pp_dll);
            }
        }

        // DesktopScale float array
        if let Ok(val) = game_obj.get(JsString::from("DesktopScale"), &mut self.context) {
            if let Some(obj) = val.as_object() {
                if obj.is_array() {
                    let mut list = Vec::new();
                    let len = obj
                        .get(JsString::from("length"), &mut self.context)
                        .ok()
                        .and_then(|v| v.as_number())
                        .unwrap_or(0.0) as usize;
                    for i in 0..len {
                        if let Ok(item) = obj.get(i, &mut self.context) {
                            if let Some(n) = item.as_number() {
                                list.push(n);
                            }
                        }
                    }
                    if !list.is_empty() {
                        config.desktop_scale = Some(list);
                    }
                }
            }
        }

        // Extract Hub metadata from Hub object
        if let Ok(h_val) = hub_obj.get(JsString::from("Handler"), &mut self.context) {
            if let Some(h) = h_val.as_object() {
                if let Ok(v) = h.get(JsString::from("Version"), &mut self.context) {
                    if let Some(n) = v.as_number() {
                        let ver = n as i32;
                        if ver >= 0 {
                            config.hub_handler_version = Some(ver);
                        }
                    }
                }
                if let Ok(id) = h.get(JsString::from("Id"), &mut self.context) {
                    if let Some(s) = id.as_string() {
                        let id_str = s.to_std_string_escaped();
                        if !id_str.is_empty() {
                            config.hub_handler_id = Some(id_str);
                        }
                    }
                }
            }
        }

        if let Ok(m_val) = hub_obj.get(JsString::from("Maintainer"), &mut self.context) {
            if let Some(m) = m_val.as_object() {
                if let Ok(name) = m.get(JsString::from("Name"), &mut self.context) {
                    if let Some(s) = name.as_string() {
                        let name_str = s.to_std_string_escaped();
                        if !name_str.is_empty() {
                            config.hub_maintainer_name = Some(name_str);
                        }
                    }
                }
                if let Ok(id) = m.get(JsString::from("Id"), &mut self.context) {
                    if let Some(s) = id.as_string() {
                        let id_str = s.to_std_string_escaped();
                        if !id_str.is_empty() {
                            config.hub_maintainer_id = Some(id_str);
                        }
                    }
                }
            }
        }

        // Extract Game.Options
        if let Ok(opts_val) = game_obj.get(JsString::from("Options"), &mut self.context) {
            if let Some(opts_arr) = opts_val.as_object() {
                let len = opts_arr
                    .get(JsString::from("length"), &mut self.context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;
                for i in 0..len {
                    if let Ok(opt_val) = opts_arr.get(i, &mut self.context) {
                        if let Some(opt_obj) = opt_val.as_object() {
                            let mut option = crate::handler_config::HandlerOption::default();
                            if let Ok(v) = opt_obj.get(JsString::from("Name"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    option.name = s.to_std_string_escaped();
                                }
                            }
                            if let Ok(v) =
                                opt_obj.get(JsString::from("Description"), &mut self.context)
                            {
                                if let Some(s) = v.as_string() {
                                    option.description = s.to_std_string_escaped();
                                }
                            }
                            if let Ok(v) = opt_obj.get(JsString::from("Key"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    option.key = s.to_std_string_escaped();
                                }
                            }
                            if let Ok(v) = opt_obj.get(JsString::from("Values"), &mut self.context)
                            {
                                if let Some(arr) = v.as_object() {
                                    let v_len = arr
                                        .get(JsString::from("length"), &mut self.context)
                                        .ok()
                                        .and_then(|n| n.as_number())
                                        .unwrap_or(0.0)
                                        as u32;
                                    for j in 0..v_len {
                                        if let Ok(item) = arr.get(j, &mut self.context) {
                                            if let Some(s) = item.as_string() {
                                                option.values.push(s.to_std_string_escaped());
                                            }
                                        }
                                    }
                                }
                            }
                            if let Ok(v) = opt_obj.get(JsString::from("Hidden"), &mut self.context)
                            {
                                if let Some(b) = v.as_boolean() {
                                    option.hidden = b;
                                }
                            }
                            config.options.push(option);
                        }
                    }
                }
            }
        }

        // Extract Game.CustomSteps
        if let Ok(steps_val) = game_obj.get(JsString::from("CustomSteps"), &mut self.context) {
            if let Some(steps_arr) = steps_val.as_object() {
                let len = steps_arr
                    .get(JsString::from("length"), &mut self.context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;
                for i in 0..len {
                    if let Ok(step_val) = steps_arr.get(i, &mut self.context) {
                        if let Some(step_obj) = step_val.as_object() {
                            let mut step = crate::handler_config::CustomStep::default();
                            if let Ok(v) =
                                step_obj.get(JsString::from("OptionKey"), &mut self.context)
                            {
                                if let Some(s) = v.as_string() {
                                    step.option_key = s.to_std_string_escaped();
                                }
                            }
                            if let Ok(v) = step_obj.get(JsString::from("Title"), &mut self.context)
                            {
                                if let Some(s) = v.as_string() {
                                    step.title = s.to_std_string_escaped();
                                }
                            }
                            if let Ok(v) =
                                step_obj.get(JsString::from("Required"), &mut self.context)
                            {
                                if let Some(b) = v.as_boolean() {
                                    step.required = b;
                                }
                            }
                            config.custom_steps.push(step);
                        }
                    }
                }
            }
        }

        // Extract Game.InjectedDlls
        if let Ok(dlls_val) = game_obj.get(JsString::from("InjectedDlls"), &mut self.context) {
            if let Some(dlls_arr) = dlls_val.as_object() {
                let len = dlls_arr
                    .get(JsString::from("length"), &mut self.context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u32;

                for i in 0..len {
                    if let Ok(item_val) = dlls_arr.get(i, &mut self.context) {
                        if let Some(item_obj) = item_val.as_object() {
                            let path = if let Ok(v) = item_obj.get(JsString::from("Path"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    s.to_std_string_escaped()
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            if path.is_empty() {
                                continue;
                            }

                            let mut dll_config = nucleus_common::dll_injection::DllInjectionConfig::new(path);

                            if let Ok(v) = item_obj.get(JsString::from("Timing"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    let t_str = s.to_std_string_escaped().to_lowercase();
                                    if t_str.contains("startupsuspended") || t_str.contains("suspended") {
                                        dll_config.timing = nucleus_common::dll_injection::InjectionTiming::AtStartupSuspended;
                                    } else if t_str.contains("window") {
                                        dll_config.timing = nucleus_common::dll_injection::InjectionTiming::OnWindowCreated;
                                    } else if t_str.contains("delay") {
                                        dll_config.timing = nucleus_common::dll_injection::InjectionTiming::AfterDelayMs(dll_config.delay_ms);
                                    } else if t_str.contains("manual") {
                                        dll_config.timing = nucleus_common::dll_injection::InjectionTiming::Manual;
                                    } else {
                                        dll_config.timing = nucleus_common::dll_injection::InjectionTiming::PostStartup;
                                    }
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("DelayMs"), &mut self.context) {
                                if let Some(n) = v.as_number() {
                                    dll_config.delay_ms = n.max(0.0) as u64;
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("Required"), &mut self.context) {
                                if let Some(b) = v.as_boolean() {
                                    dll_config.required = b;
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("ExportName"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    let export_str = s.to_std_string_escaped();
                                    if !export_str.is_empty() {
                                        dll_config.export_name = Some(export_str);
                                    }
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("ExportArgument"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    dll_config.export_argument = Some(s.to_std_string_escaped());
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("IpcChannel"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    let ipc_str = s.to_std_string_escaped().to_lowercase();
                                    if ipc_str.contains("pipe") {
                                        dll_config.ipc_channel = nucleus_common::dll_injection::IpcChannelKind::NamedPipe;
                                    } else if ipc_str.contains("shared") || ipc_str.contains("shm") {
                                        dll_config.ipc_channel = nucleus_common::dll_injection::IpcChannelKind::SharedMemory;
                                    } else {
                                        dll_config.ipc_channel = nucleus_common::dll_injection::IpcChannelKind::None;
                                    }
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("PipeName"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    let p = s.to_std_string_escaped();
                                    if !p.is_empty() {
                                        dll_config.pipe_name = Some(p);
                                    }
                                }
                            }

                            if let Ok(v) = item_obj.get(JsString::from("Architecture"), &mut self.context) {
                                if let Some(s) = v.as_string() {
                                    let arch_str = s.to_std_string_escaped().to_lowercase();
                                    if arch_str.contains("64") {
                                        dll_config.architecture = nucleus_common::dll_injection::TargetArchitecture::X64;
                                    } else if arch_str.contains("86") || arch_str.contains("32") {
                                        dll_config.architecture = nucleus_common::dll_injection::TargetArchitecture::X86;
                                    } else {
                                        dll_config.architecture = nucleus_common::dll_injection::TargetArchitecture::Auto;
                                    }
                                }
                            }

                            config.injected_dlls.push(dll_config);
                        }
                    }
                }
            }
        }

        // Ensure GUID is never empty if GameName or Hub ID is present
        if config.guid.is_empty() {
            if !config.game_name.is_empty() {
                config.guid = config.game_name.clone();
            } else if let Some(ref hid) = config.hub_handler_id {
                config.guid = hid.clone();
            }
        }

        config.legacy = crate::legacy_props::LegacyGameProps::extract(&game_obj, &mut self.context);
        config.proto_input = crate::legacy_props::extract_proto_input(&game_obj, &mut self.context);
        config.use_proto_input = config.use_proto_input
            || crate::legacy_props::proto_input_requested(&config.proto_input);

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_basic_eval() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let result = engine.execute("1 + 2").expect("Failed to eval");
        assert_eq!(result.as_number(), Some(3.0));
    }

    #[test]
    fn test_script_engine_handler_loading() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let handler_src = r#"
            Game.GameName = "Portal 2";
            Game.GUID = "portal-2-split";
            Game.SteamID = "620";
            Game.MaxPlayers = 2;
            Game.ExecutableName = "portal2.exe";
            Game.SymlinkGame = true;
            Game.UseGoldberg = true;
            Game.KillMutex = ["hl2_singleton_mutex", "SteamClientMutex"];
        "#;

        let config = engine
            .load_handler_script(handler_src)
            .expect("Failed to load handler");
        assert_eq!(config.game_name, "Portal 2");
        assert_eq!(config.guid, "portal-2-split");
        assert_eq!(config.steam_id, "620");
        assert_eq!(config.max_players, 2);
        assert_eq!(config.executable_name, "portal2.exe");
        assert!(config.symlink_game);
        assert!(config.use_goldberg);
        assert_eq!(
            config.kill_mutex,
            Some(vec![
                "hl2_singleton_mutex".to_string(),
                "SteamClientMutex".to_string()
            ])
        );
    }

    #[test]
    fn test_script_engine_nucleus_namespace() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let result = engine
            .execute("Nucleus.Folder.Documents")
            .expect("Failed to access Folder");
        assert_eq!(result.as_number(), Some(0.0));

        let save_type = engine
            .execute("Nucleus.SaveType.INI")
            .expect("Failed to access SaveType");
        assert_eq!(save_type.as_number(), Some(1.0));
    }

    #[test]
    fn test_script_engine_game_play_execution() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_play_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cfg_path = temp_dir.join("game.cfg");
        let ini_path = temp_dir.join("config.ini");

        std::fs::write(&cfg_path, "ScreenWidth = 800\r\nScreenHeight = 600\r\n").unwrap();
        std::fs::write(&ini_path, "[DISPLAY]\r\nResolution=800x600\r\n").unwrap();

        let cfg_str = cfg_path.to_str().unwrap().replace('\\', "/");
        let ini_str = ini_path.to_str().unwrap().replace('\\', "/");

        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let handler_src = format!(
            r#"
            Game.GameName = "TestGame";
            Game.Play = function() {{
                var cfg = "{cfg_str}";
                var ini = "{ini_str}";

                var line = Context.FindLineNumberInTextFile(cfg, "ScreenWidth =", Nucleus.SearchType.StartsWith);
                Context.ReplaceLinesInTextFile(cfg, [
                    line + "|ScreenWidth = " + Context.Width,
                    (line + 1) + "|ScreenHeight = " + Context.Height
                ]);

                Context.ModifySaveFile(ini, ini, Nucleus.SaveType.INI, [
                    new Nucleus.IniSaveInfo("DISPLAY", "Resolution", Context.Width + "x" + Context.Height)
                ]);
            }};
        "#
        );

        let _ = engine
            .load_handler_script(&handler_src)
            .expect("Failed to load script");

        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        ctx.width = 1920;
        ctx.height = 1080;

        engine
            .execute_play(&mut ctx)
            .expect("Failed to execute Game.Play");

        let updated_cfg = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(updated_cfg.contains("ScreenWidth = 1920"));
        assert!(updated_cfg.contains("ScreenHeight = 1080"));

        let updated_ini = std::fs::read_to_string(&ini_path).unwrap();
        assert!(updated_ini.contains("Resolution=1920x1080"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_script_engine_xml_and_file_ops() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_script_xml_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let xml_path = temp_dir.join("settings.xml");
        std::fs::write(
            &xml_path,
            r#"<config><resolution width="800" height="600" /></config>"#,
        )
        .unwrap();

        let xml_str = xml_path.to_str().unwrap().replace('\\', "/");

        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let handler_src = format!(
            r#"
            Game.GameName = "XmlGame";
            Game.Play = function() {{
                var xml = "{xml_str}";
                Context.ChangeXmlAttributeValue(xml, "//resolution", "width", Context.Width);
                Context.ChangeXmlAttributeValue(xml, "//resolution", "height", Context.Height);
            }};
        "#
        );

        let _ = engine
            .load_handler_script(&handler_src)
            .expect("Failed to load script");

        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        ctx.width = 2560;
        ctx.height = 1440;

        engine
            .execute_play(&mut ctx)
            .expect("Failed to execute Game.Play");

        let updated_xml = std::fs::read_to_string(&xml_path).unwrap();
        assert!(updated_xml.contains("width=\"2560\""));
        assert!(updated_xml.contains("height=\"1440\""));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_script_engine_load_master_handler() {
        let master_handler_path = std::path::Path::new("../../handlers/MasterHandler.js");
        if master_handler_path.exists() {
            let script = std::fs::read_to_string(master_handler_path)
                .expect("Failed to read MasterHandler.js");
            let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
            let config = engine
                .load_handler_script(&script)
                .expect("Failed to evaluate MasterHandler.js");

            assert_eq!(config.game_name, "Game Name");
            assert_eq!(config.executable_name, "Game1.exe");
            assert_eq!(config.guid, "My Game");
            assert_eq!(config.max_players, 4);
            assert_eq!(config.steam_id, "12345");
            assert_eq!(config.kill_mutex, Some(vec!["F1 2012".to_string()]));
            assert_eq!(config.dir_symlink_exclusions, Some(vec!["bin".to_string()]));
            assert_eq!(
                config.file_symlink_exclusions,
                Some(vec![
                    "steam_api.dll".to_string(),
                    "steam_interfaces.txt".to_string()
                ])
            );
            assert_eq!(
                config.file_symlink_copy_instead,
                Some(vec!["bink.dll".to_string()])
            );
            assert!(config.hook_focus);
        }
    }

    #[test]
    fn test_script_engine_community_handler_with_hub_and_options() {
        let script = r#"
            Hub.Handler.Version = 17;
            Hub.Handler.Id = "wFsXDjoAkHpL4XZm8";
            Hub.Maintainer.Name = "Talos91";
            Hub.Maintainer.Id = "eeL7HAz8zJovChWw4";

            var Nucleus = importNamespace('Nucleus.Gaming');

            var answers1 = ["No", "Yes"];
            Game.AddOption("Native Splitscreen", "Use native splitscreen", "Native", answers1);

            Game.ExecutableName = "left4dead2.exe";
            Game.SteamID = "550";
            Game.GameName = "Left 4 Dead 2";
            Game.MaxPlayers = 8;
            Game.KillMutex = ["hl2_singleton_mutex"];
            Game.DPIHandling = Nucleus.DPIHandling.InvScaled;
        "#;

        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let config = engine
            .load_handler_script(script)
            .expect("Failed to parse community handler");

        assert_eq!(config.game_name, "Left 4 Dead 2");
        assert_eq!(config.guid, "Left 4 Dead 2");
        assert_eq!(config.steam_id, "550");
        assert_eq!(config.max_players, 8);
        assert_eq!(config.hub_handler_version, Some(17));
        assert_eq!(config.hub_handler_id.as_deref(), Some("wFsXDjoAkHpL4XZm8"));
        assert_eq!(config.hub_maintainer_name.as_deref(), Some("Talos91"));
        assert_eq!(config.options.len(), 1);
        assert_eq!(config.options[0].name, "Native Splitscreen");
        assert_eq!(
            config.options[0].values,
            vec!["No".to_string(), "Yes".to_string()]
        );
    }

    #[test]
    fn test_script_engine_community_script_play_options_and_owner() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            Game.GameName = "Left 4 Dead 2";
            Game.Play = function() {
                var native = Context.Options["Native"];
                var scale = Context.Options["Scale"];
                var hooks = Context.Options["Hooks"];

                if (scale == "InvScaled") {
                    Context.DPIHandling = Nucleus.DPIHandling.InvScaled;
                }

                if (Player.Owner.IsDualHorizontal()) {
                    Context.Log("Horizontal mode active");
                }
                if (Player.Owner.IsDualVertical()) {
                    Context.Log("Vertical mode active");
                }
                var pCount = Player.Owner.GetPlayerCount();
                Context.Log("Player count: " + pCount);
                Context.Log("NucleusFolder: " + Context.NucleusFolder);
                Context.Log("LocalIP: " + Context.LocalIP);
            };
        "#;

        engine
            .load_handler_script(script)
            .expect("Failed to load script");

        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        ctx.number_of_players = 2;
        ctx.layout = "1x2 (Stacked)".to_string();
        ctx.options.insert("Native".to_string(), "No".to_string());
        ctx.options
            .insert("Scale".to_string(), "InvScaled".to_string());
        ctx.options.insert("Hooks".to_string(), "No".to_string());

        engine
            .execute_play(&mut ctx)
            .expect("execute_play should succeed without TypeError");
    }

    #[test]
    fn test_script_engine_real_l4d2_handler_if_present() {
        let l4d2_path = std::path::Path::new("C:/Games/NucleusCoop242/handlers/Left 4 Dead 2.js");
        if l4d2_path.exists() {
            let script = std::fs::read_to_string(l4d2_path).unwrap();
            let mut engine = ScriptEngine::new().unwrap();
            let _ = engine.load_handler_script(&script).unwrap();

            let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
            ctx.number_of_players = 2;
            ctx.layout = "1x2 (Stacked)".to_string();
            ctx.is_keyboard_player = true;
            ctx.root_folder = std::env::temp_dir().to_string_lossy().to_string();
            ctx.root_install_folder = std::env::temp_dir().to_string_lossy().to_string();
            ctx.instance_folder = std::env::temp_dir().to_string_lossy().to_string();

            // Run Game.Play
            let res = engine.execute_play(&mut ctx);
            assert!(
                res.is_ok(),
                "Real Left 4 Dead 2 execute_play failed: {:?}",
                res.err()
            );
        }
    }

    #[test]
    fn test_handler_properties_parity() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            Game.GameName = "Parity Test Game";
            Game.SymlinkFolders = true;
            Game.CopyFiles = ["copy1.dll", "copy2.bin"];
            Game.DirExclusions = ["exclude_dir1", "exclude_dir2"];
            Game.DirSymlinkCopyInstead = ["dir_copy1"];
            Game.UserProfileConfigPath = "AppData\\Local\\ParityGame";
            Game.UserProfileSavePath = "Saved Games\\ParityGame";
            Game.UserProfileSavePathNoCopy = true;
            Game.DocumentsConfigPath = "My Games\\ParityGame\\Config";
            Game.DocumentsSavePath = "My Games\\ParityGame\\Saves";
        "#;

        let folder = std::path::Path::new("C:/Games/Handlers/ParityGame");
        let config = engine
            .load_handler_script_with_folder(script, Some(folder))
            .expect("Failed to load script");

        assert!(config.symlink_folders);
        assert_eq!(
            config.copy_files,
            Some(vec!["copy1.dll".to_string(), "copy2.bin".to_string()])
        );
        assert_eq!(
            config.dir_exclusions,
            Some(vec!["exclude_dir1".to_string(), "exclude_dir2".to_string()])
        );
        assert_eq!(
            config.dir_symlink_copy_instead,
            Some(vec!["dir_copy1".to_string()])
        );
        assert_eq!(
            config.user_profile_config_path.as_deref(),
            Some("AppData\\Local\\ParityGame")
        );
        assert_eq!(
            config.user_profile_save_path.as_deref(),
            Some("Saved Games\\ParityGame")
        );
        assert!(config.user_profile_save_path_no_copy);
        assert_eq!(
            config.documents_config_path.as_deref(),
            Some("My Games\\ParityGame\\Config")
        );
        assert_eq!(
            config.documents_save_path.as_deref(),
            Some("My Games\\ParityGame\\Saves")
        );
        assert_eq!(
            config.handler_folder.as_deref(),
            Some("C:/Games/Handlers/ParityGame")
        );
    }

    #[test]
    fn test_execute_play_reflects_context_mutations() {
        let mut engine = ScriptEngine::new().unwrap();
        let script = r#"
            Game.Play = function() {
                Context.StartArguments = Context.StartArguments + " -windowed -noborder";
                Context.SavePath = "CustomSaveDir";
                Context.DPIHandling = Nucleus.DPIHandling.InvScaled;
            };
        "#;
        engine.load_handler_script(script).unwrap();

        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        ctx.start_arguments = "-novid".to_string();

        engine.execute_play(&mut ctx).unwrap();

        assert_eq!(ctx.start_arguments, "-novid -windowed -noborder");
        assert_eq!(ctx.save_path.as_deref(), Some("CustomSaveDir"));
        assert_eq!(
            ctx.dpi_handling,
            crate::handler_context::DPIHandling::InvScaled
        );
    }

    #[test]
    fn test_enum_inversions_and_js_namespaces() {
        use crate::enums::{CustomStepType, DPIHandling, Folder, RegType, SaveType, SearchType};

        // Test Rust enum discriminant parity matching legacy C# NucleusCoop
        assert_eq!(SearchType::Contains as i32, 0);
        assert_eq!(SearchType::Full as i32, 1);
        assert_eq!(SearchType::StartsWith as i32, 2);
        assert_eq!(SearchType::EndsWith as i32, 3);

        assert_eq!(DPIHandling::True as i32, 0);
        assert_eq!(DPIHandling::Scaled as i32, 1);
        assert_eq!(DPIHandling::InvScaled as i32, 2);

        assert_eq!(SaveType::None as i32, 0);
        assert_eq!(SaveType::INI as i32, 1);
        assert_eq!(SaveType::CFG as i32, 2);
        assert_eq!(SaveType::SCR as i32, 3);
        assert_eq!(SaveType::RegKey as i32, 4);

        assert_eq!(Folder::Documents as i32, 0);
        assert_eq!(Folder::MainGameFolder as i32, 1);
        assert_eq!(Folder::InstancedGameFolder as i32, 2);

        assert_eq!(CustomStepType::OptionStep as i32, 0);
        assert_eq!(CustomStepType::TextEntry as i32, 1);
        assert_eq!(CustomStepType::PlayerNumber as i32, 2);

        assert_eq!(RegType::DWord as i32, 4);
        assert_eq!(RegType::String as i32, 1);

        // Test JS Nucleus namespace values in Boa engine
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            var ok = true;
            if (Nucleus.SearchType.Contains !== 0) ok = false;
            if (Nucleus.SearchType.Full !== 1) ok = false;
            if (Nucleus.SearchType.StartsWith !== 2) ok = false;
            if (Nucleus.SearchType.EndsWith !== 3) ok = false;

            if (Nucleus.DPIHandling.True !== 0) ok = false;
            if (Nucleus.DPIHandling.Scaled !== 1) ok = false;
            if (Nucleus.DPIHandling.InvScaled !== 2) ok = false;

            if (Nucleus.SaveType.None !== 0) ok = false;
            if (Nucleus.SaveType.INI !== 1) ok = false;
            if (Nucleus.SaveType.CFG !== 2) ok = false;
            if (Nucleus.SaveType.SCR !== 3) ok = false;
            if (Nucleus.SaveType.RegKey !== 4) ok = false;

            Game.GameName = ok ? "EnumParityPassed" : "EnumParityFailed";
        "#;
        let config = engine
            .load_handler_script(script)
            .expect("Failed to load script");
        assert_eq!(config.game_name, "EnumParityPassed");
    }

    #[test]
    fn test_all_extracted_handler_properties() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            Game.ExecutableNames = ["game1.exe", "game2.exe"];
            Game.CMDOptions = ["-option1", "-option2"];
            Game.FakeFocusInterval = 250;
            Game.HideCursor = true;
            Game.KillProcessesOnClose = ["launcher.exe", "crashhandler.exe"];
            Game.BackupFiles = ["config.ini"];
            Game.BackupFolders = ["save_data"];
            Game.DontResize = true;
            Game.DontReposition = true;
            Game.PlayersPerInstance = 2;
            Game.GoldbergWriteSteamIDAndAccount = true;
            Game.KillMutexLauncher = ["LauncherMutex"];
            Game.KillMutexProcess = ["ProcessMutex"];
            Game.KillMutexDelay = 1500;
            Game.KillMutexTypeLauncher = "MutantLauncher";
            Game.KillMutexTypeProcess = "MutantProcess";
            Game.WindowStyleValues = ["0x10000000", "0x00080000"];
            Game.ExtWindowStyleValues = ["0x00000008"];
            Game.LockInputToggleKey = 0x23;
            Game.RequiresAdmin = true;
            Game.UseNemirtingasEpicEmu = true;
            Game.UseNemirtingasGalaxyEmu = true;
        "#;
        let config = engine
            .load_handler_script(script)
            .expect("Failed to load script");

        assert_eq!(
            config.executable_names,
            Some(vec!["game1.exe".to_string(), "game2.exe".to_string()])
        );
        assert_eq!(
            config.cmd_options,
            Some(vec!["-option1".to_string(), "-option2".to_string()])
        );
        assert_eq!(config.fake_focus_interval, 250);
        assert!(config.hide_cursor);
        assert_eq!(
            config.kill_processes_on_close,
            Some(vec![
                "launcher.exe".to_string(),
                "crashhandler.exe".to_string()
            ])
        );
        assert_eq!(config.backup_files, Some(vec!["config.ini".to_string()]));
        assert_eq!(config.backup_folders, Some(vec!["save_data".to_string()]));
        assert!(config.dont_resize);
        assert!(config.dont_reposition);
        assert_eq!(config.players_per_instance, 2);
        assert!(config.goldberg_write_account_name);
        assert_eq!(
            config.kill_mutex_launcher,
            Some(vec!["LauncherMutex".to_string()])
        );
        assert_eq!(
            config.kill_mutex_process,
            Some(vec!["ProcessMutex".to_string()])
        );
        assert_eq!(config.kill_mutex_delay, 1500);
        assert_eq!(config.kill_mutex_type_launcher, "MutantLauncher");
        assert_eq!(config.kill_mutex_type_process, "MutantProcess");
        assert_eq!(
            config.window_style_values,
            Some(vec!["0x10000000".to_string(), "0x00080000".to_string()])
        );
        assert_eq!(
            config.ext_window_style_values,
            Some(vec!["0x00000008".to_string()])
        );
        assert_eq!(config.lock_input_toggle_key, 0x23);
        assert!(config.requires_admin);
        assert!(config.use_nemirtingas_epic_emu);
        assert!(config.use_nemirtingas_galaxy_emu);
    }

    #[test]
    fn test_game_show_option_as_step() {
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            Game.AddOption("Select Team", "Choose your team", "team", ["Red", "Blue"]);
            var step = Game.ShowOptionAsStep("Select Team", false, false);
        "#;
        let config = engine
            .load_handler_script(script)
            .expect("Failed to load script");

        assert_eq!(config.options.len(), 1);
        assert_eq!(config.options[0].name, "Select Team");
        assert_eq!(config.options[0].key, "team");
        assert!(config.options[0].hidden);

        assert_eq!(config.custom_steps.len(), 1);
        assert_eq!(config.custom_steps[0].title, "Select Team");
        assert_eq!(config.custom_steps[0].option_key, "team");
        assert!(!config.custom_steps[0].required);
    }

    #[test]
    fn test_context_runtime_parity_all_methods() {
        let temp_dir = std::env::temp_dir().join(format!(
            "nucleus_script_parity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Context.ProcessID
        let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
        let script = r#"
            Game.Play = function() {
                Context.StartArguments = "PID=" + Context.ProcessID;
            };
        "#;
        engine.load_handler_script(script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        ctx.process_id = Some(9876);
        engine.execute_play(&mut ctx).unwrap();
        assert_eq!(ctx.start_arguments, "PID=9876");

        // 2. Context.RemoveLineInTextFile with SearchType
        let lines_file = temp_dir.join("lines.txt");
        let initial_lines = "Keep1\r\nDropContainsFoo\r\nExactFullMatch\r\nStartsWithPrefixBar\r\nBazEndsWithSuffix\r\nKeep2\r\n";
        std::fs::write(&lines_file, initial_lines).unwrap();
        let lines_file_str = lines_file.to_str().unwrap().replace('\\', "/");

        let script = format!(
            r#"
            Game.Play = function() {{
                var f = "{lines_file_str}";
                Context.RemoveLineInTextFile(f, "ContainsFoo", Nucleus.SearchType.Contains);
                Context.RemoveLineInTextFile(f, "ExactFullMatch", Nucleus.SearchType.Full);
                Context.RemoveLineInTextFile(f, "StartsWithPrefix", Nucleus.SearchType.StartsWith);
                Context.RemoveLineInTextFile(f, "EndsWithSuffix", Nucleus.SearchType.EndsWith);
            }};
        "#
        );
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(&script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        let updated_lines = std::fs::read_to_string(&lines_file).unwrap();
        assert!(updated_lines.contains("Keep1"));
        assert!(updated_lines.contains("Keep2"));
        assert!(!updated_lines.contains("ContainsFoo"));
        assert!(!updated_lines.contains("ExactFullMatch"));
        assert!(!updated_lines.contains("StartsWithPrefix"));
        assert!(!updated_lines.contains("EndsWithSuffix"));

        // 3. Context.ModifySaveFile for CFG (Source engine key "value") with overwrite guard
        let src_cfg = temp_dir.join("source.cfg");
        let dest_cfg = temp_dir.join("dest.cfg");
        std::fs::write(&src_cfg, "name \"Default\"\r\ncl_fov \"90\"\r\n").unwrap();

        let src_cfg_str = src_cfg.to_str().unwrap().replace('\\', "/");
        let dest_cfg_str = dest_cfg.to_str().unwrap().replace('\\', "/");

        let script = format!(
            r#"
            Game.Play = function() {{
                var src = "{src_cfg_str}";
                var dst = "{dest_cfg_str}";
                Context.ModifySaveFile(src, dst, Nucleus.SaveType.CFG, [
                    {{ Key: "name", Value: "Player0" }},
                    {{ Key: "cl_fov", Value: "110" }},
                    {{ Key: "new_setting", Value: "1" }}
                ]);
            }};
        "#
        );
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(&script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        let dest_cfg_content = std::fs::read_to_string(&dest_cfg).unwrap();
        assert!(dest_cfg_content.contains("name \"Player0\""));
        assert!(dest_cfg_content.contains("cl_fov \"110\""));
        assert!(dest_cfg_content.contains("new_setting \"1\""));

        // 4. Context.WriteTextFile (variadic and array)
        let write_arr_file = temp_dir.join("arr.txt");
        let write_var_file = temp_dir.join("var.txt");
        let write_arr_str = write_arr_file.to_str().unwrap().replace('\\', "/");
        let write_var_str = write_var_file.to_str().unwrap().replace('\\', "/");

        let script = format!(
            r#"
            Game.Play = function() {{
                Context.WriteTextFile("{write_arr_str}", ["line1", "line2", 42]);
                Context.WriteTextFile("{write_var_str}", "first", "second", 99);
            }};
        "#
        );
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(&script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        let arr_content = std::fs::read_to_string(&write_arr_file).unwrap();
        assert!(arr_content.contains("line1"));
        assert!(arr_content.contains("line2"));
        assert!(arr_content.contains("42"));

        let var_content = std::fs::read_to_string(&write_var_file).unwrap();
        assert!(var_content.contains("first"));
        assert!(var_content.contains("second"));
        assert!(var_content.contains("99"));

        // 5. Context.PatchFile string overload
        let patch_src = temp_dir.join("patch_src.bin");
        let patch_dst = temp_dir.join("patch_dst.bin");
        std::fs::write(&patch_src, b"Hello World Foo Bar").unwrap();
        let patch_src_str = patch_src.to_str().unwrap().replace('\\', "/");
        let patch_dst_str = patch_dst.to_str().unwrap().replace('\\', "/");

        let script = format!(
            r#"
            Game.Play = function() {{
                Context.PatchFile("{patch_src_str}", "{patch_dst_str}", "World", "Earth");
            }};
        "#
        );
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(&script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        let patch_dst_content = std::fs::read_to_string(&patch_dst).unwrap();
        assert_eq!(patch_dst_content, "Hello Earth Foo Bar");

        // 6. Context.PatchFileFindPattern pattern insertion overload
        let pattern_src = temp_dir.join("pattern_src.bin");
        let pattern_dst = temp_dir.join("pattern_dst.bin");
        std::fs::write(&pattern_src, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE]).unwrap();
        let pattern_src_str = pattern_src.to_str().unwrap().replace('\\', "/");
        let pattern_dst_str = pattern_dst.to_str().unwrap().replace('\\', "/");

        let script = format!(
            r#"
            Game.Play = function() {{
                Context.PatchFileFindPattern("{pattern_src_str}", "{pattern_dst_str}", "BB CC", 0x2211, 1, false);
            }};
        "#
        );
        let mut engine = ScriptEngine::new().unwrap();
        engine.load_handler_script(&script).unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        let pattern_dst_content = std::fs::read(&pattern_dst).unwrap();
        assert_eq!(pattern_dst_content, vec![0xAA, 0x11, 0x22, 0xDD, 0xEE]);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ahk_ludusavi_plugin_properties_and_directives() {
        let script = r#"
            Game.UseAutoHotkey = true;
            Game.AutoHotkeyVersion = "v2";
            Game.AutoHotkeyFiles = ["scripts/key_remap.ahk", "scripts/focus.ahk"];
            Game.AutoHotkeyCode = "WinSetTitle, A,, CustomTitle";
            Game.AutoHotkeyTiming = "Immediate";

            Game.LudusaviTitle = "Portal 2";
            Game.LudusaviPaths = ["SAVE/portal2", "CONFIG/config.cfg"];
            Game.LudusaviRegistry = ["HKEY_CURRENT_USER\\Software\\Valve\\Portal2"];
            Game.LudusaviDisabled = false;

            Game.RequiredPlugins = ["wasm_custom_hook", "audio_splitter"];
            Game.RequiredAddons = ["dark_mode_addon"];

            Game.Play = function() {
                Context.ImportAhkFile("scripts/window_pin.ahk", { version: "v2", timing: "PostWindow", args: ["--pin", "1"] });
                Context.RunAhkCode("ToolTip, Player Launched, 10, 10", { timing: "PostWindow" });
            };
        "#;

        let mut engine = ScriptEngine::new().unwrap();
        let config = engine.load_handler_script(script).unwrap();

        assert!(config.use_autohotkey);
        assert_eq!(
            config.autohotkey_version,
            nucleus_tools::autohotkey::AhkVersion::V2
        );
        assert_eq!(config.autohotkey_files.as_ref().unwrap().len(), 2);
        assert_eq!(
            config.autohotkey_files.as_ref().unwrap()[0],
            "scripts/key_remap.ahk"
        );
        assert_eq!(
            config.autohotkey_code.as_deref(),
            Some("WinSetTitle, A,, CustomTitle")
        );
        assert_eq!(
            config.autohotkey_timing,
            nucleus_tools::autohotkey::AhkTiming::Immediate
        );

        assert_eq!(config.ludusavi_title.as_deref(), Some("Portal 2"));
        assert_eq!(config.ludusavi_paths.as_ref().unwrap().len(), 2);
        assert_eq!(config.ludusavi_registry.as_ref().unwrap().len(), 1);
        assert!(!config.ludusavi_disabled);

        assert_eq!(config.required_plugins.as_ref().unwrap().len(), 2);
        assert_eq!(config.required_addons.as_ref().unwrap().len(), 1);

        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine.execute_play(&mut ctx).unwrap();

        assert_eq!(ctx.ahk_directives.len(), 2);
        assert!(!ctx.ahk_directives[0].is_inline);
        assert_eq!(ctx.ahk_directives[0].content, "scripts/window_pin.ahk");
        assert_eq!(ctx.ahk_directives[0].version.as_deref(), Some("v2"));
        assert_eq!(ctx.ahk_directives[0].timing.as_deref(), Some("PostWindow"));
        assert_eq!(ctx.ahk_directives[0].args, vec!["--pin", "1"]);

        assert!(ctx.ahk_directives[1].is_inline);
        assert_eq!(
            ctx.ahk_directives[1].content,
            "ToolTip, Player Launched, 10, 10"
        );
        assert_eq!(ctx.ahk_directives[1].timing.as_deref(), Some("PostWindow"));
    }

    #[test]
    fn test_execute_play_with_debugger() {
        let script = r#"
            Game.Play = function() {
                var x = 10;
                var y = 20;
                Context.StartArguments = "-test -w " + (x + y);
            };
        "#;

        let controller = nucleus_debug::controller::DebugController::new(
            nucleus_debug::protocol::DebugLaunchMode::DryRun,
            None,
        );

        let mut engine = ScriptEngine::new().unwrap();
        let mut ctx = crate::handler_context::HandlerContext::new(0, "Player1".to_string());
        engine
            .execute_play_with_debugger("test_handler.js", script, None, &mut ctx, &controller)
            .unwrap();

        assert_eq!(ctx.start_arguments, "-test -w 30");
    }
}
