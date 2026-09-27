use novadb_common::{Command, Measurement, MeasurementCollector, Metric, Operation};
use std::sync::Mutex;

const REPORT_INTERVAL_SECS: u64 = 10;
const READ_LATENCY_P99_SLO: Duration = Duration::from_micros(100);
const WRITE_LATENCY_P99_SLO: Duration = Duration::from_micros(150);
const SLO: SloThresholds = SloThresholds {
    read_latency_p99: READ_LATENCY_P99_SLO,
    write_latency_p99: WRITE_LATENCY_P99_SLO,
};

use novadb_common::Response;
use novadb_common::{encode_error, encode_response};
use novadb_protocol::{parse_command, parse_resp};
use novadb_server::{execute_read, execute_write};
use novadb_storage::Database;

use std::println;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
struct PendingCommandTiming {
    wait: Duration,
    hold: Duration,
    latency_start: Instant,
}

#[derive(Debug)]
struct PendingCommand {
    response: Response,
    operation: Operation,
    timing: PendingCommandTiming,
}

struct SloThresholds {
    read_latency_p99: Duration,
    write_latency_p99: Duration,
}

fn execute_read_with_timing(
    db: &Database,
    command: Command,
    wait: Duration,
    latency_start: Instant,
) -> (Response, PendingCommandTiming) {
    let execute_start = Instant::now();

    let response = execute_read(db, command);

    let hold = execute_start.elapsed();

    let timing = PendingCommandTiming {
        wait,
        hold,
        latency_start,
    };

    (response, timing)
}

fn execute_write_with_timing(
    db: &mut Database,
    command: Command,
    wait: Duration,
    latency_start: Instant,
) -> (Response, PendingCommandTiming) {
    let execute_start = Instant::now();

    let response = execute_write(db, command);

    let hold = execute_start.elapsed();

    let timing = PendingCommandTiming {
        wait,
        hold,
        latency_start,
    };

    (response, timing)
}

fn spawn_metrics_reporter(
    read_metrics: Arc<Mutex<MeasurementCollector>>,
    write_metrics: Arc<Mutex<MeasurementCollector>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(REPORT_INTERVAL_SECS));

        loop {
            interval.tick().await;

            // Swap the whiteboard for a blank one, keep the old one just long
            // enough to read off its numbers. `take` leaves a fresh Default behind.
            let read_window = {
                let mut guard = read_metrics.lock().unwrap();
                std::mem::take(&mut *guard)
            };
            let write_window = {
                let mut guard = write_metrics.lock().unwrap();
                std::mem::take(&mut *guard)
            };

            report_window("READ", &read_window, SLO.read_latency_p99);
            report_window("WRITE", &write_window, SLO.write_latency_p99);
        }
    });
}

fn report_window(label: &str, window: &MeasurementCollector, latency_slo: Duration) {
    if window.len() == 0 {
        println!("[{label}] no commands in the last {REPORT_INTERVAL_SECS}s window");
        return;
    }

    for metric in [Metric::Wait, Metric::Hold, Metric::Latency] {
        if let Some(stats) = window.statistics(metric) {
            println!(
                "[{label}] {:?} | min={:?} max={:?} avg={:?} p50={:?} p95={:?} p99={:?}",
                stats.metric, stats.min, stats.max, stats.average, stats.p50, stats.p95, stats.p99
            );
        }
    }

    check_latency_slo(label, window, latency_slo);
}

fn check_latency_slo(label: &str, window: &MeasurementCollector, slo: Duration) {
    let Some(stats) = window.statistics(Metric::Latency) else {
        return;
    };

    if stats.p99 > slo {
        println!(
            "[{label}] SLO BREACH | latency p99={:?} exceeds promise of {:?}",
            stats.p99, slo
        );
    } else {
        println!(
            "[{label}] SLO OK | latency p99={:?} within promise of {:?}",
            stats.p99, slo
        );
    }
}

