use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;

/// Time spent sending traffic before we start counting, so connection setup
/// and cold caches don't pollute the numbers.
const WARMUP: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
enum Scenario {
    Me,
    Listings,
    Login,
}

struct Config {
    scenario: Scenario,
    base: String,
    token: Option<String>,
    email: String,
    password: String,
}

enum Outcome {
    Status(u16),
    TransportError(String),
}

struct Sample {
    started_at: Instant,
    latency: Duration,
    outcome: Outcome,
}

fn usage() -> ! {
    eprintln!("usage: novadb-loadgen <me|listings|login> <concurrency> <seconds>");
    eprintln!("env:   BASE_URL (default http://127.0.0.1:3000), LOAD_EMAIL, LOAD_PASSWORD");
    std::process::exit(2);
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

async fn login_for_token(client: &Client, cfg_base: &str, email: &str, password: &str) -> String {
    let response = client
        .post(format!("{cfg_base}/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("could not reach the API; is it running?");

    if !response.status().is_success() {
        panic!("login failed with status {}", response.status());
    }

    let body: serde_json::Value = response.json().await.expect("login returned invalid JSON");
    body["token"]
        .as_str()
        .expect("login response had no token")
        .to_string()
}

async fn one_request(client: &Client, cfg: &Config) -> Outcome {
    let request = match cfg.scenario {
        Scenario::Me => client
            .get(format!("{}/me", cfg.base))
            .bearer_auth(cfg.token.as_deref().unwrap_or("")),
        Scenario::Listings => client.get(format!("{}/listings?limit=20", cfg.base)),
        Scenario::Login => client
            .post(format!("{}/login", cfg.base))
            .json(&serde_json::json!({ "email": cfg.email, "password": cfg.password })),
    };

    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            // The request isn't finished until the whole body has arrived.
            match response.bytes().await {
                Ok(_) => Outcome::Status(status),
                Err(error) => Outcome::TransportError(error.to_string()),
            }
        }
        Err(error) => Outcome::TransportError(error.to_string()),
    }
}

/// Nearest-rank percentile: the same method as your metrics.rs.
fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn mean(values: &[Duration]) -> Duration {
    let total: f64 = values.iter().map(|d| d.as_secs_f64()).sum();
    Duration::from_secs_f64(total / values.len() as f64)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        usage();
    }

    let scenario = match args[1].as_str() {
        "me" => Scenario::Me,
        "listings" => Scenario::Listings,
        "login" => Scenario::Login,
        _ => usage(),
    };
    let concurrency: usize = args[2].parse().unwrap_or_else(|_| usage());
    let seconds: u64 = args[3].parse().unwrap_or_else(|_| usage());

    let base = env_or("BASE_URL", "http://127.0.0.1:3000");
    let email = env_or("LOAD_EMAIL", "zed@mail.com");
    let password = env_or("LOAD_PASSWORD", "correct-horse-battery");

    let client = Client::builder()
        .pool_max_idle_per_host(concurrency)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    let token = match scenario {
        Scenario::Me => Some(login_for_token(&client, &base, &email, &password).await),
        _ => None,
    };

    let cfg = Arc::new(Config {
        scenario,
        base,
        token,
        email,
        password,
    });

    let start = Instant::now();
    let measure_from = start + WARMUP;
    let deadline = measure_from + Duration::from_secs(seconds);

    let mut workers = Vec::new();
    for _ in 0..concurrency {
        let client = client.clone();
        let cfg = Arc::clone(&cfg);
        workers.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            while Instant::now() < deadline {
                let started_at = Instant::now();
                let outcome = one_request(&client, &cfg).await;
                samples.push(Sample {
                    started_at,
                    latency: started_at.elapsed(),
                    outcome,
                });
            }
            samples
        }));
    }

    let mut samples = Vec::new();
    for worker in workers {
        samples.extend(worker.await.expect("worker panicked"));
    }
    samples.retain(|sample| sample.started_at >= measure_from);

    // ---- report ----
    let total = samples.len();
    if total == 0 {
        println!("no requests completed inside the measurement window");
        return;
    }

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut ok_latencies = Vec::new();
    let mut all_latencies = Vec::new();
    let mut first_transport_error: Option<String> = None;

    for sample in &samples {
        all_latencies.push(sample.latency);
        match &sample.outcome {
            Outcome::Status(code) => {
                *counts.entry(code.to_string()).or_default() += 1;
                if (200..300).contains(code) {
                    ok_latencies.push(sample.latency);
                }
            }
            Outcome::TransportError(message) => {
                *counts.entry("transport error".to_string()).or_default() += 1;
                if first_transport_error.is_none() {
                    first_transport_error = Some(message.clone());
                }
            }
        }
    }

    let throughput = total as f64 / seconds as f64;
    println!("scenario={} concurrency={concurrency} measured_for={seconds}s", args[1]);
    println!(
        "requests={total}  throughput={throughput:.0} req/s  successful={:.0} req/s",
        ok_latencies.len() as f64 / seconds as f64
    );

    print!("outcomes:");
    for (outcome, count) in &counts {
        print!("  {outcome}={count}");
    }
    println!();
    if let Some(message) = first_transport_error {
        println!("first transport error: {message}");
    }

    println!(
        "sanity check (Little's law): concurrency / throughput = {:?}, measured mean over all requests = {:?}",
        Duration::from_secs_f64(concurrency as f64 / throughput),
        mean(&all_latencies)
    );

    if ok_latencies.is_empty() {
        println!("no successful responses, so no latency percentiles");
        return;
    }
    ok_latencies.sort_unstable();
    println!(
        "latency (successful only): min={:?} mean={:?} p50={:?} p95={:?} p99={:?} max={:?}",
        ok_latencies[0],
        mean(&ok_latencies),
        percentile(&ok_latencies, 0.50),
        percentile(&ok_latencies, 0.95),
        percentile(&ok_latencies, 0.99),
        ok_latencies[ok_latencies.len() - 1],
    );
}