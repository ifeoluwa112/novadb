use novadb_common::{encode_response, parse};
use novadb_server::execute;
use novadb_storage::Database;
use novadb_network::start_server;

fn _run_command(db: &mut Database, input: &str) {
    let command = parse(input).unwrap();
    let response = execute(db, command);
    let encoded = encode_response(&response);

    println!("{encoded}");
}

fn main() -> std::io::Result<()> {
    start_server("127.0.0.1:6379")?;
    Ok(())
}
