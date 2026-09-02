use std::net::TcpListener;

pub fn start_server(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;

    println!("NovaDB listening on {}", address);

    loop {
        let (_stream, address) = listener.accept()?;

        println!("Client connected from {}", address);
    }
}