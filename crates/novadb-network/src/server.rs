use novadb_common::Command;
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
enum Operation {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
struct CommandTiming {
    wait: Duration,
    hold: Duration,
    latency: Duration,
}

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

#[derive(Debug, Clone, Copy)]
struct CollectedTiming {
    operation: Operation,
    timing: CommandTiming,
}

#[derive(Debug)]
struct ConnectionSummary {
    total: usize,
    read_count: usize,
    write_count: usize,

    read_wait_total: Duration,
    read_hold_total: Duration,
    read_latency_total: Duration,

    write_wait_total: Duration,
    write_hold_total: Duration,
    write_latency_total: Duration,

    read_wait_average: Duration,
    read_hold_average: Duration,
    read_latency_average: Duration,

    write_wait_average: Duration,
    write_hold_average: Duration,
    write_latency_average: Duration,

    read_wait_min_max: Option<(Duration, Duration)>,
    read_hold_min_max: Option<(Duration, Duration)>,
    read_latency_min_max: Option<(Duration, Duration)>,

    write_wait_min_max: Option<(Duration, Duration)>,
    write_hold_min_max: Option<(Duration, Duration)>,
    write_latency_min_max: Option<(Duration, Duration)>,

    read_wait_percentiles: DurationPercentiles,
    read_hold_percentiles: DurationPercentiles,
    read_latency_percentiles: DurationPercentiles,

    write_wait_percentiles: DurationPercentiles,
    write_hold_percentiles: DurationPercentiles,
    write_latency_percentiles: DurationPercentiles,
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

