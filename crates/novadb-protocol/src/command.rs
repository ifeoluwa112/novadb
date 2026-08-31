use crate::RespValue;
use novadb_common::Command;

#[derive(Debug, PartialEq)]
pub enum CommandError {
    NotAnArray,
    EmptyArray,
    InvalidCommand,
    MissingArgument,
}

pub fn parse_command(value: RespValue) -> Result<Command, CommandError> {
    let values = match value {
        RespValue::Array(values) => values,
        _ => return Err(CommandError::NotAnArray),
    };

    if values.is_empty() {
        return Err(CommandError::EmptyArray);
    }

    let command_name = match &values[0] {
        RespValue::BulkString(value) => value.to_uppercase(),
        _ => return Err(CommandError::InvalidCommand),
    };

    match command_name.as_str() {
        "SET" => {
            if values.len() != 3 && values.len() != 4 {
                return Err(CommandError::MissingArgument);
            }

            let key = match &values[1] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            let value = match &values[2] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            let ttl = if values.len() == 4 {
                let ttl_string = match &values[3] {
                    RespValue::BulkString(value) => value,
                    _ => return Err(CommandError::InvalidCommand),
                };

                Some(
                    ttl_string
                        .parse::<u64>()
                        .map_err(|_| CommandError::InvalidCommand)?,
                )
            } else {
                None
            };

            Ok(Command::Set { key, value, ttl })
        }

        "GET" => {
            if values.len() != 2 {
                return Err(CommandError::MissingArgument);
            }

            let key = match &values[1] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            Ok(Command::Get { key })
        }

        "DELETE" => {
            if values.len() != 2 {
                return Err(CommandError::MissingArgument);
            }

            let key = match &values[1] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            Ok(Command::Delete { key })
        }

        "EXISTS" => {
            if values.len() != 2 {
                return Err(CommandError::MissingArgument);
            }

            let key = match &values[1] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            Ok(Command::Exists { key })
        }

        "KEYS" => {
            if values.len() != 1 {
                return Err(CommandError::MissingArgument);
            }

            Ok(Command::Keys)
        }

        "TTL" => {
            if values.len() != 2 {
                return Err(CommandError::MissingArgument);
            }

            let key = match &values[1] {
                RespValue::BulkString(value) => value.clone(),
                _ => return Err(CommandError::InvalidCommand),
            };

            Ok(Command::Ttl { key })
        }

        _ => Err(CommandError::InvalidCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set_command() {
        let input = b"*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$8\r\nPrecious\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Set {
                key: "name".to_string(),
                value: "Precious".to_string(),
                ttl: None,
            }
        );
    }

    #[test]
    fn parses_get_command() {
        let input = b"*2\r\n$3\r\nGET\r\n$4\r\nname\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Get {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_delete_command() {
        let input = b"*2\r\n$6\r\nDELETE\r\n$4\r\nname\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Delete {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_exists_command() {
        let input = b"*2\r\n$6\r\nEXISTS\r\n$4\r\nname\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Exists {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_keys_command() {
        let input = b"*1\r\n$4\r\nKEYS\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(command, Command::Keys);
    }

    #[test]
    fn parses_ttl_command() {
        let input = b"*2\r\n$3\r\nTTL\r\n$7\r\nsession\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Ttl {
                key: "session".to_string(),
            }
        );
    }

    #[test]
    fn parses_set_with_ttl() {
        let input = b"*4\r\n$3\r\nSET\r\n$7\r\nsession\r\n$6\r\nabc123\r\n$2\r\n60\r\n";

        let (resp, consumed) = crate::parse_resp(input).unwrap();

        assert_eq!(consumed, input.len());

        let command = parse_command(resp).unwrap();

        assert_eq!(
            command,
            Command::Set {
                key: "session".to_string(),
                value: "abc123".to_string(),
                ttl: Some(60),
            }
        );
    }

    #[test]
    fn rejects_invalid_set_ttl() {
        let input = b"*4\r\n$3\r\nSET\r\n$7\r\nsession\r\n$6\r\nabc123\r\n$6\r\nbanana\r\n";

        let (resp, _) = crate::parse_resp(input).unwrap();

        let result = parse_command(resp);

        assert!(matches!(result, Err(CommandError::InvalidCommand)));
    }
}
