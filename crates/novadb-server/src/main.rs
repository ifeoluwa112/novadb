use std::{thread, time::Duration};

use novadb_common::parse;
use novadb_server::executor::execute;
use novadb_storage::Database;

fn main() {
    let mut db = Database::new();
    execute(&mut db, parse("SET session abc123 2").unwrap());

    execute(&mut db, parse("GET session").unwrap());

    execute(&mut db, parse("TTL session").unwrap());

    thread::sleep(Duration::from_secs(3));

    execute(&mut db, parse("GET session").unwrap());

    execute(&mut db, parse("TTL session").unwrap());
}
