use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
pub enum Metric {
    Wait,
    Hold,
    Latency,
}
#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub min: Duration,
    pub average: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}
#[derive(Debug, Clone, Copy)]
pub struct Measurement {
    pub operation: Operation,
    pub wait: Duration,
    pub hold: Duration,
    pub latency: Duration,
}

#[derive(Debug, Default)]
pub struct MeasurementCollector {
    measurements: Vec<Measurement>,
}

impl MeasurementCollector {
    pub fn new() -> Self {
        Self {
            measurements: Vec::new(),
        }
    }

    pub fn record(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    pub fn len(&self) -> usize {
        self.measurements.len()
    }

    pub fn metric_values(&self, metric: Metric) -> Vec<Duration> {
        self.measurements
            .iter()
            .map(|measurement| match metric {
                Metric::Wait => measurement.wait,
                Metric::Hold => measurement.hold,
                Metric::Latency => measurement.latency,
            })
            .collect()
    }
    pub fn percentile(&self, metric: Metric, percent: f64) -> Option<Duration> {
        let values = self.metric_values(metric);

        percentile(&values, percent)
    }

    pub fn statistics(&self) -> Option<Statistics> {
        if self.measurements.is_empty() {
            return None;
        }

        let min = self
            .measurements
            .iter()
            .map(|measurement| measurement.wait)
            .min()
            .unwrap();

        let total: Duration = self
            .measurements
            .iter()
            .map(|measurement| measurement.wait)
            .sum();

        let average = total / self.measurements.len() as u32;

        let wait_times = self.metric_values(Metric::Wait);

        let p50 = percentile(&wait_times, 0.50).unwrap();
        let p95 = percentile(&wait_times, 0.95).unwrap();
        let p99 = percentile(&wait_times, 0.99).unwrap();

        Some(Statistics {
            min,
            average,
            p50,
            p95,
            p99,
        })
    }
}

pub fn percentile(values: &[Duration], percentile: f64) -> Option<Duration> {
    if values.is_empty() {
        return None;
    }

    assert!(
        (0.0..=1.0).contains(&percentile),
        "percentile must be between 0.0 and 1.0"
    );

    let mut sorted = values.to_vec();

    sorted.sort();

    let rank = (percentile * sorted.len() as f64).ceil() as usize;

    let index = rank.saturating_sub(1);

    Some(sorted[index])
}
#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn measurement_can_represent_a_read() {
        let measurement = Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(10),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(20),
        };

        assert!(matches!(measurement.operation, Operation::Read));
    }

    #[test]
    fn collector_records_measurements() {
        let mut collector = MeasurementCollector::new();

        let measurement = Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(10),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(20),
        };

        collector.record(measurement);

        assert_eq!(collector.len(), 1);
    }

    #[test]
    fn collector_calculates_min_and_average_wait() {
        let mut collector = MeasurementCollector::new();

        collector.record(Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(10),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(20),
        });

        collector.record(Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(20),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(30),
        });

        collector.record(Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(30),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(40),
        });

        let statistics = collector.statistics().unwrap();

        assert_eq!(statistics.min, Duration::from_micros(10));
        assert_eq!(statistics.average, Duration::from_micros(20));
    }

    #[test]
    fn empty_collector_has_no_statistics() {
        let collector = MeasurementCollector::new();

        assert!(collector.statistics().is_none());
    }

    #[test]
    fn percentile_returns_expected_value() {
        let values = vec![
            Duration::from_micros(50),
            Duration::from_micros(10),
            Duration::from_micros(30),
            Duration::from_micros(20),
            Duration::from_micros(40),
        ];

        let result = percentile(&values, 0.50);

        assert_eq!(result, Some(Duration::from_micros(30)));
    }

    #[test]
    fn percentile_returns_none_for_empty_values() {
        let values: Vec<Duration> = Vec::new();

        assert_eq!(percentile(&values, 0.50), None);
    }

    #[test]
    fn collector_extracts_wait_times() {
        let mut collector = MeasurementCollector::new();

        collector.record(Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(10),
            hold: Duration::from_micros(2),
            latency: Duration::from_micros(20),
        });

        collector.record(Measurement {
            operation: Operation::Write,
            wait: Duration::from_micros(50),
            hold: Duration::from_micros(3),
            latency: Duration::from_micros(60),
        });

        let wait_times = collector.metric_values(Metric::Wait);

        assert_eq!(
            wait_times,
            vec![Duration::from_micros(10), Duration::from_micros(50),]
        );
    }

    #[test]
    fn collector_extracts_metric_values() {
        let mut collector = MeasurementCollector::new();

        collector.record(Measurement {
            operation: Operation::Read,
            wait: Duration::from_micros(10),
            hold: Duration::from_micros(2),
            latency: Duration::from_micros(20),
        });

        collector.record(Measurement {
            operation: Operation::Write,
            wait: Duration::from_micros(50),
            hold: Duration::from_micros(5),
            latency: Duration::from_micros(70),
        });

        assert_eq!(
            collector.metric_values(Metric::Wait),
            vec![Duration::from_micros(10), Duration::from_micros(50),]
        );

        assert_eq!(
            collector.metric_values(Metric::Hold),
            vec![Duration::from_micros(2), Duration::from_micros(5),]
        );

        assert_eq!(
            collector.metric_values(Metric::Latency),
            vec![Duration::from_micros(20), Duration::from_micros(70),]
        );
    }

    #[test]
    fn collector_calculates_percentile_for_metric() {
        let mut collector = MeasurementCollector::new();

        for wait in [10, 20, 30, 40, 50] {
            collector.record(Measurement {
                operation: Operation::Read,
                wait: Duration::from_micros(wait),
                hold: Duration::from_micros(2),
                latency: Duration::from_micros(20),
            });
        }

        assert_eq!(
            collector.percentile(Metric::Wait, 0.50),
            Some(Duration::from_micros(30))
        );

        assert_eq!(
            collector.percentile(Metric::Wait, 0.95),
            Some(Duration::from_micros(50))
        );

        assert_eq!(
            collector.percentile(Metric::Wait, 0.99),
            Some(Duration::from_micros(50))
        );
    }
}
