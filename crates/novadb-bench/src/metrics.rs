use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Read,
    Write,
}
#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub min: Duration,
    pub average: Duration,
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

        Some(Statistics { min, average })
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
}
