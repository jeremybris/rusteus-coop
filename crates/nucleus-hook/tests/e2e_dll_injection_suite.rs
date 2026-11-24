use nucleus_common::dll_injection::{
    DllInjectionConfig, InjectionMethod, InjectionTiming, TargetArchitecture, TargetInstanceFilter,
};
use nucleus_hook::ipc::named_pipe::{IpcMessage, NamedPipeIpcServer};
use nucleus_hook::ipc::shared_memory::SharedMemorySegment;
use nucleus_hook::memory_engine::MemoryEngine;
use nucleus_hook::pe_inspector::PeInspector;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

#[test]
fn test_e2e_pe_inspection_and_architecture_validation() {
    let kernel32_path = Path::new(r"C:\Windows\System32\kernel32.dll");
    if kernel32_path.exists() {
        let pe_info =
            PeInspector::inspect_file(kernel32_path).expect("Failed to parse kernel32.dll");
        assert_eq!(pe_info.is_64bit, cfg!(target_arch = "x86_64"));
        assert!(pe_info.exports.contains_key("LoadLibraryW"));
        assert!(pe_info.exports.contains_key("GetProcAddress"));
        assert!(pe_info.exports.contains_key("FreeLibrary"));

        // Current process compatibility check
        let current_pid = std::process::id();
        let compat = PeInspector::verify_compatibility(kernel32_path, current_pid);
        assert!(
            compat.is_ok(),
            "kernel32 must be compatible with current process bitness"
        );
    }
}

