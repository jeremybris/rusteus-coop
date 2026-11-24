//! Tests for dynamic `Context` DLL injection, IPC, and memory bridge APIs in `nucleus-script`.

use nucleus_script::{HandlerContext, ScriptEngine};

#[test]
fn test_context_memory_apis_registered_and_callable() {
    let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
    let mut ctx = HandlerContext::default();
    ctx.process_id = Some(std::process::id());

    // Inject context into engine
    engine
        .inject_context(&ctx)
        .expect("Failed to inject context");

    // Test primitive read and write via Context
    let mut target_val: i32 = 1234;
    let target_addr = &mut target_val as *mut i32 as usize;

    let script = format!(
        r#"
        let original = Context.ReadMemoryInt({target_addr});
        Context.WriteMemoryInt({target_addr}, 5678);
        let updated = Context.ReadMemoryInt({target_addr});
        [original, updated]
        "#
    );

    let result = engine.execute(&script).expect("Failed to execute script");
    assert_eq!(target_val, 5678);

    let arr = result.as_object().expect("Expected array result");
    let orig_val = arr
        .get(0, engine.context_mut())
        .unwrap()
        .as_number()
        .unwrap();
    let updated_val = arr
        .get(1, engine.context_mut())
        .unwrap()
        .as_number()
        .unwrap();

    assert_eq!(orig_val as i32, 1234);
    assert_eq!(updated_val as i32, 5678);
}

#[test]
fn test_context_pointer_resolution_and_freeze() {
    let mut engine = ScriptEngine::new().expect("Failed to create ScriptEngine");
    let mut ctx = HandlerContext::default();
    ctx.process_id = Some(std::process::id());
    engine
        .inject_context(&ctx)
        .expect("Failed to inject context");

    let mut base_struct = [0usize; 2];
    let mut target_val: i32 = 9000;
    let target_addr = &mut target_val as *mut i32 as usize;

    base_struct[1] = target_addr - 0x10; // offset 0x10
    let base_addr = base_struct.as_ptr() as usize;
    let offset0 = std::mem::size_of::<usize>();

    let script = format!(
        r#"
        let resolved = Context.ResolvePointer({base_addr}, [{offset0}, 0x10]);
        let val = Context.ReadMemoryInt(resolved);
        let freezeId = Context.FreezeMemoryInt(resolved, 9999);
        let unfrozen = Context.UnfreezeMemory(freezeId);
        [resolved, val, unfrozen]
        "#
    );

    let result = engine.execute(&script).expect("Failed to execute script");
    let arr = result.as_object().expect("Expected array result");

    let resolved_res = arr
        .get(0, engine.context_mut())
        .unwrap()
        .as_number()
        .unwrap() as usize;
    let val_res = arr
        .get(1, engine.context_mut())
        .unwrap()
        .as_number()
        .unwrap() as i32;
    let unfrozen_res = arr
        .get(2, engine.context_mut())
        .unwrap()
        .as_boolean()
        .unwrap();

    assert_eq!(resolved_res, target_addr);
    assert_eq!(val_res, 9000);
    assert!(unfrozen_res);
}
