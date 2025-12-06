use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::resp;
use crate::resp::Value;

pub async fn perform_handshake(master_host: String, master_port: u16, replica_port: u16) {
    let addr = format!("{}:{}", master_host, master_port);
    println!("Connecting to master at {}", addr);

    match TcpStream::connect(&addr).await {
        Ok(mut stream) => {
            // Step 1: Send PING
            let ping = Value::Array(vec![Value::BulkString("PING".to_string())]);
            let serialized = resp::serialize(ping);
            
            if let Err(e) = stream.write_all(serialized.as_bytes()).await {
                eprintln!("Failed to send PING to master: {}", e);
                return;
            }
            println!("Sent PING to master");

            // Read PONG response
            let mut buf = [0u8; 512];
            if let Err(e) = stream.read(&mut buf).await {
                eprintln!("Failed to read PONG response: {}", e);
                return;
            }
            println!("Received PONG from master");

            // Step 2a: Send REPLCONF listening-port <PORT>
            let replconf_port = Value::Array(vec![
                Value::BulkString("REPLCONF".to_string()),
                Value::BulkString("listening-port".to_string()),
                Value::BulkString(replica_port.to_string()),
            ]);
            let serialized = resp::serialize(replconf_port);
            
            if let Err(e) = stream.write_all(serialized.as_bytes()).await {
                eprintln!("Failed to send REPLCONF listening-port: {}", e);
                return;
            }
            println!("Sent REPLCONF listening-port {}", replica_port);

            // Read OK response
            if let Err(e) = stream.read(&mut buf).await {
                eprintln!("Failed to read REPLCONF listening-port response: {}", e);
                return;
            }
            println!("Received OK for REPLCONF listening-port");

            // Step 2b: Send REPLCONF capa psync2
            let replconf_capa = Value::Array(vec![
                Value::BulkString("REPLCONF".to_string()),
                Value::BulkString("capa".to_string()),
                Value::BulkString("psync2".to_string()),
            ]);
            let serialized = resp::serialize(replconf_capa);
            
            if let Err(e) = stream.write_all(serialized.as_bytes()).await {
                eprintln!("Failed to send REPLCONF capa psync2: {}", e);
                return;
            }
            println!("Sent REPLCONF capa psync2");

            // Read OK response
            if let Err(e) = stream.read(&mut buf).await {
                eprintln!("Failed to read REPLCONF capa response: {}", e);
                return;
            }
            println!("Received OK for REPLCONF capa psync2");
        }
        Err(e) => {
            eprintln!("Failed to connect to master at {}: {}", addr, e);
        }
    }
}