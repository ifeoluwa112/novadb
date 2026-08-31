use novadb_common::parse;
use novadb_server::executor::execute;
use novadb_storage::Database;

fn main() {
    let mut db = Database::new();
    println!(
        "{:?}",
        execute(&mut db, parse("SET name Precious").unwrap())
    );
    println!("{:?}", execute(&mut db, parse("Get name").unwrap()));

}
