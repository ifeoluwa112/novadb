use crate::Command;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    EmptyCommand,
    UnknownCommand(String),
    MissingArgument,
}

pub fn parse(input: &str) -> Result<Command, ParseError> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    match parts[0].to_uppercase().as_str() {
        "SET" => {
            if parts.len() != 3 && parts.len() != 4 {
                return Err(ParseError::MissingArgument);
            }

            let key = parts[1].to_string();
            let value = parts[2].to_string();

            let ttl = if parts.len() == 4 {
                let seconds = parts[3]
                    .parse::<u64>()
                    .map_err(|_| ParseError::UnknownCommand(parts[3].to_string()))?;

                Some(seconds)
            } else {
                None
            };

            Ok(Command::Set { key, value, ttl })
        }

        "GET" => {
            if parts.len() != 2 {
                return Err(ParseError::MissingArgument);
            }

            Ok(Command::Get {
                key: parts[1].to_string(),
            })
        }

        "DELETE" => {
            if parts.len() != 2 {
                return Err(ParseError::MissingArgument);
            }

            Ok(Command::Delete {
                key: parts[1].to_string(),
            })
        }

        "EXISTS" => {
            if parts.len() != 2 {
                return Err(ParseError::MissingArgument);
            }

            Ok(Command::Exists {
                key: parts[1].to_string(),
            })
        }

        "KEYS" => {
            if parts.len() != 1 {
                return Err(ParseError::MissingArgument);
            }

            Ok(Command::Keys)
        }

        "TTL" => {
            if parts.len() != 2 {
                return Err(ParseError::MissingArgument);
            }

            Ok(Command::Ttl {
                key: parts[1].to_string(),
            })
        }

        other => Err(ParseError::UnknownCommand(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set() {
        let command = parse("SET name Precious").unwrap();

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
    fn parses_set_with_ttl() {
        let command = parse("SET session abc123 60").unwrap();

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
    fn parses_get() {
        let command = parse("GET name").unwrap();

        assert_eq!(
            command,
            Command::Get {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_delete() {
        let command = parse("DELETE name").unwrap();

        assert_eq!(
            command,
            Command::Delete {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_exists() {
        let command = parse("EXISTS name").unwrap();

        assert_eq!(
            command,
            Command::Exists {
                key: "name".to_string(),
            }
        );
    }

    #[test]
    fn parses_keys() {
        let command = parse("KEYS").unwrap();

        assert_eq!(command, Command::Keys);
    }

    #[test]
    fn parses_ttl() {
        let command = parse("TTL session").unwrap();

        assert_eq!(
            command,
            Command::Ttl {
                key: "session".to_string(),
            }
        );
    }

    #[test]
    fn rejects_invalid_ttl() {
        let result = parse("SET session abc123 banana");

        assert!(matches!(result, Err(ParseError::UnknownCommand(_))));
    }

    #[test]
    fn rejects_unknown_command() {
        let result = parse("BANANA name");

        assert!(matches!(result, Err(ParseError::UnknownCommand(_))));
    }

    #[test]
    fn rejects_missing_argument() {
        let result = parse("GET").unwrap_err();

        assert_eq!(result, ParseError::MissingArgument);
    }

    #[test]
    fn rejects_empty_command() {
        let result = parse("").unwrap_err();

        assert_eq!(result, ParseError::EmptyCommand);
    }
}
