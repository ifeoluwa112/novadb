use std::io::Read;
use std::net::TcpListener;

use novadb_protocol::{parse_command, parse_resp};

pub fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379")?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let (mut stream, address) = listener.accept()?;

    println!("Client connected: {address}");

    let mut read_buffer = [0u8; 1024];
    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer)?;

        if bytes_read == 0 {
            println!("Client disconnected");
            break;
        }

        receive_buffer.extend_from_slice(&read_buffer[..bytes_read]);

        match parse_resp(&receive_buffer) {
            Ok((value, consumed)) => {
                println!("Parsed RESP value: {value:?}");

                match parse_command(value) {
                    Ok(command) => {
                        println!("Parsed command: {command:?}");
                    }

                    Err(error) => {
                        println!("Command error: {error:?}");
                    }
                }

                receive_buffer.drain(..consumed);
            }

            Err(novadb_protocol::RespError::Incomplete) => {
                println!("Waiting for more data...");
            }

            Err(error) => {
                println!("Protocol error: {error:?}");
                break;
            }
        }
    }

    Ok(())
}
