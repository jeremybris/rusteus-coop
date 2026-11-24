use nucleus_hook::pe_inspector::{PeInspector, PeMachine};
use std::path::Path;

#[test]
fn test_inspect_kernel32_pe_header() {
    let kernel32_path = Path::new("C:\\Windows\\System32\\kernel32.dll");
    if kernel32_path.exists() {
        let info =
            PeInspector::inspect_file(kernel32_path).expect("Failed to parse kernel32.dll PE");
        assert_eq!(info.is_dll, true);
        assert_eq!(info.machine, PeMachine::X64);
        assert_eq!(info.is_64bit, true);
        assert!(
            info.exports.contains_key("LoadLibraryW"),
            "Should export LoadLibraryW"
        );
        assert!(
            info.exports.contains_key("GetProcAddress"),
            "Should export GetProcAddress"
        );
        assert!(
            info.exports.contains_key("VirtualAlloc"),
            "Should export VirtualAlloc"
        );
    }
}

#[test]
fn test_inspect_invalid_pe_returns_error() {
    let invalid_bytes = b"NOT_A_PE_FILE_HEADER_DATA";
    let res = PeInspector::inspect_bytes(invalid_bytes);
    assert!(res.is_err(), "Invalid bytes should fail parsing");
}
