use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

const CLIENTS: usize = 5;
const COMMANDS_PER_CLIENT: usize = 20;

fn main() {
    let mut handles = Vec::new();

    for client_id in 0..CLIENTS {
        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect("127.0.0.1:6379").unwrap();

            println!("Client {client_id} connected");

            for command_id in 0..COMMANDS_PER_CLIENT {
                let key = format!("client:{client_id}:key:{command_id}");

                let request = if command_id % 4 == 0 {
                    // WRITE
                    format!(
                        "*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n$5\r\nhello\r\n",
                        key.len(),
                        key
                    )
                } else if command_id % 4 == 1 {
                    // READ
                    format!(
                        "*2\r\n$3\r\nGET\r\n${}\r\n{}\r\n",
                        key.len(),
                        key
                    )
                } else if command_id % 4 == 2 {
                    // READ
                    format!(
                        "*2\r\n$6\r\nEXISTS\r\n${}\r\n{}\r\n",
                        key.len(),
                        key
                    )
                } else {
                    // READ
                    format!(
                        "*2\r\n$3\r\nTTL\r\n${}\r\n{}\r\n",
                        key.len(),
                        key
                    )
                };

                stream.write_all(request.as_bytes()).unwrap();

                let mut response = [0u8; 1024];

                let bytes_read = stream.read(&mut response).unwrap();

                println!(
                    "Client {client_id} command {command_id}: {}",
                    String::from_utf8_lossy(&response[..bytes_read])
                );
            }

            println!("Client {client_id} finished");
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Load test finished");
}