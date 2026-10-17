use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;

const MAX_CONNECTIONS: usize = 16;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const POOL_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BULK_LEN: usize = 1024 * 1024;

#[derive(Debug)]
#[allow(unused)]
pub enum NovaError {
    Io(std::io::Error),
    Protocol(String),
    Timeout,
}

impl From<std::io::Error> for NovaError {
    fn from(error: std::io::Error) -> Self {
        NovaError::Io(error)
    }
}

#[derive(Debug)]
#[allow(unused)]
enum Reply {
    Simple(String),
    Bulk(String),
    Integer(i64),
    Null,
    Error(String),
}

type Conn = BufReader<TcpStream>;

struct Inner {
    addr: String,
    idle: Mutex<Vec<Conn>>, // only ever locked briefly, never across an .await
    permits: Semaphore,     // caps total connections in use
}

/// A small connection pool speaking RESP to NovaDB.
#[derive(Clone)]
pub struct NovaClient {
    inner: Arc<Inner>,
}

impl NovaClient {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                addr: addr.into(),
                idle: Mutex::new(Vec::new()),
                permits: Semaphore::new(MAX_CONNECTIONS),
            }),
        }
    }

    pub async fn set_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_secs: u64,
    ) -> Result<(), NovaError> {
        let ttl = ttl_secs.to_string();
        match self.execute(&["SET", key, value, &ttl]).await? {
            Reply::Simple(_) => Ok(()),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to SET: {other:?}"
            ))),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, NovaError> {
        match self.execute(&["GET", key]).await? {
            Reply::Bulk(value) => Ok(Some(value)),
            Reply::Null => Ok(None),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to GET: {other:?}"
            ))),
        }
    }

    pub async fn delete(&self, key: &str) -> Result<bool, NovaError> {
        match self.execute(&["DELETE", key]).await? {
            Reply::Integer(count) => Ok(count > 0),
            other => Err(NovaError::Protocol(format!(
                "unexpected reply to DELETE: {other:?}"
            ))),
        }
    }

    async fn execute(&self, parts: &[&str]) -> Result<Reply, NovaError> {
        let request = encode_command(parts);

        // Wait (briefly) for a free slot; if the pool is saturated, fail fast.
        let _permit = tokio::time::timeout(POOL_WAIT_TIMEOUT, self.inner.permits.acquire())
            .await
            .map_err(|_| NovaError::Timeout)?
            .map_err(|_| NovaError::Protocol("connection pool closed".into()))?;

        // Try a reused connection first. It may have gone stale (NovaDB restarted),
        // so on an I/O error we retry exactly once on a fresh connection.
        // That retry is only safe because SET/GET/DELETE are idempotent.
        let reused = self.inner.idle.lock().unwrap().pop();
        if let Some(conn) = reused {
            match self.run_on(conn, &request).await {
                Ok(reply) => return Ok(reply),
                Err(NovaError::Io(_)) => {} // stale socket: fall through to a fresh one
                Err(other) => return Err(other),
            }
        }

        let conn = connect(&self.inner.addr).await?;
        self.run_on(conn, &request).await
    }

    /// Runs one command. The connection goes back to the pool only if the whole
    /// exchange succeeded; on any error it is dropped, because a half-read
    /// connection would corrupt the next caller's reply.
    async fn run_on(&self, mut conn: Conn, request: &[u8]) -> Result<Reply, NovaError> {
        let outcome = tokio::time::timeout(COMMAND_TIMEOUT, async {
            conn.get_mut().write_all(request).await?;
            read_reply(&mut conn).await
        })
        .await;

        match outcome {
            Err(_elapsed) => Err(NovaError::Timeout),
            Ok(Err(error)) => Err(error),
            Ok(Ok(reply)) => {
                self.inner.idle.lock().unwrap().push(conn);
                Ok(reply)
            }
        }
    }
}

async fn connect(addr: &str) -> Result<Conn, NovaError> {
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| NovaError::Timeout)??;
    stream.set_nodelay(true)?; // don't let the OS hold tiny messages back to batch them
    Ok(BufReader::new(stream))
}

fn encode_command(parts: &[&str]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", parts.len());
    for part in parts {
        out.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }
    out.into_bytes()
}

/// Reads exactly one RESP reply, consuming exactly its bytes and no more,
/// which is what makes it safe to reuse the connection afterward.
async fn read_reply(conn: &mut Conn) -> Result<Reply, NovaError> {
    let mut raw = String::new();
    let bytes = conn.read_line(&mut raw).await?;
    if bytes == 0 {
        return Err(NovaError::Io(std::io::ErrorKind::UnexpectedEof.into()));
    }
    let line = raw.trim_end_matches(|c: char| c == '\r' || c == '\n');

    match line.chars().next() {
        Some('+') => Ok(Reply::Simple(line[1..].to_string())),
        Some('-') => Ok(Reply::Error(line[1..].to_string())),
        Some(':') => line[1..]
            .parse::<i64>()
            .map(Reply::Integer)
            .map_err(|_| NovaError::Protocol("bad integer reply".into())),
        Some('$') => {
            let length: i64 = line[1..]
                .parse()
                .map_err(|_| NovaError::Protocol("bad bulk string length".into()))?;
            if length < 0 {
                return Ok(Reply::Null);
            }
            let length = length as usize;
            if length > MAX_BULK_LEN {
                return Err(NovaError::Protocol("bulk string too large".into()));
            }
            let mut payload = vec![0u8; length + 2];
            conn.read_exact(&mut payload).await?;
            if &payload[length..] != b"\r\n" {
                return Err(NovaError::Protocol("bulk string missing CRLF".into()));
            }
            payload.truncate(length);
            String::from_utf8(payload.to_vec())
                .map(Reply::Bulk)
                .map_err(|_| NovaError::Protocol("bulk string was not valid UTF-8".into()))
        }
        _ => Err(NovaError::Protocol("unknown reply type".into())),
    }
}
