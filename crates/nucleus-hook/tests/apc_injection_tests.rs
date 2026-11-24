//! Unit test verifying ToolHelp thread discovery and QueueUserApc configuration.

use nucleus_common::dll_injection::{
    DllInjectionConfig, InjectionMethod, InjectionTiming, TargetArchitecture,
};
use nucleus_hook::injector::DllInjector;

#[test]
fn test_find_current_process_primary_thread() {
    let current_pid = std::process::id();
    let tid = DllInjector::find_primary_thread_id(current_pid)
        .expect("failed to find thread for current process");
    assert!(tid > 0);
}

#[test]
fn test_apc_config_properties() {
    let mut config = DllInjectionConfig::new("peerparty_hook.dll");
    config.timing = InjectionTiming::AtStartupSuspended;
    config.method = InjectionMethod::QueueUserApc;
    config.architecture = TargetArchitecture::Auto;

    assert_eq!(config.timing, InjectionTiming::AtStartupSuspended);
    assert_eq!(config.method, InjectionMethod::QueueUserApc);
    assert_eq!(config.architecture, TargetArchitecture::Auto);
}
