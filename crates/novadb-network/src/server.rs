use novadb_common::{encode_error, encode_response};
use novadb_protocol::{parse_command, parse_resp};
use novadb_server::execute;
use novadb_storage::Database;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379")?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let database = Arc::new(Mutex::new(Database::new()));

    for stream in listener.incoming() {
        let stream = stream?;

        let address = stream.peer_addr()?;

        println!("Client connected: {address}");

        let database = Arc::clone(&database);

        std::thread::spawn(move || {
            if let Err(error) = handle_client(stream, database) {
                println!("Client error: {error}");
            }
        });
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, database: Arc<Mutex<Database>>) -> std::io::Result<()> {
    let mut read_buffer = [0u8; 1024];

    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer)?;

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
                                let mut db = database.lock().unwrap();

                                execute(&mut db, command)
                            };

                            let encoded = encode_response(&response);

                            println!("Encoded response: {encoded:?}");

                            stream.write_all(encoded.as_bytes())?;
                        }

                        Err(error) => {
                            println!("Command error: {error:?}");

                            let encoded = encode_error(&format!("{error:?}"));

                            stream.write_all(encoded.as_bytes())?;
                        }
                    }

                    receive_buffer.drain(..consumed);
                }

                Err(novadb_protocol::RespError::Incomplete) => {
                    break;
                }

                Err(error) => {
                    println!("Protocol error: {error:?}");

                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("{error:?}"),
                    ));
                }
            }
        }
    }

    Ok(())
}
