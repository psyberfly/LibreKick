use std::{
    collections::VecDeque,
    sync::{LazyLock, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const LOG_CAPACITY: usize = 2048;

#[derive(Clone, Copy)]
enum LogType {
    Entering,
    Leaving,
    Debug,
    Error,
}

impl LogType {
    fn as_str(self) -> &'static str {
        match self {
            LogType::Entering => "ENTER",
            LogType::Leaving => "LEAVE",
            LogType::Debug => "DEBUG",
            LogType::Error => "ERROR",
        }
    }
}

pub struct LogSnapshot {
    pub lines: Vec<String>,
    pub sequence: u64,
}

struct LoggerState {
    lines: VecDeque<String>,
    sequence: u64,
}

pub struct Logger {
    state: Mutex<LoggerState>,
}

impl Logger {
    fn new() -> Self {
        Self {
            state: Mutex::new(LoggerState {
                lines: VecDeque::with_capacity(LOG_CAPACITY),
                sequence: 0,
            }),
        }
    }

    fn push_line(&self, log_type: LogType, message: String) {
        if let Ok(mut state) = self.state.lock() {
            if state.lines.len() >= LOG_CAPACITY {
                let _ = state.lines.pop_front();
            }
            let ts_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            state
                .lines
                .push_back(format!("[{ts_ms}] [{}] {message}", log_type.as_str()));
            state.sequence = state.sequence.wrapping_add(1);
        }
    }

    pub fn entering(&self, function_name: &str, params: impl Into<String>) {
        self.push_line(
            LogType::Entering,
            format!("{function_name} params={} ", params.into()),
        );
    }

    pub fn leaving(&self, function_name: &str) {
        self.push_line(LogType::Leaving, function_name.to_owned());
    }

    pub fn debug(&self, message: impl Into<String>) {
        self.push_line(LogType::Debug, message.into());
    }

    pub fn error(&self, message: impl Into<String>) {
        self.push_line(LogType::Error, message.into());
    }

    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.lines.clear();
            state.sequence = state.sequence.wrapping_add(1);
        }
    }

    pub fn snapshot(&self) -> LogSnapshot {
        if let Ok(state) = self.state.lock() {
            return LogSnapshot {
                lines: state.lines.iter().cloned().collect(),
                sequence: state.sequence,
            };
        }

        LogSnapshot {
            lines: Vec::new(),
            sequence: 0,
        }
    }
}

pub static LOGGER: LazyLock<Logger> = LazyLock::new(Logger::new);
