use std::time::Duration;

use novadb_common::Command;
use novadb_storage::Database;

pub fn execute(db: &mut Database, command: Command) {
    match command {
        Command::Set { key, value, ttl } => match ttl {
            Some(seconds) => {
                db.set_with_ttl(&key, &value, Duration::from_secs(seconds));
            }

            None => {
                db.set(&key, &value);
            }
        },
        Command::Get { key } => {
            let value = db.get(&key);

            println!("GET {key} -> {value:?}");
        }

        Command::Delete { key } => {
            let result = db.delete(&key);

            println!("DELETE {key} -> {result:?}");
        }

        Command::Exists { key } => {
            let exists = db.exists(&key);

            println!("EXISTS {key} -> {exists}");
        }

        Command::Keys => {
            let keys: Vec<&String> = db.keys().collect();

            println!("KEYS -> {keys:?}");
        }

        Command::Ttl { key } => {
            let ttl = db.ttl(&key);

            println!("TTL {key} -> {ttl}");
        }
    }
}
