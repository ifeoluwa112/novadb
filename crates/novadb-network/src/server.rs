use novadb_common::{encode_error, encode_response};
use novadb_protocol::{parse_command, parse_resp};
use novadb_server::execute;
use novadb_storage::Database;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream};

pub async fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let database = Arc::new(Mutex::new(Database::new()));

    loop {
        let (stream, address) = listener.accept().await?;

        println!("Client connected: {address}");

        let database = Arc::clone(&database);

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, database).await {
                println!("Client error: {error}");
            }
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    database: Arc<Mutex<Database>>,
) -> std::io::Result<()> {
    let mut read_buffer = [0u8; 1024];

    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            println!("Client disconnected");
            break;
        }

        receive_buffer.extend_from_slice(&read_buffer[..bytes_read]);

        loop {
            match parse_resp(&receive_buffer) {
                Ok((value, consumed)) => {
                    println!("Parsed RESP value: {value:?}");

                    match parse_command(value) {
                        Ok(command) => {
                            println!("Parsed command: {command:?}");

                            let response = {
                                let mut db = database.lock().await;

                                execute(&mut db, command)
                            };

                            let encoded = encode_response(&response);

                            println!("Encoded response: {encoded:?}");

                            stream.write_all(encoded.as_bytes()).await?;
                        }

                        Err(error) => {
                            println!("Command error: {error:?}");

                            let encoded = encode_error(&error.to_string());

                            stream.write_all(encoded.as_bytes()).await?;
                        }
                    }

                    receive_buffer.drain(..consumed);
                }

                Err(novadb_protocol::RespError::Incomplete) => {
                    break;
                }

                Err(error) => {
                    println!("Protocol error: {error:?}");

                    let encoded = encode_error(&error.to_string());

                    stream.write_all(encoded.as_bytes()).await?;

                    receive_buffer.clear();
                    break;
                }
            }
        }
    }

    Ok(())
}
