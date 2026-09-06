use std::collections::VecDeque;

pub const SESSION_LOG_CAPACITY: usize = 512;

#[derive(Clone)]
pub struct SessionLogSnapshot {
    pub lines: Vec<String>,
    pub sequence: u64,
}

pub struct SessionLogger {
    lines: VecDeque<String>,
    sequence: u64,
}

impl Default for SessionLogger {
    fn default() -> Self {
        Self {
            lines: VecDeque::with_capacity(SESSION_LOG_CAPACITY),
            sequence: 0,
        }
    }
}

impl SessionLogger {
    pub fn push(&mut self, line: String) {
        if self.lines.len() >= SESSION_LOG_CAPACITY {
            let _ = self.lines.pop_front();
        }
        self.lines.push_back(line);
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn snapshot(&self) -> SessionLogSnapshot {
        SessionLogSnapshot {
            lines: self.lines.iter().cloned().collect(),
            sequence: self.sequence,
        }
    }
}
