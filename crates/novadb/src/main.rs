use novadb_common::{encode_response, parse};
use novadb_server::execute;
use novadb_storage::Database;

fn run_command(db: &mut Database, input: &str) {
    let command = parse(input).unwrap();
    let response = execute(db, command);
    let encoded = encode_response(&response);

    println!("{encoded}");
}

fn main() {
    let mut db = Database::new();

    run_command(&mut db, "SET name Precious");
    run_command(&mut db, "GET name");
    run_command(&mut db, "EXISTS name");
    run_command(&mut db, "TTL name");
    run_command(&mut db, "DELETE name");
    run_command(&mut db, "GET name");

    run_command(&mut db, "SET session abc123 2");
    run_command(&mut db, "GET session");
    run_command(&mut db, "TTL session");

    std::thread::sleep(std::time::Duration::from_secs(3));

    run_command(&mut db, "GET session");
    run_command(&mut db, "TTL session");
}
