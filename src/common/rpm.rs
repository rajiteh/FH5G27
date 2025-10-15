use crate::common::telemetry::TelemetryParser;

#[derive(Default)]
pub struct RPM {
    current: f32,
    max: f32,
    idle: f32,
    staleness: u8,
    is_race_active: bool,
}

impl RPM {
    const STALENESS_THRESHOLD: u8 = 5;

    pub fn new() -> Self {
        RPM {
            ..Default::default()
        }
    }

    fn increment_staleness(&mut self) {
        if self.staleness < Self::STALENESS_THRESHOLD {
            self.staleness += 1;
        }
    }

    fn reset_staleness(&mut self) {
        if self.staleness != 0 {
            self.staleness = 0;
        }
    }

    pub fn is_stale(&self) -> bool {
        self.staleness >= Self::STALENESS_THRESHOLD
    }

    pub fn state(&self) -> (f32, f32, f32) {
        (self.current, self.max, self.idle)
    }

    pub fn update(&mut self, data: &[u8], parser: &dyn TelemetryParser) {
        let (current, max, idle, is_race_active) = parser.parse_rpm_data(data);
        
        if (self.current, self.max, self.idle, self.is_race_active) == (current, max, idle, is_race_active) {
            self.increment_staleness();
        } else {
            self.reset_staleness();
            self.current = current;
            self.max = max;
            self.idle = idle;
            self.is_race_active = is_race_active;
        }
    }

    pub fn is_race_active(&self) -> bool {
        self.is_race_active
    }
}