#[test]
fn test_e2e_memory_engine_full_workflow() {
    // 1. Target memory buffer with known bytes
    let mut data_buffer: Vec<u8> = vec![0x11, 0x22, 0x33, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
    let buffer_addr = data_buffer.as_mut_ptr() as usize;
    let current_pid = std::process::id();

    // 2. AOB Pattern scan with wildcard mask
    let pattern = [0xAA, 0x00, 0xCC, 0xDD];
    let mask = "x?xx";
    let matches = MemoryEngine::aob_scan(current_pid, &pattern, mask).expect("AOB scan failed");
    assert!(
        !matches.is_empty(),
        "Must find at least one match for buffer pattern"
    );

    // 3. Multi-level pointer chain resolution
    #[repr(C)]
    struct Level2 {
        _pad: [u8; 16],
        health: i32,
    }
    #[repr(C)]
    struct Level1 {
        _pad: [u8; 8],
        l2_ptr: *const Level2,
    }

    let l2 = Level2 {
        _pad: [0; 16],
        health: 1337,
    };
    let l1 = Level1 {
        _pad: [0; 8],
        l2_ptr: &l2 as *const Level2,
    };
    let base_ptr = &l1 as *const Level1 as usize;

    let offsets = vec![8, 16];
    let is_64bit = std::mem::size_of::<usize>() == 8;
    let resolved_addr =
        MemoryEngine::resolve_pointer_chain(current_pid, base_ptr, &offsets, is_64bit)
            .expect("Failed to resolve pointer chain");

    let val: i32 = MemoryEngine::read_value(current_pid, resolved_addr).expect("Read value failed");
    assert_eq!(val, 1337);

    // 4. Memory Freezing & Heartbeat enforcement loop
    let freeze_target = buffer_addr + 4; // points to 0xBB
    let original: u32 = MemoryEngine::read_value(current_pid, freeze_target).expect("Read failed");
    let freeze_val: u32 = 0x99999999;
    let slot_id = MemoryEngine::freeze_value(
        current_pid,
        freeze_target,
        freeze_val.to_ne_bytes().to_vec(),
    );

    // Tamper with memory
    let tampered: u32 = 0x12345678;
    MemoryEngine::write_value(current_pid, freeze_target, &tampered).expect("Write failed");
    let read_back: u32 = MemoryEngine::read_value(current_pid, freeze_target).expect("Read failed");
    assert_eq!(read_back, 0x12345678);

    // Heartbeat tick enforces frozen value
    MemoryEngine::apply_frozen_slots();
    let restored: u32 = MemoryEngine::read_value(current_pid, freeze_target).expect("Read failed");
    assert_eq!(
        restored, freeze_val,
        "Frozen value must be restored on heartbeat tick"
    );

    // Unfreeze and restore original
    assert!(MemoryEngine::unfreeze_value(slot_id));
    MemoryEngine::write_value(current_pid, freeze_target, &original).expect("Restore failed");
}

#[tokio::test]
async fn test_e2e_named_pipe_and_shared_memory_ipc() {
    let test_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let pipe_name = format!(r"\\.\pipe\nucleus_e2e_test_{}", test_id);

    // 1. Start Named Pipe server
    let server = NamedPipeIpcServer::bind(&pipe_name).expect("Failed to bind pipe server");

    // 2. Client task connects, writes frame, and reads response
    let pipe_name_clone = pipe_name.clone();
    let client_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = ClientOptions::new()
            .open(&pipe_name_clone)
            .expect("Client failed to open pipe");

        let payload = b"{\"msg_type\":\"SetPlayerSlot\",\"slot\":1}";
        let len_prefix = (payload.len() as u32).to_le_bytes();
        client
            .write_all(&len_prefix)
            .await
            .expect("Failed to write len");
        client
            .write_all(payload)
            .await
            .expect("Failed to write payload");
        client.flush().await.expect("Failed to flush");

        let mut reply_len_buf = [0u8; 4];
        client
            .read_exact(&mut reply_len_buf)
            .await
            .expect("Failed to read reply len");
        let reply_len = u32::from_le_bytes(reply_len_buf) as usize;
        let mut reply_buf = vec![0u8; reply_len];
        client
            .read_exact(&mut reply_buf)
            .await
            .expect("Failed to read reply payload");

        String::from_utf8(reply_buf).expect("Reply not valid UTF-8")
    });

    // Accept client connection on server
    let mut connection = server.accept().await.expect("Failed to accept connection");
    let received = connection.recv().await.expect("Failed to receive message");
    assert!(received.raw_payload().contains("SetPlayerSlot"));

    let reply = IpcMessage::from_json(&serde_json::json!({
        "status": "ok",
        "acknowledged": true
    }))
    .expect("Failed to serialize reply");
    connection.send(&reply).await.expect("Failed to send reply");

    let client_reply = client_task.await.expect("Client task failed");
    assert!(client_reply.contains("\"status\":\"ok\""));

    // 3. Shared Memory segment creation and verification
    let shm_name = format!("Local\\nucleus_e2e_shm_{}", test_id);
    let mut shm = SharedMemorySegment::create(&shm_name, 1024)
        .expect("Failed to create shared memory segment");
    shm.write_bytes(0, b"HELLO_SPLITSCREEN_IPC")
        .expect("Failed to write to shared memory");

    let mut read_buf = vec![0u8; 21];
    shm.read_bytes(0, &mut read_buf)
        .expect("Failed to read from shared memory");
    assert_eq!(&read_buf, b"HELLO_SPLITSCREEN_IPC");
}

#[test]
fn test_e2e_dll_injection_config_validation() {
    let mut cfg = DllInjectionConfig::new("mods/custom_fov.dll");
    cfg.target_instances = TargetInstanceFilter::Specific(vec![0, 1]);
    cfg.timing = InjectionTiming::OnWindowCreated;
    cfg.delay_ms = 500;
    cfg.architecture = TargetArchitecture::Auto;
    cfg.method = InjectionMethod::CreateRemoteThread;
    cfg.export_name = Some("SetFovDegrees".to_string());
    cfg.export_argument = Some("95".to_string());
    cfg.eject_on_close = true;

    // Verify filter logic
    assert!(cfg.target_instances.applies_to(0, 4));
    assert!(cfg.target_instances.applies_to(1, 4));
    assert!(!cfg.target_instances.applies_to(2, 4));

    // Verify JSON serialization roundtrip
    let json = serde_json::to_string(&cfg).expect("JSON serialize failed");
    let deserialized: DllInjectionConfig =
        serde_json::from_str(&json).expect("JSON deserialize failed");
    assert_eq!(cfg, deserialized);
}
