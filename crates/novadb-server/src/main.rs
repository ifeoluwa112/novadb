use novadb_common::parse;
use novadb_server::{execute_read, execute_write};
use novadb_storage::Database;

fn main() {
    let mut db = Database::new();
    println!(
        "{:?}",
        execute_write(&mut db, parse("SET name Precious").unwrap())
    );
    println!("{:?}", execute_read(&db, parse("GET name").unwrap()));
}
