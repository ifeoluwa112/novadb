use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Read,
    Write,
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
    pub measurements: Vec<Measurement>,
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
}