pub async fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let database = Arc::new(RwLock::new(Database::new()));
    let active_connections = Arc::new(AtomicUsize::new(0));
    let total_connections = Arc::new(AtomicUsize::new(0));

    // ONE shared notebook per operation type, on the manager's desk —
    // every customer (every connection) writes into these, not their own.
    let read_metrics = Arc::new(Mutex::new(MeasurementCollector::new()));
    let write_metrics = Arc::new(Mutex::new(MeasurementCollector::new()));

    // The background clerk who checks the whiteboard on a schedule.
    spawn_metrics_reporter(Arc::clone(&read_metrics), Arc::clone(&write_metrics));

    loop {
        let (stream, address) = listener.accept().await?;

        let current_active = active_connections.fetch_add(1, Ordering::Relaxed) + 1;

        let total_accepted = total_connections.fetch_add(1, Ordering::Relaxed) + 1;

        println!(
            "Client connected: {address} | active={} | total_accepted={}",
            current_active, total_accepted,
        );

        let database = Arc::clone(&database);
        let active_connections = Arc::clone(&active_connections);
        let read_metrics = Arc::clone(&read_metrics);
        let write_metrics = Arc::clone(&write_metrics);
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, database, read_metrics, write_metrics).await {
                println!("Client error: {error}");
            }

            let active = active_connections.fetch_sub(1, Ordering::Relaxed) - 1;

            println!("Client disconnected | active={}", active,);
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    database: Arc<RwLock<Database>>,
    read_metrics: Arc<Mutex<MeasurementCollector>>,
    write_metrics: Arc<Mutex<MeasurementCollector>>,
) -> std::io::Result<()> {
    let mut read_buffer = [0u8; 1024];
    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer).await?;
        if bytes_read == 0 {
            println!("Client disconnected");
            break; // no per-connection summary anymore — the clerk handles reporting now
        }

        receive_buffer.extend_from_slice(&read_buffer[..bytes_read]);

        loop {
            match parse_resp(&receive_buffer) {
                Ok((value, consumed)) => {
                    match parse_command(value) {
                        Ok(command) => {
                            let pending = match command {
                                Command::Get { .. }
                                | Command::Exists { .. }
                                | Command::Keys
                                | Command::Ttl { .. } => {
                                    let latency_start = Instant::now();
                                    let lock_start = Instant::now();
                                    let db = database.read().await;
                                    let wait_time = lock_start.elapsed();

                                    let (response, timing) = execute_read_with_timing(
                                        &db,
                                        command,
                                        wait_time,
                                        latency_start,
                                    );

                                    PendingCommand {
                                        response,
                                        operation: Operation::Read,
                                        timing,
                                    }
                                }

                                Command::Set { .. } | Command::Delete { .. } => {
                                    let latency_start = Instant::now();
                                    let lock_start = Instant::now();
                                    let mut db = database.write().await;
                                    let wait_time = lock_start.elapsed();

                                    let (response, timing) = execute_write_with_timing(
                                        &mut db,
                                        command,
                                        wait_time,
                                        latency_start,
                                    );

                                    PendingCommand {
                                        response,
                                        operation: Operation::Write,
                                        timing,
                                    }
                                }
                            };

                            let encoded = encode_response(&pending.response);
                            stream.write_all(encoded.as_bytes()).await?;

                            let latency = pending.timing.latency_start.elapsed();

                            let measurement = Measurement {
                                operation: pending.operation,
                                wait: pending.timing.wait,
                                hold: pending.timing.hold,
                                latency,
                            };

                            // Write into the SHARED notebook, not a private one.
                            match pending.operation {
                                Operation::Read => {
                                    read_metrics.lock().unwrap().record(measurement);
                                }
                                Operation::Write => {
                                    write_metrics.lock().unwrap().record(measurement);
                                }
                            }

                            receive_buffer.drain(..consumed);
                        }

                        Err(error) => {
                            let encoded = encode_error(&error.to_string());
                            stream.write_all(encoded.as_bytes()).await?;
                            receive_buffer.drain(..consumed);
                        }
                    }
                }

                Err(novadb_protocol::RespError::Incomplete) => break,

                Err(error) => {
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
