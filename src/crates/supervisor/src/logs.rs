//! Ring buffer for sidecar output.

use ffg_core::redaction::redact_text;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub const DEFAULT_LOG_CAPACITY: usize = 2_000;
const MAX_LINE_LENGTH: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub stream: LogStream,
    pub line: String,
}

#[derive(Debug)]
pub struct LogBuffer {
    capacity: usize,
    lines: VecDeque<LogLine>,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_LOG_CAPACITY)
    }
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            lines: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    pub fn push(&mut self, stream: LogStream, line: &str) {
        let line = redact_text(line.trim_end(), &[]);
        let line = if line.len() > MAX_LINE_LENGTH {
            let mut truncated: String = line.chars().take(MAX_LINE_LENGTH).collect();
            truncated.push('…');
            truncated
        } else {
            line
        };
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(LogLine { stream, line });
    }

    pub fn tail(&self, limit: usize) -> Vec<LogLine> {
        let start = self.lines.len().saturating_sub(limit);
        self.lines.iter().skip(start).cloned().collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

pub async fn read_stream<R: AsyncRead + Unpin>(
    reader: R,
    stream: LogStream,
    logs: Arc<Mutex<LogBuffer>>,
) {
    let mut reader = BufReader::new(reader).lines();
    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                if let Ok(mut buffer) = logs.lock() {
                    buffer.push(stream, &line);
                }
            }
            Ok(None) => break,
            Err(error) => {
                if let Ok(mut buffer) = logs.lock() {
                    buffer.push(LogStream::Stderr, &format!("[log reader] {error}"));
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_configured_capacity() {
        let mut buffer = LogBuffer::new(2);
        buffer.push(LogStream::Stdout, "one");
        buffer.push(LogStream::Stdout, "two");
        buffer.push(LogStream::Stderr, "three");
        let lines = buffer.tail(10);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line, "two");
        assert_eq!(lines[1].stream, LogStream::Stderr);
    }

    #[test]
    fn redacts_bearer_tokens() {
        let mut buffer = LogBuffer::default();
        buffer.push(LogStream::Stdout, "Authorization: Bearer abc123");
        let line = &buffer.tail(1)[0].line;
        assert!(!line.contains("abc123"));
        assert!(line.contains("[REDACTED]"));
    }

    #[test]
    fn truncates_very_long_lines() {
        let mut buffer = LogBuffer::default();
        buffer.push(LogStream::Stdout, &"x".repeat(MAX_LINE_LENGTH + 10));
        assert!(buffer.tail(1)[0].line.ends_with('…'));
    }
}
