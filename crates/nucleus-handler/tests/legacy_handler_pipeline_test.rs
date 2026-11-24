//! Drives the staging pipeline with a handler script written against the original API and
//! checks the resulting instance folders.

use nucleus_handler::game_handler::GameHandler;
use nucleus_handler::traits::IGameHandler;
use nucleus_script::ScriptEngine;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const HANDLER: &str = r#"
Game.GUID = "legacy-pipeline";
Game.GameName = "Legacy Pipeline";
Game.ExecutableName = "game.exe";
Game.SymlinkGame = true;
Game.SymlinkExe = false;
Game.MaxPlayers = 2;
Game.CopyFoldersTo = ["cfg|cfg_copy"];
Game.HardlinkFoldersTo = ["cfg|cfg_hard"];
Game.RenameAndOrMoveFiles = ["intro.bik|movies\\intro_disabled.bik"];
Game.DeleteFiles = ["2|splash.bik"];
Game.HexEditExe = ["NAME=AAAA|NAME=P1__", "NAME=AAAA|NAME=P2__"];
Game.HexEditExeAddress = ["0|4E55"];
Game.SymlinkFiles = ["data\\shared.pak"];
Game.UseHandlerSteamIds = true;
Game.PlayerSteamIDs = ["76561190000000011", "76561190000000022"];
Game.Play = function() {
    Context.ProceedSymlink();
    Context.WriteTextFile(Context.RootFolder + "\\player.txt",
        [Context.PlayerSteamID, Context.Arch, String(Context.NumControllers), String(Context.PlayerSteamIDs.length)]);
    Context.StartArguments = "-windowed -cfg \"" + Context.RootFolder + "\\my cfg.ini\"";
    // Like many published handlers, the input-lock callback is defined inside Game.Play.
    Game.ProtoInput.OnInputLocked = function() {
        for (var i = 0; i < PlayerList.Count; i++) {
            var player = PlayerList[i];
            ProtoInput.InstallHook(player.ProtoInputInstanceHandle, ProtoInput.Values.SetCursorPosHookID);
            System.Threading.Thread.Sleep(250);
        }
    };
};
"#;

fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "nucleus_legacy_pipeline_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let game = root.join("game");
    let staging = root.join("staging");
    std::fs::create_dir_all(game.join("cfg")).unwrap();
    std::fs::create_dir_all(game.join("data")).unwrap();
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(game.join("game.exe"), b"MZ__ NAME=AAAA END").unwrap();
    std::fs::write(game.join("cfg").join("video.ini"), "w=1920").unwrap();
    std::fs::write(game.join("data").join("shared.pak"), "pak").unwrap();
    std::fs::write(game.join("intro.bik"), "intro").unwrap();
    std::fs::write(game.join("splash.bik"), "splash").unwrap();
    (root, game, staging)
}

