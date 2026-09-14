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
