use std::time::Duration;

use novadb_common::{Command, Response};
use novadb_storage::Database;

pub fn execute(db: &mut Database, command: Command) -> Response {
    match command {
        Command::Set { key, value, ttl } => {
            match ttl {
                Some(seconds) => {
                    db.set_with_ttl(&key, &value, Duration::from_secs(seconds));
                }

                None => {
                    db.set(&key, &value);
                }
            }

            Response::SimpleString("OK".to_string())
        }
        Command::Get { key } => match db.get(&key) {
            Some(value) => Response::BulkString(value.to_string()),
            None => Response::Null,
        },

        Command::Delete { key } => {
            let result = db.delete(&key);

            if result.is_some() {
                Response::Integer(1)
            } else {
                Response::Integer(0)
            }
        }

        Command::Exists { key } => {
            let exists = db.exists(&key);

            if exists {
                Response::Integer(1)
            } else {
                Response::Integer(0)
            }
        }

        Command::Keys => {
            let keys: Vec<&String> = db.keys().collect();

            let value = keys.into_iter().cloned().collect::<Vec<String>>().join(" ");

            Response::BulkString(value)
        }

        Command::Ttl { key } => Response::Integer(db.ttl(&key)),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn set_returns_ok() {
        let mut db = Database::new();

        let response = execute(
            &mut db,
            Command::Set {
                key: "name".to_string(),
                value: "Precious".to_string(),
                ttl: None,
            },
        );

        assert_eq!(response, Response::SimpleString("OK".to_string()));
    }

    #[test]
    fn get_returns_value() {
        let mut db = Database::new();

        db.set("name", "Precious");

        let response = execute(
            &mut db,
            Command::Get {
                key: "name".to_string(),
            },
        );

        assert_eq!(response, Response::BulkString("Precious".to_string()));
    }

    #[test]
    fn get_missing_key_returns_null() {
        let mut db = Database::new();

        let response = execute(
            &mut db,
            Command::Get {
                key: "missing".to_string(),
            },
        );

        assert_eq!(response, Response::Null);
    }

    #[test]
    fn exists_returns_one_for_existing_key() {
        let mut db = Database::new();

        db.set("name", "Precious");

        let response = execute(
            &mut db,
            Command::Exists {
                key: "name".to_string(),
            },
        );

        assert_eq!(response, Response::Integer(1));
    }

    #[test]
    fn exists_returns_zero_for_missing_key() {
        let mut db = Database::new();

        let response = execute(
            &mut db,
            Command::Exists {
                key: "name".to_string(),
            },
        );

        assert_eq!(response, Response::Integer(0));
    }

    #[test]
    fn key_has_no_ttl() {
        let mut db = Database::new();
        db.set("name", "Precious");

        let response = execute(
            &mut db,
            Command::Ttl {
                key: "name".to_string(),
            },
        );
        assert_eq!(response, Response::Integer(-1));
    }
}
