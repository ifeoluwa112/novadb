use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug)]
pub enum NovaError {
    Io(std::io::Error),
    Protocol(String),
}

impl From<std::io::Error> for NovaError {
    fn from(error: std::io::Error) -> Self {
        NovaError::Io(error)
    }
}

impl std::fmt::Display for NovaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NovaError::Io(error) => write!(f, "I/O error: {error}"),
            NovaError::Protocol(message) => write!(f, "protocol error: {message}"),
        }
    }
}

impl std::error::Error for NovaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NovaError::Io(error) => Some(error),
            NovaError::Protocol(_) => None,
        }
    }
}

#[derive(Debug)]
enum Reply {
    #[allow(dead_code)]
    Simple(String),
    Bulk(String),
    Integer(i64),
    Null,
    #[allow(dead_code)]
    Error(String),
}

/// Talks to NovaDB over its RESP protocol: the same wire format your
/// server parses, now written from the client side.
#[derive(Clone)]
pub struct NovaClient {
    addr: String,
}

impl NovaClient {
    pub fn new(addr: impl Into<String>) -> Self {
        Self { addr: addr.into() }
    }

    pub async fn set_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_secs: u64,
    ) -> Result<(), NovaError> {
        let ttl = ttl_secs.to_string();
        match self.send(&["SET", key, value, &ttl]).await? {
            Reply::Simple(_) => Ok(()),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to SET: {other:?}"
            ))),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, NovaError> {
        match self.send(&["GET", key]).await? {
            Reply::Bulk(value) => Ok(Some(value)),
            Reply::Null => Ok(None),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to GET: {other:?}"
            ))),
        }
    }

    pub async fn delete(&self, key: &str) -> Result<bool, NovaError> {
        match self.send(&["DELETE", key]).await? {
            Reply::Integer(count) => Ok(count > 0),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to DELETE: {other:?}"
            ))),
        }
    }

    /// Encode a command as a RESP array of bulk strings, send it, read one reply.
    async fn send(&self, parts: &[&str]) -> Result<Reply, NovaError> {
        let mut request = format!("*{}\r\n", parts.len());
        for part in parts {
            request.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
        }

        let mut stream = TcpStream::connect(&self.addr).await?;
        stream.write_all(request.as_bytes()).await?;

        let mut buffer = [0u8; 1024];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Err(NovaError::Protocol("NovaDB closed the connection".into()));
        }

        parse_reply(&buffer[..bytes_read])
    }
}

fn parse_reply(bytes: &[u8]) -> Result<Reply, NovaError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| NovaError::Protocol("reply was not valid UTF-8".into()))?;

    let line_end = text
        .find("\r\n")
        .ok_or_else(|| NovaError::Protocol("incomplete reply".into()))?;
    let first_line = &text[..line_end];
    let rest = &text.as_bytes()[line_end + 2..];

    match first_line.chars().next() {
        Some('+') => Ok(Reply::Simple(first_line[1..].to_string())),
        Some('-') => Ok(Reply::Error(first_line[1..].to_string())),
        Some(':') => first_line[1..]
            .parse::<i64>()
            .map(Reply::Integer)
            .map_err(|_| NovaError::Protocol("bad integer reply".into())),
        Some('$') => {
            if first_line == "$-1" {
                return Ok(Reply::Null);
            }
            let length: usize = first_line[1..]
                .parse()
                .map_err(|_| NovaError::Protocol("bad bulk string length".into()))?;
            let payload = rest
                .get(..length)
                .ok_or_else(|| NovaError::Protocol("incomplete bulk string".into()))?;
            String::from_utf8(payload.to_vec())
                .map(Reply::Bulk)
                .map_err(|_| NovaError::Protocol("bulk string was not valid UTF-8".into()))
        }
        _ => Err(NovaError::Protocol("unknown reply type".into())),
    }
}
