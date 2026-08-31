use std::usize;

#[derive(Debug, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    BulkString(String),
    Array(Vec<RespValue>),
}

#[derive(Debug, PartialEq)]
pub enum RespError {
    Incomplete,
    Invalid(String),
}

fn find_crlf(input: &[u8]) -> Option<usize> {
    input.windows(2).position(|window| window == b"\r\n")
}

fn parse_simple_string(input: &[u8]) -> Result<(RespValue, usize), RespError> {
    let Some(end) = find_crlf(input) else {
        return Err(RespError::Incomplete);
    };

    let bytes = &input[1..end];

    let value =
        std::str::from_utf8(bytes).map_err(|_| RespError::Invalid("invalid UTF-8".to_string()))?;

    Ok((RespValue::SimpleString(value.to_string()), end + 2))
}

fn parse_bulk_string(input: &[u8]) -> Result<(RespValue, usize), RespError> {
    let Some(header_end) = find_crlf(input) else {
        return Err(RespError::Incomplete);
    };

    let length_bytes = &input[1..header_end];

    let length_str = std::str::from_utf8(length_bytes)
        .map_err(|_| RespError::Invalid("invalid bulk string length".to_string()))?;

    let length = length_str
        .parse::<usize>()
        .map_err(|_| RespError::Invalid("invalid bulk string length".to_string()))?;

    let payload_start = header_end + 2;
    let payload_end = payload_start + length;

    if input.len() < payload_end + 2 {
        return Err(RespError::Incomplete);
    }

    if &input[payload_end..payload_end + 2] != b"\r\n" {
        return Err(RespError::Invalid("bulk string missing CRLF".to_string()));
    }

    let bytes = &input[payload_start..payload_end];

    let value =
        std::str::from_utf8(bytes).map_err(|_| RespError::Invalid("invalid UTF-8".to_string()))?;

    Ok((RespValue::BulkString(value.to_string()), payload_end + 2))
}

fn parse_array(input: &[u8]) -> Result<(RespValue, usize), RespError> {
    let Some(header_end) = find_crlf(input) else {
        return Err(RespError::Incomplete);
    };

    let count_bytes = &input[1..header_end];

    let count_str = std::str::from_utf8(count_bytes)
        .map_err(|_| RespError::Invalid("invalid array length".to_string()))?;

    let count = count_str
        .parse::<usize>()
        .map_err(|_| RespError::Invalid("invalid array length".to_string()))?;

    let mut values = Vec::with_capacity(count);

    let mut offset = header_end + 2;

    for _ in 0..count {
        let (value, consumed) = parse_resp(&input[offset..])?;

        values.push(value);

        offset += consumed;
    }

    Ok((RespValue::Array(values), offset))
}

pub fn parse_resp(input: &[u8]) -> Result<(RespValue, usize), RespError> {
    if input.is_empty() {
        return Err(RespError::Incomplete);
    }

    match input[0] {
        b'+' => parse_simple_string(input),
        b'$' => parse_bulk_string(input),
        b'*' => parse_array(input),
        _ => Err(RespError::Invalid("unknown RESP type".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_set_command() {
        let input = b"*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$8\r\nPrecious\r\n";

        let (value, consumed) = parse_resp(input).unwrap();

        println!("VALUE: {value:?}");
        println!("CONSUMED: {consumed}");

        assert_eq!(consumed, input.len());
    }
}
