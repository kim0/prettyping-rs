use std::collections::BTreeMap;
use std::time::Duration;

use super::{EngineTime, PingEngine, PingEngineError, ProbeRequest, SequenceNumber, TimedEvent};

#[derive(Debug, Clone, Default)]
pub struct MockEngine {
    now: EngineTime,
    sent: Vec<ProbeRequest>,
    events: BTreeMap<EngineTime, Vec<TimedEvent>>,
    send_failures: BTreeMap<SequenceNumber, String>,
}

impl MockEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_now(now: Duration) -> Self {
        Self {
            now,
            ..Self::default()
        }
    }

    pub fn queue_event(&mut self, event: TimedEvent) {
        self.events.entry(event.at).or_default().push(event);
    }

    pub fn queue_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = TimedEvent>,
    {
        for event in events {
            self.queue_event(event);
        }
    }

    pub fn fail_send_for_seq(&mut self, seq: SequenceNumber, message: impl Into<String>) {
        self.send_failures.insert(seq, message.into());
    }

    #[must_use]
    pub fn sent_requests(&self) -> &[ProbeRequest] {
        &self.sent
    }

    #[must_use]
    pub fn now_time(&self) -> EngineTime {
        self.now
    }
}

impl PingEngine for MockEngine {
    fn now(&self) -> EngineTime {
        self.now
    }

    fn send_probe(&mut self, request: ProbeRequest) -> Result<(), PingEngineError> {
        if request.sent_at != self.now {
            return Err(PingEngineError::InvalidProbeRequest {
                seq: request.seq,
                message: format!(
                    "sent_at {:?} does not match engine now {:?}",
                    request.sent_at, self.now
                ),
            });
        }

        if let Some(message) = self.send_failures.remove(&request.seq) {
            return Err(PingEngineError::SendFailed {
                seq: request.seq,
                message,
            });
        }

        self.sent.push(request);
        Ok(())
    }

    fn poll_until(&mut self, deadline: EngineTime) -> Result<Vec<TimedEvent>, PingEngineError> {
        if deadline < self.now {
            return Err(PingEngineError::NonMonotonicPoll);
        }

        let selected_time = self
            .events
            .range(self.now..=deadline)
            .next()
            .map(|(event_time, _)| *event_time);

        match selected_time {
            Some(event_time) => {
                self.now = event_time;
                Ok(self.events.remove(&event_time).unwrap_or_default())
            }
            None => {
                self.now = deadline;
                Ok(Vec::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::MockEngine;
    use crate::engine::{PingEngine, PingEvent, PingReply, TimedEvent};

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn poll_until_ignores_stale_events_and_keeps_time_monotonic() {
        let mut engine = MockEngine::with_now(ms(1_000));
        engine.queue_event(TimedEvent {
            at: ms(900),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        });

        let events = engine
            .poll_until(ms(1_500))
            .expect("poll should succeed without rewinding");

        assert!(events.is_empty(), "stale event must not be consumed");
        assert_eq!(engine.now_time(), ms(1_500), "now must remain monotonic");
    }
}
