//! JavaScript scripting engine and handler configuration for Nucleus Coop.
//!
//! Provides the runtime execution environment for game handler scripts using the Boa JS engine,
//! along with configuration structures, runtime context, and bridge interfaces.

pub mod bridge;
pub mod context_ext;
pub mod debugger;
pub mod engine;
pub mod enums;
pub mod handler_config;
pub mod handler_context;
pub mod legacy_props;
pub mod meta_info;
pub mod proto_script;

pub use bridge::JsBridge;
pub use engine::ScriptEngine;
pub use enums::*;
pub use handler_config::{HandlerConfig, HandlerOption};
pub use handler_context::HandlerContext;
pub use meta_info::GameMetaInfo;
