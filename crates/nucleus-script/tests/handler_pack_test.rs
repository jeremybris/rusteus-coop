//! Loads a folder of real handler scripts through the engine.
//!
//! Point `NUCLEUS_HANDLER_PACK_DIR` at a folder of `.js` handlers (for example the scripts
//! extracted from the community handler packages) to run it:
//!
//! ```powershell
//! $env:NUCLEUS_HANDLER_PACK_DIR = "C:\path\to\handlers"
//! cargo test -p nucleus-script --test handler_pack_test -- --nocapture
//! ```
//!
//! Only the top-level configuration of each script is evaluated. `Game.Play` and the other
//! callbacks are never invoked, so nothing on the machine is modified.

use nucleus_script::ScriptEngine;

#[test]
fn every_handler_in_the_pack_loads() {
    let Ok(dir) = std::env::var("NUCLEUS_HANDLER_PACK_DIR") else {
        eprintln!("NUCLEUS_HANDLER_PACK_DIR not set; skipping handler pack test");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("handler pack directory must be readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("js"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .js handlers found in {dir}");

    let mut failures: Vec<(String, String)> = Vec::new();
    let mut with_play = 0usize;
    let mut with_proto = 0usize;
    let mut with_lock_callbacks = 0usize;
    for file in &files {
        let bytes = std::fs::read(file).unwrap();
        // Handlers are saved as UTF-8 with or without BOM, or occasionally as ANSI.
        let source = String::from_utf8_lossy(&bytes);
        let source = source.trim_start_matches('\u{feff}');
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        let mut engine = match ScriptEngine::new() {
            Ok(e) => e,
            Err(e) => {
                failures.push((name, format!("engine: {e}")));
                continue;
            }
        };
        match engine.load_handler_script(source) {
            Ok(config) => {
                if nucleus_script::legacy_props::proto_input_requested(&config.proto_input) {
                    with_proto += 1;
                }
                let mut has = |expr: &str| -> bool {
                    engine
                        .execute(&format!("typeof ({expr}) === 'function'"))
                        .ok()
                        .and_then(|v| v.as_boolean())
                        .unwrap_or(false)
                };
                if has("Game.Play") {
                    with_play += 1;
                }
                if has("Game.ProtoInput.OnInputLocked") || has("Game.ProtoInput.OnInputUnlocked") {
                    with_lock_callbacks += 1;
                }
            }
            Err(e) => failures.push((name, e.to_string())),
        }
    }

    eprintln!(
        "handler pack: {} scripts, {} loaded, {} failed; {} define Game.Play, {} request \
         ProtoInput injection, {} define input-lock callbacks",
        files.len(),
        files.len() - failures.len(),
        failures.len(),
        with_play,
        with_proto,
        with_lock_callbacks
    );
    for (name, err) in &failures {
        let short: String = err.chars().take(160).collect();
        eprintln!("  FAILED {name}: {short}");
    }
    assert!(
        failures.is_empty(),
        "{} of {} handlers failed to load",
        failures.len(),
        files.len()
    );
}