    fn _len(&self) -> usize {
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

fn average_duration(total: Duration, count: usize) -> Duration {
    if count == 0 {
        Duration::ZERO
    } else {
        total / count as u32
    }
}

fn duration_min_max(values: &[Duration]) -> Option<(Duration, Duration)> {
    let mut iter = values.iter().copied();

    let first = iter.next()?;

    let mut min = first;
    let mut max = first;

    for value in iter {
        min = min.min(value);
        max = max.max(value);
    }

    Some((min, max))
}

fn duration_percentile(values: &[Duration], percentile: f64) -> Option<Duration> {
    if values.is_empty() {
        return None;
    }

    assert!(
        (0.0..=1.0).contains(&percentile),
        "percentile must be between 0.0 and 1.0"
    );

    let mut sorted = values.to_vec();
    sorted.sort_unstable();

    let rank = (percentile * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1);

    sorted.get(index).copied()
}

fn print_duration_percentiles(label: &str, percentiles: DurationPercentiles) {
    println!(
        "{label} | p50={:?} | p95={:?} | p99={:?}",
        percentiles.p50, percentiles.p95, percentiles.p99,
    );
}

fn summarize_connection(measurements: &[CollectedTiming]) -> ConnectionSummary {
    let mut read_count = 0;
    let mut write_count = 0;

    let mut read_wait_total = Duration::ZERO;
    let mut write_wait_total = Duration::ZERO;

    let mut read_hold_total = Duration::ZERO;
    let mut write_hold_total = Duration::ZERO;

    let mut read_latency_total = Duration::ZERO;
    let mut write_latency_total = Duration::ZERO;

    let mut read_wait_values = Vec::new();
    let mut read_hold_values = Vec::new();
    let mut write_wait_values = Vec::new();
    let mut write_hold_values = Vec::new();
    let mut read_latency_values = Vec::new();
    let mut write_latency_values = Vec::new();

    for measurement in measurements {
        match measurement.operation {
            Operation::Read => {
                read_count += 1;

                read_wait_total += measurement.timing.wait;
                read_hold_total += measurement.timing.hold;
                read_latency_total += measurement.timing.latency;

                read_wait_values.push(measurement.timing.wait);
                read_hold_values.push(measurement.timing.hold);
                read_latency_values.push(measurement.timing.latency);
            }

            Operation::Write => {
                write_count += 1;

                write_wait_total += measurement.timing.wait;
                write_hold_total += measurement.timing.hold;
                write_latency_total += measurement.timing.latency;

                write_wait_values.push(measurement.timing.wait);
                write_hold_values.push(measurement.timing.hold);
                write_latency_values.push(measurement.timing.latency);
            }
        }
    }

    let read_wait_average = average_duration(read_wait_total, read_count);
    let read_hold_average = average_duration(read_hold_total, read_count);
    let read_latency_average = average_duration(read_latency_total, read_count);

    let write_wait_average = average_duration(write_wait_total, write_count);
    let write_hold_average = average_duration(write_hold_total, write_count);
    let write_latency_average = average_duration(write_latency_total, write_count);

    let read_wait_min_max = duration_min_max(&read_wait_values);
    let read_hold_min_max = duration_min_max(&read_hold_values);
    let read_latency_min_max = duration_min_max(&read_latency_values);

    let write_wait_min_max = duration_min_max(&write_wait_values);
    let write_hold_min_max = duration_min_max(&write_hold_values);
    let write_latency_min_max = duration_min_max(&write_latency_values);

    let read_wait_percentiles = duration_percentiles(&read_wait_values);
    let read_hold_percentiles = duration_percentiles(&read_hold_values);
    let read_latency_percentiles = duration_percentiles(&read_latency_values);

    let write_wait_percentiles = duration_percentiles(&write_wait_values);
    let write_hold_percentiles = duration_percentiles(&write_hold_values);
    let write_latency_percentiles = duration_percentiles(&write_latency_values);

    ConnectionSummary {
        total: measurements.len(),
        read_count,
        write_count,

        read_wait_total,
        read_hold_total,
        read_latency_total,

        write_wait_total,
        write_hold_total,
        write_latency_total,

        read_wait_average,
        read_hold_average,
        read_latency_average,

        write_wait_average,
        write_hold_average,
        write_latency_average,

        read_wait_min_max,
        read_hold_min_max,
        read_latency_min_max,

        write_wait_min_max,
        write_hold_min_max,
        write_latency_min_max,

        read_wait_percentiles,
        read_hold_percentiles,
        read_latency_percentiles,

        write_wait_percentiles,
        write_hold_percentiles,
        write_latency_percentiles,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DurationPercentiles {
    p50: Option<Duration>,
    p95: Option<Duration>,
    p99: Option<Duration>,
}

fn duration_percentiles(values: &[Duration]) -> DurationPercentiles {
    DurationPercentiles {
        p50: duration_percentile(values, 0.50),
        p95: duration_percentile(values, 0.95),
        p99: duration_percentile(values, 0.99),
    }
}

pub async fn run() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    println!("NovaDB listening on 127.0.0.1:6379");

    let database = Arc::new(RwLock::new(Database::new()));
    let active_connections = Arc::new(AtomicUsize::new(0));
    let total_connections = Arc::new(AtomicUsize::new(0));

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
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, database).await {
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
) -> std::io::Result<()> {
    let mut collector = ConnectionTimingCollector::new();
    let mut read_buffer = [0u8; 1024];
    let mut receive_buffer = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer).await?;
        if bytes_read == 0 {
            println!("Client disconnected");

            let measurements = collector.finish();

            let summary = summarize_connection(&measurements);
            println!(
                "Connection summary | total_commands={} | reads={} | writes={}",
                summary.total, summary.read_count, summary.write_count,
            );
            println!(
                "Read timing  | wait_total={:?} | hold_total={:?} | latency_total={:?}",
                summary.read_wait_total, summary.read_hold_total, summary.read_latency_total,
            );

            println!(
                "Read average | wait={:?} | hold={:?} | latency={:?}",
                summary.read_wait_average, summary.read_hold_average, summary.read_latency_average,
            );
            println!(
                "Write timing | wait_total={:?} | hold_total={:?} | latency_total={:?}",
                summary.write_wait_total, summary.write_hold_total, summary.write_latency_total,
            );

            println!(
                "Write average | wait={:?} | hold={:?} | latency={:?}",
                summary.write_wait_average,
                summary.write_hold_average,
                summary.write_latency_average,
            );

            println!("Read wait min/max: {:?}", summary.read_wait_min_max);
            println!("Read hold min/max: {:?}", summary.read_hold_min_max);
            println!("Read latency min/max: {:?}", summary.read_latency_min_max);

            println!("Write wait min/max: {:?}", summary.write_wait_min_max);
            println!("Write hold min/max: {:?}", summary.write_hold_min_max);
            println!("Write latency min/max: {:?}", summary.write_latency_min_max);

            print_duration_percentiles("Read wait percentiles", summary.read_wait_percentiles);

            print_duration_percentiles("Write wait percentiles", summary.write_wait_percentiles);

            print_duration_percentiles("Read hold percentiles", summary.read_hold_percentiles);

            print_duration_percentiles("Write hold percentiles", summary.write_hold_percentiles);

            print_duration_percentiles(
                "Read latency percentiles",
                summary.read_latency_percentiles,
            );

            print_duration_percentiles(
                "Write latency percentiles",
                summary.write_latency_percentiles,
            );

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
                                    println!(
                                        "READ  | wait={:?} | hold={:?}",
                                        timing.wait, timing.hold
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
                                    println!(
                                        "WRITE | wait={:?} | hold={:?}",
                                        timing.wait, timing.hold
                                    );

                                    PendingCommand {
                                        response,
                                        operation: Operation::Write,
                                        timing,
                                    }
                                }
                            };

                            let encoded = encode_response(&pending.response);

                            println!("Encoded response: {encoded:?}");

                            stream.write_all(encoded.as_bytes()).await?;

                            let latency = pending.timing.latency_start.elapsed();

                            let timing = CommandTiming {
                                wait: pending.timing.wait,
                                hold: pending.timing.hold,
                                latency,
                            };

                            collector.record(CollectedTiming {
                                operation: pending.operation,
                                timing,
                            });

                            println!(
                                "COMPLETED | operation={:?} | wait={:?} | hold={:?} | latency={:?}",
                                pending.operation, timing.wait, timing.hold, timing.latency,
                            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{assert_eq, time::Duration};

    #[test]
    fn percentile_returns_none_for_empty_values() {
        let values: Vec<Duration> = Vec::new();

        assert_eq!(duration_percentile(&values, 0.50), None);
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let values = vec![
            Duration::from_millis(8),
            Duration::from_millis(2),
            Duration::from_millis(12),
            Duration::from_millis(5),
            Duration::from_millis(3),
        ];

        assert_eq!(
            duration_percentile(&values, 0.50),
            Some(Duration::from_millis(5))
        );

        assert_eq!(
            duration_percentile(&values, 0.95),
            Some(Duration::from_millis(12))
        );
    }

    #[test]
    fn percentile_does_not_modify_original_values() {
        let values = vec![
            Duration::from_millis(8),
            Duration::from_millis(2),
            Duration::from_millis(5),
        ];

        let original = values.clone();

        let _ = duration_percentile(&values, 0.50);

        assert_eq!(values, original);
    }

    #[test]
    fn percentiles_return_expected_values() {
        let values = vec![
            Duration::from_millis(8),
            Duration::from_millis(2),
            Duration::from_millis(12),
            Duration::from_millis(5),
            Duration::from_millis(3),
        ];

        let percentiles = duration_percentiles(&values);

        assert_eq!(
            percentiles,
            DurationPercentiles {
                p50: Some(Duration::from_millis(5)),
                p95: Some(Duration::from_millis(12)),
                p99: Some(Duration::from_millis(12)),
            }
        );
    }

    #[test]
    fn percentiles_return_none_for_empty_values() {
        let values: Vec<Duration> = Vec::new();

        assert_eq!(
            duration_percentiles(&values),
            DurationPercentiles {
                p50: None,
                p95: None,
                p99: None,
            }
        );
    }

    #[test]
    fn validates_masurements_summary() {
        let mut collector = ConnectionTimingCollector::new();

        collector.record(CollectedTiming {
            operation: Operation::Read,
            timing: CommandTiming {
                wait: Duration::from_micros(10),
                hold: Duration::from_micros(2),
                latency: Duration::from_micros(20),
            },
        });
        collector.record(CollectedTiming {
            operation: Operation::Read,
            timing: CommandTiming {
                wait: Duration::from_micros(10),
                hold: Duration::from_micros(2),
                latency: Duration::from_micros(20),
            },
        });
        collector.record(CollectedTiming {
            operation: Operation::Write,
            timing: CommandTiming {
                wait: Duration::from_micros(10),
                hold: Duration::from_micros(2),
                latency: Duration::from_micros(20),
            },
        });

        let measurements = collector.finish();

        let summary = summarize_connection(&measurements);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.read_count, 2);
        assert_eq!(summary.write_count, 1);
        assert_eq!(summary.read_wait_total, Duration::from_micros(20));
        assert_eq!(summary.write_latency_total, Duration::from_micros(20));
        assert_eq!(summary.read_wait_average, Duration::from_micros(10));
        assert_eq!(summary.write_hold_average, Duration::from_micros(2));
        assert_eq!(
            summary.read_latency_min_max,
            Some((Duration::from_micros(20), Duration::from_micros(20)))
        );
        assert_eq!(
            summary.write_latency_min_max,
            Some((Duration::from_micros(20), Duration::from_micros(20)))
        );
        assert_eq!(
            summary.read_wait_percentiles.p50,
            Some(Duration::from_micros(10))
        );
    }
}
