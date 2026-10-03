use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

const CLIENTS: usize = 100;
const COMMANDS_PER_CLIENT: usize = 20;

// The entire point: every single client, every single command, touches
// this one exact key. No diversity at all — this is the adversarial case
// sharding can't help with, by construction.
const HOT_KEY: &str = "trending_post";

fn main() {
    let mut handles = Vec::new();

    for client_id in 0..CLIENTS {
        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect("127.0.0.1:6379").unwrap();

            for command_id in 0..COMMANDS_PER_CLIENT {
                // Mostly reads, with occasional writes — like everyone
                // refreshing a popular page while a few updates land.
                let request = if command_id % 5 == 0 {
                    format!(
                        "*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n$5\r\nhello\r\n",
                        HOT_KEY.len(),
                        HOT_KEY
                    )
                } else {
                    format!("*2\r\n$3\r\nGET\r\n${}\r\n{}\r\n", HOT_KEY.len(), HOT_KEY)
                };

                stream.write_all(request.as_bytes()).unwrap();

                let mut response = [0u8; 1024];
                let _ = stream.read(&mut response).unwrap();
            }

            println!("Client {client_id} finished hammering the hot key");
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Hot key test finished");
}
