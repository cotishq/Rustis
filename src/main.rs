use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;
use bytes::Buf;
use anyhow::Result;

mod resp;
mod db;
mod commands;
mod replication;

use resp::Value;
use db::{Db, ServerConfig, ServerRole};
use commands::dispatch;
use replication::handshake::perform_handshake;


fn parse_args() -> (String, ServerConfig) {
    let args: Vec<String> = std::env::args().collect();
    let mut port = "6379".to_string();
    let mut config = ServerConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--replicaof" => {
                if i + 1 < args.len() {
                    let replica_info = &args[i + 1];
                    let parts: Vec<&str> = replica_info.split_whitespace().collect();
                    if parts.len() == 2 {
                        let master_host = parts[0].to_string();
                        let master_port: u16 = parts[1].parse().unwrap_or(6379);
                        config.role = ServerRole::Slave { master_host, master_port };
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    (port, config)
}

#[tokio::main]
async fn main() {
    let (port, config) = parse_args();
    let listener = TcpListener::bind(format!("127.0.0.1:{}" ,port)).await.unwrap();
    println!("Rustis server listening on 127.0.0.1:{}", port);

    // If replica mode, connect to master and perform handshake
    if let ServerRole::Slave { ref master_host, master_port } = config.role {
        let replica_port: u16 = port.parse().unwrap_or(6379);
        tokio::spawn(perform_handshake(master_host.clone(), master_port, replica_port));
    }

    let store = Db::new(config);

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
    let mut in_transaction = false;
    let mut queued_commands: Vec<(String, Vec<Value>)> = Vec::new();

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

                        // Handle transaction commands specially
                        let response = match command.as_str() {
                            "MULTI" => {
                                in_transaction = true;
                                queued_commands.clear();
                                Value::SimpleString("OK".into())
                            }
                            "EXEC" => {
                                if in_transaction {
                                    in_transaction = false;
                                    
                                    // Execute all queued commands
                                    let mut results = Vec::new();
                                    for (cmd, cmd_args) in queued_commands.drain(..) {
                                        let result = dispatch(&cmd, &cmd_args, &db).await;
                                        results.push(result);
                                    }
                                    
                                    Value::Array(results)
                                } else {
                                    Value::Error("ERR EXEC without MULTI".into())
                                }
                            }
                            "DISCARD" => {
                                if in_transaction {
                                    in_transaction = false;
                                    queued_commands.clear();
                                    Value::SimpleString("OK".into())
                                } else {
                                    Value::Error("ERR DISCARD without MULTI".into())
                                }
                            }
                            _ => {
                                if in_transaction {
                                    // Queue the command instead of executing it
                                    queued_commands.push((command.clone(), args));
                                    Value::SimpleString("QUEUED".into())
                                } else {
                                    // Execute normally if not in a transaction
                                    dispatch(&command, &args, &db).await
                                }
                            }
                        };

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