#[test]
fn original_api_handler_stages_instances_correctly() {
    let (root, game, staging) = setup("stage");

    let mut engine = ScriptEngine::new().unwrap();
    let config = engine.load_handler_script(HANDLER).unwrap();
    drop(engine);

    let prompts = Arc::new(AtomicUsize::new(0));
    let prompts_seen = prompts.clone();
    let mut handler = GameHandler::new(config, game.clone(), game.join("game.exe"), staging, 2)
        .with_script_source(HANDLER.to_string())
        .with_prompt_handler(Arc::new(move |_msg: &str| {
            prompts_seen.fetch_add(1, Ordering::SeqCst);
            true
        }));

    handler.prepare_instances().expect("prepare_instances");
    assert_eq!(handler.instance_folders.len(), 2);

    for (idx, folder) in handler.instance_folders.iter().enumerate() {
        // Folder placement.
        assert_eq!(
            std::fs::read_to_string(folder.join("cfg_copy").join("video.ini")).unwrap(),
            "w=1920"
        );
        assert!(folder.join("cfg_hard").join("video.ini").exists());

        // Rename applies to everyone, delete only to instance 2.
        assert!(folder.join("movies").join("intro_disabled.bik").exists());
        assert!(!folder.join("intro.bik").exists());
        assert_eq!(folder.join("splash.bik").exists(), idx == 0);

        // Each player gets their own HexEditExe entry; the address edit hits both.
        let exe = std::fs::read(folder.join("game.exe")).unwrap();
        let expected: &[u8] = if idx == 0 {
            b"NU__ NAME=P1__ END"
        } else {
            b"NU__ NAME=P2__ END"
        };
        assert_eq!(exe, expected);

        // Game.Play ran with the original Context API available.
        assert_eq!(
            std::fs::read_to_string(folder.join("data").join("shared.pak")).unwrap(),
            "pak"
        );
        let player = std::fs::read_to_string(folder.join("player.txt")).unwrap();
        let lines: Vec<&str> = player.lines().collect();
        let expected_id = if idx == 0 {
            "76561190000000011"
        } else {
            "76561190000000022"
        };
        assert_eq!(lines[0], expected_id);
        assert!(lines[1] == "x64" || lines[1] == "x86");
        assert_eq!(lines[2], "2", "two players without keyboard assignments");
        assert_eq!(lines[3], "2");

        // Quoted start arguments survive as a single argument.
        let ctx = &handler.contexts[idx];
        let args = nucleus_handler::launch::split_command_line(&ctx.start_arguments);
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "-windowed");
        assert!(args[2].ends_with("my cfg.ini"));
        assert_eq!(ctx.player_steam_id.as_deref(), Some(expected_id));
    }

    // The callback defined inside Game.Play was recorded for both players, pauses included,
    // against placeholder handles that are resolved once the instances exist.
    use nucleus_handler::proto_launch::PLACEHOLDER_HANDLE_BASE as BASE;
    use nucleus_script::proto_script::{ProtoCall, ProtoCallback};
    assert_eq!(handler.proto_recordings.len(), 1);
    let (which, calls) = &handler.proto_recordings[0];
    assert_eq!(*which, ProtoCallback::InputLocked);
    assert_eq!(
        calls,
        &vec![
            ProtoCall::InstallHook {
                handle: BASE,
                hook: 4
            },
            ProtoCall::Sleep { ms: 250 },
            ProtoCall::InstallHook {
                handle: BASE + 1,
                hook: 4
            },
            ProtoCall::Sleep { ms: 250 },
        ]
    );

    // The delete prompt was shown once (for instance 2 only).
    assert_eq!(prompts.load(Ordering::SeqCst), 1);

    // The installed game is untouched by any of it.
    assert_eq!(
        std::fs::read(game.join("game.exe")).unwrap(),
        b"MZ__ NAME=AAAA END"
    );
    assert!(game.join("intro.bik").exists());
    assert!(game.join("splash.bik").exists());

    handler.stop().expect("stop");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn delete_files_is_skipped_without_confirmation() {
    let (root, game, staging) = setup("noprompt");
    let script = r#"
        Game.GUID = "legacy-noprompt";
        Game.GameName = "Legacy NoPrompt";
        Game.ExecutableName = "game.exe";
        Game.SymlinkGame = true;
        Game.DeleteFiles = ["splash.bik"];
    "#;
    let mut engine = ScriptEngine::new().unwrap();
    let config = engine.load_handler_script(script).unwrap();
    drop(engine);

    // No prompt handler: a headless session must not delete anything unasked.
    let mut handler = GameHandler::new(config, game.clone(), game.join("game.exe"), staging, 1);
    handler.prepare_instances().expect("prepare_instances");
    assert!(handler.instance_folders[0].join("splash.bik").exists());
    handler.stop().expect("stop");

    // With IgnoreDeleteFilesPrompt the handler's request is carried out directly.
    let (root2, game2, staging2) = setup("ignoreprompt");
    let script2 = format!("{script}\nGame.IgnoreDeleteFilesPrompt = true;");
    let mut engine = ScriptEngine::new().unwrap();
    let config2 = engine.load_handler_script(&script2).unwrap();
    drop(engine);
    let mut handler2 =
        GameHandler::new(config2, game2.clone(), game2.join("game.exe"), staging2, 1);
    handler2.prepare_instances().expect("prepare_instances");
    assert!(!handler2.instance_folders[0].join("splash.bik").exists());
    assert!(
        game2.join("splash.bik").exists(),
        "only the instance copy is removed"
    );
    handler2.stop().expect("stop");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn play_first_smart_steam_emu_and_custom_prompts() {
    let (root, game, staging) = setup("playfirst");
    // A stand-in for the bundled SmartSteamEmu so its loader can be deployed.
    let utils = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("utils");
    let sse = utils.join("SmartSteamEmu");
    std::fs::create_dir_all(&sse).unwrap();
    for f in [
        "SmartSteamLoader.exe",
        "SmartSteamLoader_x64.exe",
        "SmartSteamEmu.dll",
    ] {
        if !sse.join(f).exists() {
            std::fs::write(sse.join(f), b"stub").unwrap();
        }
    }

    let script = r#"
        Game.GUID = "legacy-playfirst";
        Game.GameName = "Legacy PlayFirst";
        Game.ExecutableName = "game.exe";
        Game.SymlinkGame = true;
        Game.SymlinkExe = false;
        Game.SteamID = "12345";
        Game.NeedsSteamEmulation = true;
        Game.GamePlayBeforeGameSetup = true;
        Game.UseX360ce = true;
        Game.CustomUserGeneralPrompts = ["Server address"];
        Game.CustomUserPlayerPrompts = ["Character name"];
        Game.SaveCustomUserPlayerValues = true;
        Game.Play = function() {
            // With GamePlayBeforeGameSetup the input tools are not deployed yet.
            var early = !Context.FileExists(Context.RootFolder + "\\x360ce.ini");
            Context.WriteTextFile(Context.RootFolder + "\\order.txt",
                [String(early), Context.CustomUserGeneralValues[0], Context.CustomUserPlayerValues[0]]);
            Context.StartArguments = "-name " + Context.CustomUserPlayerValues[0];
        };
    "#;
    let mut engine = ScriptEngine::new().unwrap();
    let config = engine.load_handler_script(script).unwrap();
    drop(engine);
    assert!(config.legacy.game_play_before_game_setup);

    let asked = Arc::new(AtomicUsize::new(0));
    let make = |asked: Arc<AtomicUsize>, staging: PathBuf| {
        GameHandler::new(
            config.clone(),
            game.clone(),
            game.join("game.exe"),
            staging,
            2,
        )
        .with_script_source(script.to_string())
        .with_text_prompt_handler(Arc::new(move |message: &str, default: &str| {
            asked.fetch_add(1, Ordering::SeqCst);
            if !default.is_empty() {
                default.to_string()
            } else if message.contains("Server") {
                "10.0.0.5".to_string()
            } else if message.starts_with("Player 2") {
                "Zed".to_string()
            } else {
                "Ann".to_string()
            }
        }))
    };
    let mut handler = make(asked.clone(), staging.clone());
    // Remove answers remembered by an earlier run of this test.
    let _ = std::fs::remove_dir_all(
        utils
            .parent()
            .unwrap()
            .join("content")
            .join("custom_values"),
    );
    handler.prepare_instances().expect("prepare_instances");

    for (idx, folder) in handler.instance_folders.iter().enumerate() {
        let name = if idx == 0 { "Ann" } else { "Zed" };
        let order = std::fs::read_to_string(folder.join("order.txt")).unwrap();
        let lines: Vec<&str> = order.lines().collect();
        assert_eq!(lines, vec!["true", "10.0.0.5", name]);
        assert!(
            folder.join("x360ce.ini").exists(),
            "tools are still deployed afterwards"
        );

        // The instance starts through the SmartSteamEmu loader, which gets the real
        // executable and the arguments Game.Play produced.
        let ctx = &handler.contexts[idx];
        assert!(
            ctx.exe_path.contains("SmartSteamLoader"),
            "{}",
            ctx.exe_path
        );
        assert!(ctx.start_arguments.is_empty());
        let ini = std::fs::read_to_string(
            std::path::Path::new(&ctx.exe_path)
                .parent()
                .unwrap()
                .join("SmartSteamEmu.ini"),
        )
        .unwrap();
        assert!(ini.contains("game.exe"), "{ini}");
        assert!(ini.contains(&format!("-name {name}")), "{ini}");
        assert!(ini.contains("12345"), "{ini}");
    }
    // One general prompt for the session plus one player prompt each.
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    handler.stop().expect("stop");

    // Saved player answers are reused; only the general prompt is asked again.
    let asked_again = Arc::new(AtomicUsize::new(0));
    let (root2, _game2, staging2) = setup("playfirst2");
    let mut second = make(asked_again.clone(), staging2);
    second.prepare_instances().expect("prepare_instances");
    assert_eq!(asked_again.load(Ordering::SeqCst), 1);
    let order = std::fs::read_to_string(second.instance_folders[1].join("order.txt")).unwrap();
    assert!(order.contains("Zed"));
    second.stop().expect("stop");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}
