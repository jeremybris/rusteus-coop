//! End-to-end checks that handler scripts written against the original `Game` / `Context`
//! API evaluate and behave the same through the Boa bridge.

use nucleus_hook::proto_input::{DrawFakeCursor, SetWindowPosHook};
use nucleus_script::{HandlerContext, ScriptEngine};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nucleus_ctx_parity_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn js_path(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

#[test]
fn legacy_game_properties_are_extracted() {
    let mut engine = ScriptEngine::new().unwrap();
    let config = engine
        .load_handler_script(
            r#"
            Game.GUID = "legacy";
            Game.GameName = "Legacy";
            Game.ExecutableName = "legacy.exe";
            Game.SymlinkFiles = ["data\\settings.ini"];
            Game.CopyFoldersTo = ["cfg|cfg_copy"];
            Game.DeleteFiles = ["2|intro.bik"];
            Game.RenameAndOrMoveFiles = ["a.dll|b.dll"];
            Game.HexEditAllExes = ["steam_appid|nucle_appid"];
            Game.HexEditExeAddress = ["1|1A2B|9090"];
            Game.CopyCustomUtils = ["fix.dll|bin"];
            Game.UseHandlerSteamIds = true;
            Game.PlayerSteamIDs = ["76561198000000001", "76561198000000002"];
            Game.XInputPlusNoIni = true;
            Game.UseDInputBlocker = true;
            Game.SendNormalMouseInput = false;
            Game.HookFocusInstances = "1,2";
            Game.OnStop = function() {};
            Game.ProtoInput.InjectStartup = true;
            Game.ProtoInput.RegisterRawInputHook = true;
            Game.ProtoInput.GetRawInputDataHook = true;
            Game.ProtoInput.MessageFilterHook = true;
            Game.ProtoInput.DrawFakeCursor = true;
            Game.ProtoInput.SetWindowPosHook = "DontResize";
            Game.ProtoInput.FocusLoopIntervalMilliseconds = 25;
            Game.ProtoInput.BlockedMessages = [0x0008, 28];
            Game.ProtoInput.RenameHandles = ["GameMutex"];
            Game.ProtoInput.WindowActvateAppFilter = true;
            Game.ProtoInput.MultipleProtoControllers = true;
        "#,
        )
        .unwrap();

    let l = &config.legacy;
    assert_eq!(l.symlink_files, vec!["data\\settings.ini".to_string()]);
    assert_eq!(l.copy_folders_to, vec!["cfg|cfg_copy".to_string()]);
    assert_eq!(l.delete_files, vec!["2|intro.bik".to_string()]);
    assert_eq!(l.rename_and_or_move_files, vec!["a.dll|b.dll".to_string()]);
    assert_eq!(l.hex_edit_all_exes.len(), 1);
    assert_eq!(l.hex_edit_exe_address, vec!["1|1A2B|9090".to_string()]);
    assert_eq!(l.copy_custom_utils, vec!["fix.dll|bin".to_string()]);
    assert!(l.use_handler_steam_ids);
    assert_eq!(l.player_steam_ids.len(), 2);
    assert!(l.xinput_plus_no_ini);
    assert!(l.use_dinput_blocker);
    assert!(!l.send_normal_mouse_input);
    assert!(l.send_normal_keyboard_input, "untouched defaults stay true");
    assert_eq!(l.hook_focus_instances, "1,2");
    assert!(l.has_on_stop);
    assert!(!l.has_on_custom_hotkey_1);

    let p = &config.proto_input;
    assert!(p.inject_startup);
    assert!(p.register_raw_input_hook && p.get_raw_input_data_hook && p.message_filter_hook);
    assert_eq!(p.draw_fake_cursor, DrawFakeCursor::True);
    assert_eq!(p.set_window_pos_hook, SetWindowPosHook::DontResize);
    assert_eq!(p.focus_loop_interval_milliseconds, 25);
    assert_eq!(p.blocked_messages, vec![8, 28]);
    assert_eq!(p.rename_handles, vec!["GameMutex".to_string()]);
    assert!(p.window_activate_app_filter);
    assert!(p.multiple_proto_controllers);
    assert!(!p.clip_cursor_hook);
    assert!(config.use_proto_input);
}

#[test]
fn context_file_api_matches_original_behaviour() {
    let dir = temp_dir("files");
    let game = dir.join("game");
    let inst = dir.join("inst");
    let script_folder = dir.join("handlers").join("MyHandler");
    std::fs::create_dir_all(game.join("data")).unwrap();
    std::fs::create_dir_all(&inst).unwrap();
    std::fs::create_dir_all(script_folder.join("extra")).unwrap();
    std::fs::write(game.join("data").join("settings.ini"), "orig").unwrap();
    std::fs::write(script_folder.join("extra").join("patch.cfg"), "patched").unwrap();
    std::fs::write(
        inst.join("config.txt"),
        "res=1920\r\nfull=1\r\nname=abc\r\n",
    )
    .unwrap();
    std::fs::write(inst.join("game.bin"), [0u8; 16]).unwrap();
    std::fs::write(inst.join("save1.dat"), "s").unwrap();
    std::fs::create_dir_all(inst.join("Profile_Alpha")).unwrap();

    let mut ctx = HandlerContext::new(1, "Player2".to_string());
    ctx.root_folder = inst.to_string_lossy().to_string();
    ctx.orig_root_folder = game.to_string_lossy().to_string();
    ctx.root_install_folder = game.to_string_lossy().to_string();
    ctx.script_folder = Some(script_folder.to_string_lossy().to_string());
    ctx.nucleus_folder = Some(dir.to_string_lossy().to_string());
    ctx.symlink_files = vec!["data\\settings.ini".to_string()];
    ctx.arch = "x64".to_string();
    ctx.player_steam_id = Some("76561198000000002".to_string());
    ctx.player_steam_ids = vec![
        "76561198000000001".to_string(),
        "76561198000000002".to_string(),
    ];
    ctx.has_keyboard_player = true;
    ctx.num_controllers = 1;
    ctx.num_keyboards = 1;
    ctx.gamepad_guid = Some("12345678-0000-0000-0000-000000000000".to_string());

    let script = format!(
        r#"
        Game.GUID = "ctx";
        Game.GameName = "Ctx";
        Game.ExecutableName = "ctx.exe";
        Game.SymlinkExe = true;
        var out = {{}};
        Game.Play = function() {{
            var root = "{inst}";
            out.mirrored = Context.SymlinkExe === true && Context.GameName === "Ctx";
            out.arch = Context.Arch;
            out.steam = Context.PlayerSteamID + "/" + Context.PlayerSteamIDs.length;
            out.kb = Context.HasKeyboardPlayer && Context.NumControllers === 1 && Context.NumKeyboards === 1;
            out.x360 = Context.x360ceGamepadGuid;
            out.paths = [Context.NucleusUserRoot, Context.DocumentsPlayer, Context.EnvironmentPlayer,
                         Context.DocumentsRoot, Context.EnvironmentRoot, Context.UtilFolder,
                         Context.HandlersFolder, Context.UserName].every(function(p) {{ return typeof p === "string"; }});
            out.docPlayer = Context.DocumentsPlayer;
            out.upper = Context.ToUpperCase("abc") + Context.ToLowerCase("DEF") + Context.ConvertToString(42);
            out.rand = Context.RandomString(8, true);
            out.fileName = Context.GetFileName("C:\\games\\x\\game.exe");
            out.regType = Nucleus.RegType.DWord + "," + Nucleus.RegType.String + "," + Nucleus.RegType.QWord;

            out.symlinked = Context.ProceedSymlink();
            out.copied = Context.CopyScriptFolder(root + "\\fromscript");
            out.copyFolder = Context.CopyFolder(root + "\\fromscript", root + "\\fromscript2");
            out.found = Context.FindFiles(root, "*.dat").length;
            out.foundAll = Context.FindFiles(root, "*.cfg", true).length;
            out.partial = Context.FindFilePartialName(root, ["patch"]).length;
            out.sub = Context.SearchSubFolder(root, "Profile_");
            out.subs = Context.SearchSubFolders(root, "Profile_").length;
            out.noSub = Context.SearchSubFolder(root, "Missing_");
            Context.ReplacePartialLinesInTextFile(root + "\\config.txt", ["1|\\d+|960"]);
            Context.EditTextFile(root + "\\config.txt", ["full=", "name="], ["full=0", "Delete"], "utf-8");
            Context.HexEdit(root + "\\game.bin", "4", [0xDE, 0xAD]);
            Context.BackupFile([root + "\\save1.dat"], true);
            Context.MoveFolder("Profile_Alpha", "Profile_Beta");
            Context.ModifiedDate(root + "\\save1.dat", 2020, 1, 2, 3, 4, 5);
            var info = Context.NewSaveInfo("Video", "Width", "960");
            out.info = info.Section + info.Key + info.Value;
            Context.SetProtoInputMultipleControllers(2, 3, 0, 0);
            Context.HideTaskBar();
            Context.HideDesktop(true);
            Context.KeepEditedRegKeys = true;
            Context.Wait(1);
            Context.StartArguments = "-windowed";
        }};
        Game.OnStop = function() {{ out.stopped = true; }};
        "#,
        inst = js_path(&inst)
    );

    let mut engine = ScriptEngine::new().unwrap();
    engine.load_handler_script(&script).unwrap();
    engine.execute_play(&mut ctx).unwrap();

    let read = |engine: &mut ScriptEngine, expr: &str| -> String {
        let v = engine.execute(&format!("String({expr})")).unwrap();
        v.as_string().unwrap().to_std_string_escaped()
    };
    assert_eq!(read(&mut engine, "out.mirrored"), "true");
    assert_eq!(read(&mut engine, "out.arch"), "x64");
    assert_eq!(read(&mut engine, "out.steam"), "76561198000000002/2");
    assert_eq!(read(&mut engine, "out.kb"), "true");
    assert_eq!(
        read(&mut engine, "out.x360"),
        "IG_12345678000000000000000000000000"
    );
    assert_eq!(read(&mut engine, "out.paths"), "true");
    assert!(read(&mut engine, "out.docPlayer").contains("Player2"));
    assert_eq!(read(&mut engine, "out.upper"), "ABCdef42");
    let rand = read(&mut engine, "out.rand");
    assert_eq!(rand.len(), 8);
    assert!(rand.chars().all(|c| c.is_ascii_lowercase()));
    assert_eq!(read(&mut engine, "out.fileName"), "game.exe");
    assert_eq!(read(&mut engine, "out.regType"), "4,1,11");
    assert_eq!(read(&mut engine, "out.found"), "1");
    assert_eq!(read(&mut engine, "out.foundAll"), "2");
    assert!(read(&mut engine, "out.partial").parse::<usize>().unwrap() >= 2);
    assert!(read(&mut engine, "out.sub").ends_with("Profile_Alpha"));
    assert_eq!(read(&mut engine, "out.subs"), "1");
    assert_eq!(read(&mut engine, "out.noSub"), "null");
    assert_eq!(read(&mut engine, "out.info"), "VideoWidth960");

    // Files on disk.
    assert_eq!(
        std::fs::read_to_string(inst.join("data").join("settings.ini")).unwrap(),
        "orig"
    );
    assert_eq!(
        std::fs::read_to_string(inst.join("fromscript").join("extra").join("patch.cfg")).unwrap(),
        "patched"
    );
    assert!(
        inst.join("fromscript2")
            .join("extra")
            .join("patch.cfg")
            .exists()
    );
    assert_eq!(
        std::fs::read_to_string(inst.join("config.txt")).unwrap(),
        "res=960\r\nfull=0\r\n"
    );
    let bin = std::fs::read(inst.join("game.bin")).unwrap();
    assert_eq!(&bin[4..6], &[0xDE, 0xAD]);
    assert_eq!(bin.len(), 16);
    assert!(inst.join("save1_NUCLEUS_BACKUP.dat").exists());
    assert!(inst.join("Profile_Beta").is_dir());
    assert!(!inst.join("Profile_Alpha").exists());
    let modified = std::fs::metadata(inst.join("save1.dat"))
        .unwrap()
        .modified()
        .unwrap();
    assert!(
        modified < std::time::SystemTime::now() - std::time::Duration::from_secs(86_400 * 365),
        "ModifiedDate should move the timestamp into the past"
    );

    // Requests reflected back to the launcher.
    assert_eq!(ctx.proto_controllers, [2, 3, 0, 0]);
    assert!(ctx.keep_edited_reg_keys);
    assert_eq!(ctx.start_arguments, "-windowed");
    // Player index 1 is not the first player, so desktop/taskbar requests are ignored.
    assert!(!ctx.hide_taskbar_requested);
    assert!(!ctx.hide_desktop_requested);

    assert!(engine.call_game_callback("OnStop").unwrap());
    assert_eq!(read(&mut engine, "out.stopped"), "true");
    assert!(!engine.call_game_callback("OnCustomHotKey_1").unwrap());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn first_player_desktop_requests_are_reflected() {
    let mut ctx = HandlerContext::new(0, "Player1".to_string());
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .load_handler_script(
            r#"
            Game.GUID = "hide";
            Game.GameName = "Hide";
            Game.ExecutableName = "hide.exe";
            Game.Play = function() { Context.HideDesktop(true); };
        "#,
        )
        .unwrap();
    engine.execute_play(&mut ctx).unwrap();
    assert!(ctx.hide_desktop_requested);
    assert!(ctx.hide_taskbar_requested);
}

#[cfg(windows)]
#[test]
fn context_registry_api_round_trips_and_backs_up() {
    let dir = temp_dir("reg");
    let leaf = format!("ctx_parity_{}", std::process::id());
    let mut ctx = HandlerContext::new(0, "Player1".to_string());
    ctx.nucleus_folder = Some(dir.to_string_lossy().to_string());

    let script = format!(
        r#"
        Game.GUID = "reg";
        Game.GameName = "Reg";
        Game.ExecutableName = "reg.exe";
        var out = {{}};
        Game.Play = function() {{
            var key = "Software\\NucleusCoopTests\\{leaf}";
            Context.EditRegKeyNoBackup("HKEY_CURRENT_USER", key, "Width", 1280, Nucleus.RegType.DWord);
            Context.EditRegKey("HKEY_CURRENT_USER", key, "Name", "Player1", Nucleus.RegType.String);
            out.width = Context.ReadRegKey("HKEY_CURRENT_USER", key, "Width");
            out.name = Context.ReadRegKey("HKEY_CURRENT_USER", key, "Name");
            Context.DeleteRegKeyValues("HKEY_CURRENT_USER", key, ["Width"]);
            out.gone = Context.ReadRegKey("HKEY_CURRENT_USER", key, "Width");
            out.bad = Context.ReadRegKey("HKEY_CLASSES_ROOT", key, "Width");
            Context.DeleteRegKey("HKEY_CURRENT_USER", "Software\\NucleusCoopTests", "{leaf}");
            out.after = Context.ReadRegKey("HKEY_CURRENT_USER", key, "Name");
        }};
        "#
    );
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_handler_script(&script).unwrap();
    engine.execute_play(&mut ctx).unwrap();

    let read = |engine: &mut ScriptEngine, expr: &str| -> String {
        let v = engine.execute(&format!("String({expr})")).unwrap();
        v.as_string().unwrap().to_std_string_escaped()
    };
    assert_eq!(read(&mut engine, "out.width"), "1280");
    assert_eq!(read(&mut engine, "out.name"), "Player1");
    assert_eq!(read(&mut engine, "out.gone"), "null");
    assert_eq!(read(&mut engine, "out.bad"), "");
    assert_eq!(read(&mut engine, "out.after"), "null");

    // The key existed before the backed-up edit, so an export must have been written.
    let backup = dir.join("utils").join("backup").join(format!("{leaf}.reg"));
    assert!(backup.exists(), "expected registry export at {backup:?}");
    // Restoring re-imports the export (recreating the key) and removes the export file.
    assert_eq!(
        nucleus_script::context_ext::registry::restore_backups(&dir),
        2,
        "the edited key and its parent were both exported"
    );
    assert!(!backup.exists());
    assert!(nucleus_script::context_ext::registry::key_exists(
        "HKEY_CURRENT_USER",
        &format!("Software\\NucleusCoopTests\\{leaf}")
    ));
    nucleus_script::context_ext::registry::delete_sub_key(
        "HKEY_CURRENT_USER",
        "Software",
        "NucleusCoopTests",
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn older_hook_settings_are_translated_to_proto_input() {
    use nucleus_hook::proto_input::{DrawFakeCursor, MoveWindowHook, SetWindowPosHook};
    let mut engine = ScriptEngine::new().unwrap();
    let config = engine
        .load_handler_script(
            r#"
            Game.GUID = "old-hooks";
            Game.GameName = "Old Hooks";
            Game.ExecutableName = "old.exe";
            Game.SupportsMultipleKeyboardsAndMice = true;
            Game.SetWindowHook = true;
            Game.HookFilterRawInput = true;
            Game.HookFilterMouseMessages = true;
            Game.HookGetKeyState = false;
            Game.SendNormalMouseInput = false;
            Game.SendScrollWheel = true;
            Game.PreventWindowDeactivation = true;
        "#,
        )
        .unwrap();
    assert!(config.proto_from_legacy_hooks);
    assert!(config.use_proto_input);
    let p = &config.proto_input;
    assert!(p.inject_runtime_easyhook && !p.inject_startup);
    assert_eq!(p.set_window_pos_hook, SetWindowPosHook::True);
    assert_eq!(p.move_window_hook, MoveWindowHook::True);
    assert!(p.raw_input_filter && p.register_raw_input_hook && p.get_raw_input_data_hook);
    assert!(p.message_filter_hook && p.mouse_move_filter && p.mouse_button_filter);
    assert!(p.get_cursor_pos_hook && p.set_cursor_pos_hook, "defaults stay enabled");
    assert!(!p.get_key_state_hook, "explicitly disabled hook stays off");
    assert!(!p.send_mouse_movement_messages && !p.send_mouse_button_messages);
    assert!(p.send_keyboard_button_messages && p.send_mouse_wheel_messages);
    assert!(p.focus_hooks && p.window_activate_filter);
    assert_eq!(p.draw_fake_cursor, DrawFakeCursor::True);

    // Start-time hooks switch to startup injection.
    let mut engine = ScriptEngine::new().unwrap();
    let early = engine
        .load_handler_script(
            r#"
            Game.GUID = "old-hooks-start";
            Game.GameName = "Old Hooks Start";
            Game.ExecutableName = "old.exe";
            Game.HookInit = true;
            Game.BlockRawInput = true;
        "#,
        )
        .unwrap();
    assert!(early.proto_from_legacy_hooks);
    assert!(early.proto_input.inject_startup && early.proto_input.block_raw_input_hook);

    // A handler that configures ProtoInput itself is left exactly as written.
    let mut engine = ScriptEngine::new().unwrap();
    let own = engine
        .load_handler_script(
            r#"
            Game.GUID = "own-proto";
            Game.GameName = "Own Proto";
            Game.ExecutableName = "own.exe";
            Game.SetWindowHook = true;
            Game.ProtoInput.InjectStartup = true;
            Game.ProtoInput.TranslateXinputtoMKB = true;
            Game.ProtoInput.TranslateXButtonA = 70;
            Game.ProtoInput.TranslateXAccelration = 9;
            Game.ProtoInput.ScaleFromX = 1280;
        "#,
        )
        .unwrap();
    assert!(!own.proto_from_legacy_hooks);
    assert_eq!(own.proto_input.set_window_pos_hook, SetWindowPosHook::None);
    assert!(own.proto_extra.translate_xinput_to_mkb);
    assert_eq!(own.proto_extra.button_a, 70);
    assert_eq!(own.proto_extra.acceleration, 9);
    assert_eq!(own.proto_extra.scale_from_x, 1280);

    // No hook settings at all: nothing is derived.
    let mut engine = ScriptEngine::new().unwrap();
    let plain = engine
        .load_handler_script(
            r#"Game.GUID = "plain"; Game.GameName = "Plain"; Game.ExecutableName = "p.exe";"#,
        )
        .unwrap();
    assert!(!plain.proto_from_legacy_hooks && !plain.use_proto_input);
}
