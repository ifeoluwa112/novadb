#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    SimpleString(String),
    BulkString(String),
    Integer(i64),
    Null,
}

pub fn encode_response(response: &Response) -> String {
    match response {
        Response::SimpleString(value) => format!("+{}\r\n", value),

        Response::BulkString(value) => format!("${}\r\n{}\r\n", value.len(), value),

        Response::Integer(value) => format!(":{}\r\n", value),

        Response::Null => "$-1\r\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_string() {
        let response = Response::SimpleString("OK".to_string());

        assert_eq!(encode_response(&response), "+OK\r\n");
    }

    #[test]
    fn encodes_bulk_string() {
        let response = Response::BulkString("Precious".to_string());

        assert_eq!(encode_response(&response), "$8\r\nPrecious\r\n");
    }

    #[test]
    fn encodes_integer() {
        let response = Response::Integer(42);

        assert_eq!(encode_response(&response), ":42\r\n");
    }

    #[test]
    fn encodes_negative_integer() {
        let response = Response::Integer(-2);

        assert_eq!(encode_response(&response), ":-2\r\n");
    }

    #[test]
    fn encodes_null() {
        let response = Response::Null;

        assert_eq!(encode_response(&response), "$-1\r\n");
    }
}
