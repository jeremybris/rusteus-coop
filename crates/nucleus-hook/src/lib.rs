//! Low-level process hooking, DLL injection, and input interception for Nucleus Coop.
//!
//! Provides the core primitives to inject DLLs into game instances, configure ProtoInput
//! hooks for multiple input devices, and maintain simulated window focus.

pub mod focus_hook;
pub mod injector;
pub mod ipc;
pub mod memory_engine;
pub mod pe_inspector;
pub mod privilege;
pub mod proto_extras;
pub mod proto_input;
pub mod remote_call;
pub mod remote_memory;

pub use focus_hook::FocusHook;
pub use injector::DllInjector;
pub use ipc::{
    send_pipe_message_sync, IpcMessage, NamedPipeConnection, NamedPipeIpcServer, SharedMemorySegment,
};
pub use memory_engine::{FrozenMemorySlot, MemoryEngine};
pub use pe_inspector::{PeInfo, PeInspector, PeMachine};
pub use privilege::open_target_process;
pub use remote_call::RemoteRpc;
pub use remote_memory::RemoteMemoryGuard;
pub use proto_input::{
    DrawFakeCursor, InjectMethod, MoveWindowHook, PlayerProtoInputConfig, ProcessBitness,
    ProtoHookIDs, ProtoInput, ProtoInputConfig, ProtoInputLoader, ProtoInputMessageFilterIDs,
    ProtoInputRuntime, ProtoInputUtil, ProtoMessageFilterIDs, PutMouseInsideWindow,
    SetRemoveBorderHook, SetWindowPosHook, guid_bytes_to_dinput_fields, guid_to_dinput_fields,
};
