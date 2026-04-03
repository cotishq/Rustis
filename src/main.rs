use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;
use bytes::Buf;
use anyhow::Result;

mod resp;
mod db;
mod rdb;
mod commands;
mod replication;
mod client;
mod tui;

use resp::Value;
use db::{Db, ServerConfig, ServerRole};
use commands::dispatch;
use replication::handshake::perform_handshake;
use client::ClientState;


struct AppArgs {
    port: String,
    host: String,
    config: ServerConfig,
    tui_mode: bool,
}

fn parse_args() -> AppArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut port = "6379".to_string();
    let mut host = "127.0.0.1".to_string();
    let mut config = ServerConfig::default();
    let mut tui_mode = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--tui" => {
                tui_mode = true;
                i += 1;
            }
            "--host" => {
                if i + 1 < args.len() {
                    host = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
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
            "--dir" => {
                if i + 1 < args.len() {
                    config.dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--dbfilename" => {
                if i + 1 < args.len() {
                    config.dbfilename = Some(args[i + 1].clone());
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

    AppArgs {
        port,
        host,
        config,
        tui_mode,
    }
}

#[tokio::main]
async fn main() {
    let args = parse_args();

    if args.tui_mode {
        if let Err(e) = tui::run(&args.host, &args.port) {
            eprintln!("TUI error: {}", e);
        }
        return;
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{}" ,args.port)).await.unwrap();
    println!("Rustis server listening on 127.0.0.1:{}", args.port);

    // If replica mode, connect to master and perform handshake
    if let ServerRole::Slave { ref master_host, master_port } = args.config.role {
        let replica_port: u16 = args.port.parse().unwrap_or(6379);
        tokio::spawn(perform_handshake(master_host.clone(), master_port, replica_port));
    }

    let store = Db::new(args.config);

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



async fn handle_connection(stream: TcpStream, db: Db) -> Result<()> {
    let mut buffer = BytesMut::with_capacity(512);
    let mut client = ClientState::new();
    let (mut reader, mut writer) = stream.into_split();

    loop {
        // Check for pubsub messages if subscribed
        if client.is_subscribed() {
            // First, process any remaining data in the buffer before waiting
            if !buffer.is_empty() {
                process_buffer(&mut buffer, &mut writer, &db, &mut client).await?;
                writer.flush().await?;
                continue;
            }
            tokio::select! {
                // Wait for pubsub messages
                Some(msg) = client.pubsub_rx.recv() => {
                    let response = Value::Array(vec![
                        Value::BulkString("message".into()),
                        Value::BulkString(msg.channel),
                        Value::BulkString(msg.message),
                    ]);
                    let serialized = resp::serialize(response);
                    writer.write_all(serialized.as_bytes()).await?;
                    writer.flush().await?;
                }
                // Read more data from client
                result = reader.read_buf(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            println!("Connection closed");
                            return Ok(());
                        }
                        Ok(_) => {
                            process_buffer(&mut buffer, &mut writer, &db, &mut client).await?;
                            writer.flush().await?;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        } else {
            // Not subscribed - simple read loop
            // First, process any remaining data in the buffer
            if !buffer.is_empty() {
                process_buffer(&mut buffer, &mut writer, &db, &mut client).await?;
                continue;
            }
            let n = reader.read_buf(&mut buffer).await?;
            if n == 0 {
                println!("Connection closed");
                return Ok(());
            }
            process_buffer(&mut buffer, &mut writer, &db, &mut client).await?;
        }
    }
}

use tokio::net::tcp::OwnedWriteHalf;

async fn process_buffer(
    buffer: &mut BytesMut,
    writer: &mut OwnedWriteHalf,
    db: &Db,
    client: &mut ClientState,
) -> Result<()> {
    loop {
        if buffer.is_empty() {
            break;
        }

        match resp::parse_message(buffer.clone()) {
            Ok((value, consumed)) => {
                buffer.advance(consumed);

                match extract_command(value) {
                    Ok((command, args)) => {
                        println!("Command: {}, Args: {:?}", command, args);

                        let response = if client.in_transaction
                            && command != "MULTI"
                            && command != "EXEC"
                            && command != "DISCARD"
                        {
                            client.queued_commands.push((command.clone(), args));
                            Value::SimpleString("QUEUED".into())
                        } else {
                            dispatch(&command, &args, db, client).await
                        };

                        let serialized = resp::serialize(response);
                        writer.write_all(serialized.as_bytes()).await?;
                    }
                    Err(e) => {
                        eprintln!("Command extraction error: {}", e);
                        let error = Value::SimpleString("ERR invalid command format".into());
                        let serialized = resp::serialize(error);
                        writer.write_all(serialized.as_bytes()).await?;
                    }
                }
            }
            Err(_) => break,
        }
    }

    if buffer.len() > 1024 * 1024 {
        let error = Value::SimpleString("ERR protocol error: too large bulk count".into());
        let serialized = resp::serialize(error);
        writer.write_all(serialized.as_bytes()).await?;
        return Err(anyhow::anyhow!("Buffer too large"));
    }

    Ok(())
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
