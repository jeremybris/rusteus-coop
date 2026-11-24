use nucleus_hook::privilege::open_target_process;
use nucleus_hook::remote_memory::RemoteMemoryGuard;

#[test]
fn test_remote_memory_guard_local_allocation_and_drop() {
    let current_pid = std::process::id();
    let process = open_target_process(current_pid, true).expect("Failed to open current process");

    let test_data = b"NucleusCoopInjectionTestString123";
    {
        let guard = RemoteMemoryGuard::allocate(process, test_data.len() + 1)
            .expect("Failed to allocate remote memory");
        assert!(
            !guard.address().is_null(),
            "Allocated address must not be null"
        );
        assert_eq!(guard.size(), test_data.len() + 1);

        guard
            .write(test_data)
            .expect("Failed to write to allocated memory");

        // Verify bytes written
        let mut read_buf = vec![0u8; test_data.len()];
        let read_res = guard.read(&mut read_buf);
        assert!(read_res.is_ok());
        assert_eq!(&read_buf[..], test_data);
    } // guard drops here and calls VirtualFreeEx automatically
}
