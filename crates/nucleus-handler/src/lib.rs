//! Multi-instance game session handler and orchestration for Nucleus Coop.
//!
//! Provides the execution pipeline including instance directory staging, process launching,
//! window positioning, mutex killing, and hook management.

pub mod environment;
pub mod file_ops;
pub mod game_handler;
pub mod hooks;
pub mod instance;
pub mod launch;
pub mod mutex;
pub mod proto_launch;
pub mod traits;
pub mod window;

pub use environment::EnvironmentSetup;
pub use game_handler::GameHandler;
pub use hooks::HookManager;
pub use instance::InstanceSetup;
pub use launch::ProcessLauncher;
pub use mutex::MutexManager;
pub use traits::IGameHandler;
pub use window::{SplitMode, WindowBounds, WindowSetup};
