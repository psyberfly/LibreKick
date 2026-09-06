use crate::config;

#[derive(Clone, Copy)]
pub struct Voice {
    sample_rate: f32,
    is_active: bool,
    time_seconds: f32,
    note_hz: f32,
    velocity: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            sample_rate: config::app_config().default_sample_rate, 
            is_active: false,
            time_seconds: 0.0,
            note_hz: 55.0,
            velocity: 1.0,
        }
    }
}

impl Voice {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn note_on(
        &mut self,
        note_hz: f32,
        velocity: f32,
    ) {
        self.is_active = true;
        self.time_seconds = 0.0;
        self.note_hz = note_hz.max(20.0);
        self.velocity = velocity.clamp(0.0, 1.0);
    }

    pub fn note_off(&mut self) {
        self.is_active = false;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn advance(&mut self) {
        self.time_seconds += 1.0 / self.sample_rate;
    }

    pub fn reset_time(&mut self) {
        self.time_seconds = 0.0;
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    pub fn note_hz(&self) -> f32 {
        self.note_hz
    }

    pub fn velocity(&self) -> f32 {
        self.velocity
    }
}