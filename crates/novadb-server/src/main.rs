use novadb_common::parse;
use novadb_server::executor::execute;
use novadb_storage::Database;

fn main() {
    let mut db = Database::new();

    execute(&mut db, parse("SET name Precious").unwrap());
    execute(&mut db, parse("SET language Rust").unwrap());

    execute(&mut db, parse("GET name").unwrap());
    execute(&mut db, parse("GET language").unwrap());

    execute(&mut db, parse("EXISTS name").unwrap());
    execute(&mut db, parse("EXISTS country").unwrap());

    execute(&mut db, parse("KEYS").unwrap());

    execute(&mut db, parse("DELETE language").unwrap());

    execute(&mut db, parse("GET language").unwrap());
}
