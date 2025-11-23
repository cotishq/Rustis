use resp::Value;
use tokio::net::{TcpListener , TcpStream};
use anyhow::Result;
use bytes::Bytes;

mod resp;
mod db;

use db::Db;

#[tokio::main]
async fn main() {

    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    let store = Db::new();

    loop {
        let stream = listener.accept().await;
        match stream {
            Ok((stream, _)) => {
                println!("accepted a new connection");

                tokio::spawn({
                    let store = store.clone();
                    async move {
                        handle_conn(stream, store).await
                    }
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

async fn handle_conn(stream: TcpStream , store: Db){
    let mut handler = resp::RespHandler::new(stream);

    println!("Starting read loop");

    loop {
        let value = handler.read_value().await.unwrap();

        println!("Got value {:?}" , value);

        let response = if let Some(v) = value{
            let (command , args) = extract_command(v).unwrap() ;
            match command.as_str(){
                "ping" => Value::SimpleString("PONG".to_string()),
                "echo" => args.first().unwrap().clone(),
                "set" => {
                    if args.len() < 2 {
                        Value::SimpleString("Err wrong number of arguments".into())
                    } else {
                        let key = unpack_bulk_str(args[0].clone()).unwrap();
                        let value = unpack_bulk_str(args[1].clone()).unwrap();

                        let expire = if args.len() >= 4 && unpack_bulk_str(args[2].clone()).unwrap().to_lowercase() == "px" {
                            let ms: u64 = unpack_bulk_str(args[3].clone()).unwrap().parse().unwrap();
                            Some(tokio::time::Duration::from_millis(ms))
                        } else {
                            None
                        };

                        store.set(key, Bytes::from(value), expire);

                        Value::SimpleString("OK".to_string())
                    }
                }
                "get" => {
                    let key = unpack_bulk_str(args[0].clone()).unwrap();

                    match store.get(&key) {
                        Some(v) => Value::BulkString(String::from_utf8(v.to_vec()).unwrap()),
                        None => Value::BulkString("nil".into())
                    }
                }
                c => panic!("Cannot handle command {}" , c),
            }
        } else {
            break;
        };

        println!("sending value {:?}" , response);

        handler.write_value(response).await.unwrap();
    }
}

fn extract_command(value : Value) -> Result<(String , Vec<Value>)>{
    match value{
        Value::Array(a) => {
            Ok((
                unpack_bulk_str(a.first().unwrap().clone())?,
                a.into_iter().skip(1).collect(),
            ))
        },
        _ => Err(anyhow::anyhow!("Unexpected command format")),
    }
}

fn unpack_bulk_str(value: Value) -> Result<String> {
    match value {
        Value::BulkString(s) => Ok(s),
        _ => Err(anyhow::anyhow!("Expected command to be a bulk string"))
    }
}
