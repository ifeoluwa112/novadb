use novadb_common::Command;
use novadb_common::Response;
use novadb_common::{encode_error, encode_response};
use novadb_protocol::{parse_command, parse_resp};
use novadb_server::{execute_read, execute_write};
use novadb_storage::Database;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
enum Operation {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
struct CommandTiming {
    wait: Duration,
    hold: Duration,
}

#[derive(Debug, Clone, Copy)]
struct CollectedTiming {
    _operation: Operation,
    _timing: CommandTiming,
}

#[derive(Debug, Default)]
struct ConnectionTimingCollector {
    measurements: Vec<CollectedTiming>,
}

impl ConnectionTimingCollector {
    fn new() -> Self {
        Self {
            measurements: Vec::new(),
        }
    }
    fn record(&mut self, measurement: CollectedTiming) {
        self.measurements.push(measurement);
    }

    fn len(&self) -> usize {
        self.measurements.len()
    }

    fn finish(self) -> Vec<CollectedTiming> {
        self.measurements
    }
}

fn execute_read_with_timing(
    db: &Database,
    command: Command,
    wait: Duration,
) -> (Response, CommandTiming) {
    let execute_start = Instant::now();

    let response = execute_read(db, command);

    let hold = execute_start.elapsed();

    let timing = CommandTiming { wait, hold };

    (response, timing)
}

fn execute_write_with_timing(
    db: &mut Database,
    command: Command,
    wait: Duration,
) -> (Response, CommandTiming) {
    let execute_start = Instant::now();

    let response = execute_write(db, command);
    let hold = execute_start.elapsed();

    let timing = CommandTiming { wait, hold };

    (response, timing)
}

pub async fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let database = Arc::new(RwLock::new(Database::new()));

    loop {
        let (stream, address) = listener.accept().await?;

        println!("Client connected: {address}");

        let database = Arc::clone(&database);

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, database).await {
                println!("Client error: {error}");
            }
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    database: Arc<RwLock<Database>>,
) -> std::io::Result<()> {
    let mut collector = ConnectionTimingCollector::new();
    let mut read_buffer = [0u8; 1024];

    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            println!("Client disconnected");

            let measurements = collector.finish();

            println!("Collected {} timing measurements", measurements.len());

            break;
        }

        receive_buffer.extend_from_slice(&read_buffer[..bytes_read]);

        loop {
            match parse_resp(&receive_buffer) {
                Ok((value, consumed)) => {
                    println!("Parsed RESP value: {value:?}");

                    match parse_command(value) {
                        Ok(command) => {
                            println!("Parsed command: {command:?}");

                            let response = match command {
                                Command::Get { .. }
                                | Command::Exists { .. }
                                | Command::Keys
                                | Command::Ttl { .. } => {
                                    let lock_start = Instant::now();
                                    let db = database.read().await;
                                    let wait_time = lock_start.elapsed();

                                    let (response, timing) =
                                        execute_read_with_timing(&db, command, wait_time);
                                    collector.record(CollectedTiming {
                                        _operation: Operation::Read,
                                        _timing: timing,
                                    });

                                    println!(
                                        "READ  | wait={:?} | hold={:?}",
                                        timing.wait, timing.hold
                                    );
                                    response
                                }

                                Command::Set { .. } | Command::Delete { .. } => {
                                    let lock_start = Instant::now();
                                    let mut db = database.write().await;
                                    let wait_time = lock_start.elapsed();
                                    let (response, timing) =
                                        execute_write_with_timing(&mut db, command, wait_time);

                                    collector.record(CollectedTiming {
                                        _operation: Operation::Write,
                                        _timing: timing,
                                    });

                                    println!(
                                        "WRITE | wait={:?} | hold={:?}",
                                        timing.wait, timing.hold
                                    );

                                    response
                                }
                            };

                            let encoded = encode_response(&response);

                            println!("Encoded response: {encoded:?}");

                            stream.write_all(encoded.as_bytes()).await?;
                        }

                        Err(error) => {
                            println!("Command error: {error:?}");

                            let encoded = encode_error(&error.to_string());

                            stream.write_all(encoded.as_bytes()).await?;
                        }
                    }

                    receive_buffer.drain(..consumed);
                }

                Err(novadb_protocol::RespError::Incomplete) => {
                    break;
                }

                Err(error) => {
                    println!("Protocol error: {error:?}");

                    let encoded = encode_error(&error.to_string());

                    stream.write_all(encoded.as_bytes()).await?;

                    receive_buffer.clear();
                    break;
                }
            }
        }
    }

    Ok(())
}
