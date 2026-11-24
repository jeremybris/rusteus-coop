//! Real-time Inter-Process Communication (Named Pipes and Shared Memory).

pub mod named_pipe;
pub mod shared_memory;

pub use named_pipe::{IpcMessage, NamedPipeConnection, NamedPipeIpcServer, send_pipe_message_sync};
pub use shared_memory::SharedMemorySegment;
