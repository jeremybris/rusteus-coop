//! Windows Named Pipe IPC server and connection handling.

use nucleus_common::error::{NucleusError, NucleusResult};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

/// Maximum IPC frame size (16 MB) to prevent out-of-memory denial of service.
pub const MAX_IPC_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Represents an IPC message containing a JSON or UTF-8 payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcMessage {
    payload: String,
}

impl IpcMessage {
    /// Creates a new IPC message from a raw UTF-8 string payload.
    pub fn new(payload: impl Into<String>) -> Self {
        Self {
            payload: payload.into(),
        }
    }

    /// Serializes a value into a JSON IPC message.
    pub fn from_json<T: Serialize>(value: &T) -> NucleusResult<Self> {
        let json_str = serde_json::to_string(value).map_err(|e| {
            NucleusError::Ipc(format!("Failed to serialize IPC payload to JSON: {e}"))
        })?;
        Ok(Self::new(json_str))
    }

    /// Deserializes the IPC message payload from JSON into a typed structure.
    pub fn to_json<T: DeserializeOwned>(&self) -> NucleusResult<T> {
        serde_json::from_str(&self.payload).map_err(|e| {
            NucleusError::Ipc(format!(
                "Failed to deserialize IPC payload from JSON: {e}. Payload: {}",
                self.payload
            ))
        })
    }

    /// Returns the raw UTF-8 string payload.
    pub fn raw_payload(&self) -> &str {
        &self.payload
    }
}

/// An active connection with a client over a Windows named pipe.
pub struct NamedPipeConnection {
    #[cfg(windows)]
    server: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl NamedPipeConnection {
    /// Sends an IPC message prefixed with a 4-byte little-endian length header.
    pub async fn send(&mut self, msg: &IpcMessage) -> NucleusResult<()> {
        #[cfg(windows)]
        {
            let payload = msg.raw_payload().as_bytes();
            if payload.len() > MAX_IPC_FRAME_SIZE {
                return Err(NucleusError::Ipc(format!(
                    "IPC message payload length {} exceeds maximum allowed {}",
                    payload.len(),
                    MAX_IPC_FRAME_SIZE
                )));
            }
            let len_bytes = (payload.len() as u32).to_le_bytes();
            self.server
                .write_all(&len_bytes)
                .await
                .map_err(|e| NucleusError::Ipc(format!("Failed writing IPC length prefix: {e}")))?;
            self.server
                .write_all(payload)
                .await
                .map_err(|e| NucleusError::Ipc(format!("Failed writing IPC payload: {e}")))?;
            self.server
                .flush()
                .await
                .map_err(|e| NucleusError::Ipc(format!("Failed flushing IPC pipe: {e}")))?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = msg;
            Err(NucleusError::Platform(
                "Named pipes are only supported on Windows".into(),
            ))
        }
    }

    /// Receives an IPC message prefixed with a 4-byte little-endian length header.
    pub async fn recv(&mut self) -> NucleusResult<IpcMessage> {
        #[cfg(windows)]
        {
            let mut len_buf = [0u8; 4];
            self.server
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| NucleusError::Ipc(format!("Failed reading IPC length prefix: {e}")))?;
            let length = u32::from_le_bytes(len_buf) as usize;
            if length > MAX_IPC_FRAME_SIZE {
                return Err(NucleusError::Ipc(format!(
                    "Incoming IPC message length {length} exceeds maximum allowed {MAX_IPC_FRAME_SIZE}"
                )));
            }

            let mut payload_buf = vec![0u8; length];
            self.server
                .read_exact(&mut payload_buf)
                .await
                .map_err(|e| NucleusError::Ipc(format!("Failed reading IPC payload: {e}")))?;

            let payload_str = String::from_utf8(payload_buf).map_err(|e| {
                NucleusError::Ipc(format!("Incoming IPC payload is not valid UTF-8: {e}"))
            })?;

            Ok(IpcMessage::new(payload_str))
        }
        #[cfg(not(windows))]
        {
            Err(NucleusError::Platform(
                "Named pipes are only supported on Windows".into(),
            ))
        }
    }
}

/// Asynchronous Named Pipe IPC server for communicating with injected DLLs.
#[derive(Clone)]
pub struct NamedPipeIpcServer {
    pipe_name: String,
    #[cfg(windows)]
    initial_server: Arc<Mutex<Option<tokio::net::windows::named_pipe::NamedPipeServer>>>,
}

impl NamedPipeIpcServer {
    /// Binds a new named pipe server on the given path (e.g. `\\.\pipe\nucleus_instance_0`).
    pub fn bind(pipe_name: &str) -> NucleusResult<Self> {
        #[cfg(windows)]
        {
            let server = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)
                .map_err(|e| {
                    NucleusError::Ipc(format!("Failed to bind named pipe {pipe_name}: {e}"))
                })?;

            Ok(Self {
                pipe_name: pipe_name.to_string(),
                initial_server: Arc::new(Mutex::new(Some(server))),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = pipe_name;
            Err(NucleusError::Platform(
                "Named pipes are only supported on Windows".into(),
            ))
        }
    }

    /// Returns the pipe name.
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    /// Accepts an incoming client connection.
    pub async fn accept(&self) -> NucleusResult<NamedPipeConnection> {
        #[cfg(windows)]
        {
            let mut guard = self.initial_server.lock().await;
            let server = match guard.take() {
                Some(s) => s,
                None => tokio::net::windows::named_pipe::ServerOptions::new()
                    .create(&self.pipe_name)
                    .map_err(|e| {
                        NucleusError::Ipc(format!(
                            "Failed to create subsequent instance of named pipe {}: {e}",
                            self.pipe_name
                        ))
                    })?,
            };

            server.connect().await.map_err(|e| {
                NucleusError::Ipc(format!(
                    "Failed waiting for client to connect on {}: {e}",
                    self.pipe_name
                ))
            })?;

            Ok(NamedPipeConnection { server })
        }
        #[cfg(not(windows))]
        {
            Err(NucleusError::Platform(
                "Named pipes are only supported on Windows".into(),
            ))
        }
    }
}

/// Synchronously sends a message to a named pipe and reads the response.
pub fn send_pipe_message_sync(pipe_name: &str, payload: &str) -> NucleusResult<String> {
    #[cfg(windows)]
    {
        use std::io::{Read, Write};
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(pipe_name)
            .map_err(|e| {
                NucleusError::Ipc(format!(
                    "Failed to connect to named pipe '{pipe_name}': {e}"
                ))
            })?;

        let len_prefix = (payload.len() as u32).to_le_bytes();
        file.write_all(&len_prefix).map_err(|e| {
            NucleusError::Ipc(format!("Failed to write length prefix to pipe: {e}"))
        })?;
        file.write_all(payload.as_bytes())
            .map_err(|e| NucleusError::Ipc(format!("Failed to write payload to pipe: {e}")))?;
        file.flush()
            .map_err(|e| NucleusError::Ipc(format!("Failed to flush pipe: {e}")))?;

        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf).map_err(|e| {
            NucleusError::Ipc(format!("Failed to read response length from pipe: {e}"))
        })?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        file.read_exact(&mut resp_buf).map_err(|e| {
            NucleusError::Ipc(format!("Failed to read response payload from pipe: {e}"))
        })?;

        String::from_utf8(resp_buf)
            .map_err(|e| NucleusError::Ipc(format!("Response from pipe is not valid UTF-8: {e}")))
    }
    #[cfg(not(windows))]
    {
        let _ = (pipe_name, payload);
        Err(NucleusError::Platform(
            "Named pipes are only supported on Windows".into(),
        ))
    }
}
