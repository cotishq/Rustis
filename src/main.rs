use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;
use bytes::Buf;
use anyhow::Result;

mod resp;
mod db;
mod commands;

use resp::Value;
use db::Db;
use commands::dispatch;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    println!("Rustis server listening on 127.0.0.1:6379");

    let store = Db::new();

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection from: {}", addr);
                let db = store.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, db).await {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, db: Db) -> Result<()> {
    let mut buffer = BytesMut::with_capacity(512);

    loop {
        let n = stream.read_buf(&mut buffer).await?;

        if n == 0 {
            println!("Connection closed");
            return Ok(());
        }

        // Parse the RESP message
        match resp::parse_message(buffer.clone()) {
            Ok((value, consumed)) => {
                buffer.advance(consumed);

                // Extract command and args
                match extract_command(value) {
                    Ok((command, args)) => {
                        println!("Command: {}, Args: {:?}", command, args);

                        // Dispatch the command
                        let response = dispatch(&command, &args, &db).await;

                        // Send the response
                        let serialized = resp::serialize(response);
                        stream.write_all(serialized.as_bytes()).await?;
                    }
                    Err(e) => {
                        eprintln!("Command extraction error: {}", e);
                        let error = Value::SimpleString("ERR invalid command format".into());
                        let serialized = resp::serialize(error);
                        stream.write_all(serialized.as_bytes()).await?;
                    }
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
                // Wait for more data
                if buffer.len() > 1024 * 1024 {
                    let error = Value::SimpleString("ERR protocol error: too large bulk count".into());
                    let serialized = resp::serialize(error);
                    stream.write_all(serialized.as_bytes()).await?;
                    return Err(e);
                }
            }
        }
    }
}

fn extract_command(value: Value) -> Result<(String, Vec<Value>)> {
    match value {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Err(anyhow::anyhow!("Empty command array"));
            }

            let command = match &arr[0] {
                Value::BulkString(s) => s.to_ascii_uppercase(),
                _ => return Err(anyhow::anyhow!("Command is not a bulk string")),
            };

            let args = arr.into_iter().skip(1).collect();
            Ok((command, args))
        }
        _ => Err(anyhow::anyhow!("Expected array")),
    }
}
