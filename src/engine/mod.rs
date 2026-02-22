use std::net::IpAddr;
use std::time::Duration;

use thiserror::Error;

pub mod mock;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod unix_surge;

pub type SequenceNumber = u64;
pub type EngineTime = Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRequest {
    pub seq: SequenceNumber,
    pub target: IpAddr,
    pub sent_at: EngineTime,
    pub payload_size: usize,
    pub ttl: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingReply {
    pub seq: SequenceNumber,
    pub ttl: Option<u8>,
    pub payload_size: usize,
    pub source: Option<IpAddr>,
}

impl PingReply {
    #[must_use]
    pub fn for_seq(seq: SequenceNumber) -> Self {
        Self {
            seq,
            ttl: None,
            payload_size: 0,
            source: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PingEvent {
    Reply(PingReply),
    Interrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEvent {
    pub at: EngineTime,
    pub event: PingEvent,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PingEngineError {
    #[error("send failed for seq {seq}: {message}")]
    SendFailed {
        seq: SequenceNumber,
        message: String,
    },
    #[error("poll failed: {message}")]
    PollFailed { message: String },
    #[error("invalid engine timeline: poll deadline moved backwards")]
    NonMonotonicPoll,
    #[error("invalid probe request for seq {seq}: {message}")]
    InvalidProbeRequest {
        seq: SequenceNumber,
        message: String,
    },
}

pub trait PingEngine {
    fn now(&self) -> EngineTime;

    fn send_probe(&mut self, request: ProbeRequest) -> Result<(), PingEngineError>;

    /// Polls for the next event up to `deadline`.
    ///
    /// Implementations must keep time monotonic. If there is an event at or before
    /// `deadline`, return all events for that exact timestamp and advance `now()` to
    /// that timestamp. If no event exists before `deadline`, advance to `deadline` and
    /// return an empty vector.
    fn poll_until(&mut self, deadline: EngineTime) -> Result<Vec<TimedEvent>, PingEngineError>;
}
