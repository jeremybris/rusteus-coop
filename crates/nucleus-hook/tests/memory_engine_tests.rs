//! Unit tests for in-process memory manipulation engine (Cheat Engine parity).

#[cfg(windows)]
mod windows_tests {
    use nucleus_hook::memory_engine::MemoryEngine;

    #[test]
    fn test_memory_engine_read_write_primitive_values() {
        let current_pid = std::process::id();

        // Test integer read/write
        let mut target_val: i32 = 1337;
        let addr = &mut target_val as *mut i32 as usize;

        let read_back: i32 =
            MemoryEngine::read_value(current_pid, addr).expect("Failed to read primitive value");
        assert_eq!(read_back, 1337);

        // Write new value
        MemoryEngine::write_value(current_pid, addr, &9999i32)
            .expect("Failed to write primitive value");
        assert_eq!(target_val, 9999);

        // Test float read/write
        let mut target_float: f32 = 3.14159;
        let float_addr = &mut target_float as *mut f32 as usize;
        let read_float: f32 =
            MemoryEngine::read_value(current_pid, float_addr).expect("Failed to read float");
        assert!((read_float - 3.14159).abs() < 0.0001);

        MemoryEngine::write_value(current_pid, float_addr, &2.71828f32)
            .expect("Failed to write float");
        assert!((target_float - 2.71828).abs() < 0.0001);
    }

    #[test]
    fn test_memory_engine_aob_scan_exact_and_wildcard() {
        let current_pid = std::process::id();

        // Create buffer with known unique sequence
        let buffer: Vec<u8> = vec![
            0xAA, 0xBB, 0x11, 0x22, 0x33, 0x44, 0xCC, 0xDD, 0xDE, 0xAD, 0xBE, 0xEF, 0x77, 0x88,
            0x99, 0x00,
        ];
        let buf_addr = buffer.as_ptr() as usize;

        // Exact pattern scan
        let exact_pattern = [0xDE, 0xAD, 0xBE, 0xEF];
        let exact_mask = "xxxx";

        let matches = MemoryEngine::aob_scan_range(
            current_pid,
            buf_addr,
            buffer.len(),
            &exact_pattern,
            exact_mask,
        )
        .expect("Exact AOB scan failed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], buf_addr + 8);

        // Wildcard pattern scan: DE ?? BE EF
        let wildcard_pattern = [0xDE, 0x00, 0xBE, 0xEF];
        let wildcard_mask = "x?xx";
        let wildcard_matches = MemoryEngine::aob_scan_range(
            current_pid,
            buf_addr,
            buffer.len(),
            &wildcard_pattern,
            wildcard_mask,
        )
        .expect("Wildcard AOB scan failed");

        assert_eq!(wildcard_matches.len(), 1);
        assert_eq!(wildcard_matches[0], buf_addr + 8);
    }

    #[test]
    fn test_memory_engine_resolve_pointer_chain() {
        let current_pid = std::process::id();

        // Construct 2-level pointer chain in memory:
        // Base -> [offset0] points to second_struct
        // second_struct -> [offset1] is target variable (4242)
        let mut base_struct = [0usize; 4];
        let mut second_struct = [0u32; 8];

        second_struct[4] = 4242; // target variable at offset 4 * 4 = 16 (0x10)
        let second_struct_addr = second_struct.as_ptr() as usize;

        // base_struct[1] is at offset 1 * size_of::<usize>()
        base_struct[1] = second_struct_addr;

        let base_addr = base_struct.as_ptr() as usize;
        let offset0 = std::mem::size_of::<usize>(); // 0x08 on 64-bit, 0x04 on 32-bit
        let offset1 = 4 * std::mem::size_of::<u32>(); // 0x10 (16 bytes)

        let resolved = MemoryEngine::resolve_pointer_chain(
            current_pid,
            base_addr,
            &[offset0, offset1],
            cfg!(target_pointer_width = "64"),
        )
        .expect("Failed to resolve pointer chain");

        let expected_addr = &second_struct[4] as *const u32 as usize;
        assert_eq!(resolved, expected_addr);

        let val: u32 = MemoryEngine::read_value(current_pid, resolved)
            .expect("Failed reading from resolved pointer address");
        assert_eq!(val, 4242);
    }
}
