//! Unit tests for Named Pipe and Shared Memory IPC in `nucleus-hook`.

#[cfg(windows)]
mod windows_tests {
    use nucleus_hook::ipc::named_pipe::{IpcMessage, NamedPipeIpcServer};
    use nucleus_hook::ipc::shared_memory::SharedMemorySegment;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ClientOptions;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_named_pipe_ipc_roundtrip() {
        let pipe_name = format!(r"\\.\pipe\nucleus_test_{}", Uuid::new_v4());
        let server =
            NamedPipeIpcServer::bind(&pipe_name).expect("Failed to bind NamedPipeIpcServer");

        // Spawn a client task that connects, sends a message, and reads the reply
        let pipe_name_clone = pipe_name.clone();
        let client_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut client = ClientOptions::new()
                .open(&pipe_name_clone)
                .expect("Failed to open named pipe client");

            // Send 4-byte length prefixed message: {"action":"ping","id":1}
            let payload = b"{\"action\":\"ping\",\"id\":1}";
            let len_prefix = (payload.len() as u32).to_le_bytes();
            client
                .write_all(&len_prefix)
                .await
                .expect("Failed to write len");
            client
                .write_all(payload)
                .await
                .expect("Failed to write payload");
            client.flush().await.expect("Failed to flush");

            // Read 4-byte length prefixed reply
            let mut reply_len_buf = [0u8; 4];
            client
                .read_exact(&mut reply_len_buf)
                .await
                .expect("Failed to read reply len");
            let reply_len = u32::from_le_bytes(reply_len_buf) as usize;
            let mut reply_buf = vec![0u8; reply_len];
            client
                .read_exact(&mut reply_buf)
                .await
                .expect("Failed to read reply payload");

            String::from_utf8(reply_buf).expect("Reply not valid UTF-8")
        });

        // Accept client connection on server
        let mut connection = server.accept().await.expect("Failed to accept connection");
        let msg = connection.recv().await.expect("Failed to receive message");
        assert_eq!(msg.raw_payload(), "{\"action\":\"ping\",\"id\":1}");

        // Send reply back to client
        let reply = IpcMessage::from_json(&serde_json::json!({
            "action": "pong",
            "id": 1,
            "status": "success"
        }))
        .expect("Failed to serialize reply");
        connection.send(&reply).await.expect("Failed to send reply");

        let client_reply = client_task.await.expect("Client task failed");
        assert!(client_reply.contains("\"action\":\"pong\""));
        assert!(client_reply.contains("\"status\":\"success\""));
    }

    #[test]
    fn test_shared_memory_create_write_read_roundtrip() {
        let shm_name = format!("Nucleus_TestShm_{}", Uuid::new_v4());
        let size = 65536;

        let mut producer = SharedMemorySegment::create(&shm_name, size)
            .expect("Failed to create shared memory segment");
        assert_eq!(producer.size(), size);

        // Write magic and test data
        let test_data = b"Hello Nucleus Shared Memory!";
        producer
            .write_bytes(0, test_data)
            .expect("Failed to write bytes to producer");

        // Consumer opens existing segment
        let consumer = SharedMemorySegment::open(&shm_name)
            .expect("Failed to open existing shared memory segment");
        let mut read_buf = vec![0u8; test_data.len()];
        consumer
            .read_bytes(0, &mut read_buf)
            .expect("Failed to read bytes from consumer");

        assert_eq!(&read_buf, test_data);
    }
}
