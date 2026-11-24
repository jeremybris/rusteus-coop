use nucleus_platform::ProcessMemoryManager;
use nucleus_platform_windows::WindowsProcessMemoryManager;

#[test]
fn test_windows_process_memory_manager_read_write() {
    let mgr = WindowsProcessMemoryManager::new();
    let our_pid = std::process::id();

    let mut sample_data = [0xAAu8, 0xBB, 0xCC, 0xDD];
    let address = sample_data.as_mut_ptr() as usize;

    // Read initial bytes
    let mut read_buf = [0u8; 4];
    let read_bytes = mgr
        .read_memory(our_pid, address, &mut read_buf)
        .expect("read_memory failed");
    assert_eq!(read_bytes, 4);
    assert_eq!(read_buf, [0xAA, 0xBB, 0xCC, 0xDD]);

    // Write new bytes
    let new_data = [0x11u8, 0x22, 0x33, 0x44];
    let written = mgr
        .write_memory(our_pid, address, &new_data)
        .expect("write_memory failed");
    assert_eq!(written, 4);
    assert_eq!(sample_data, [0x11, 0x22, 0x33, 0x44]);

    // Protect memory
    let old_protect = mgr
        .virtual_protect(our_pid, address, 4, 0x40)
        .expect("virtual_protect failed"); // PAGE_EXECUTE_READWRITE
    assert!(old_protect > 0);
}
