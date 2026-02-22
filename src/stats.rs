use crate::app::AppEvent;
use crate::ring_buffer::RingBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerStats {
    pub count: u64,
    pub min_ms: u32,
    pub avg_ms: u32,
    pub max_ms: u32,
    pub mdev_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossStats {
    pub lost: u64,
    pub total: u64,
    pub percent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalStatsSnapshot {
    pub loss: LossStats,
    pub rtt: IntegerStats,
    pub last_rtt_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentStatsSnapshot {
    pub loss: LossStats,
    pub rtt: IntegerStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub global: Option<GlobalStatsSnapshot>,
    pub recent: Option<RecentStatsSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    total_received: u64,
    total_lost: u64,
    total_rtt_sum: u64,
    min_rtt_ms: Option<u32>,
    max_rtt_ms: Option<u32>,
    last_rtt_ms: u32,
    recent: RingBuffer<RecentSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecentSample {
    Reply(u32),
    Timeout,
}

impl Stats {
    #[must_use]
    pub fn new(last: u32) -> Self {
        let capacity = usize::try_from(last).unwrap_or(usize::MAX);
        Self {
            total_received: 0,
            total_lost: 0,
            total_rtt_sum: 0,
            min_rtt_ms: None,
            max_rtt_ms: None,
            last_rtt_ms: 0,
            recent: RingBuffer::with_capacity(capacity),
        }
    }

    pub fn apply(&mut self, event: &AppEvent) {
        match event {
            AppEvent::ProbeReply {
                rtt_ms, duplicate, ..
            } => {
                if *duplicate {
                    return;
                }
                let value = u32::try_from(*rtt_ms).unwrap_or(u32::MAX);
                self.total_received = self.total_received.saturating_add(1);
                self.total_rtt_sum = self.total_rtt_sum.saturating_add(u64::from(value));
                self.last_rtt_ms = value;

                self.min_rtt_ms = Some(match self.min_rtt_ms {
                    Some(current) => current.min(value),
                    None => value,
                });

                self.max_rtt_ms = Some(match self.max_rtt_ms {
                    Some(current) => current.max(value),
                    None => value,
                });

                let _ = self.recent.push(RecentSample::Reply(value));
            }
            AppEvent::ProbeTimeout { .. } => {
                self.total_lost = self.total_lost.saturating_add(1);
                let _ = self.recent.push(RecentSample::Timeout);
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn snapshot(&self, include_global: bool, include_recent: bool) -> StatsSnapshot {
        let global = include_global.then(|| self.global_snapshot());
        let recent = include_recent.then(|| self.recent_snapshot());
        StatsSnapshot { global, recent }
    }

    #[must_use]
    pub fn global_snapshot(&self) -> GlobalStatsSnapshot {
        let loss = loss_stats(
            self.total_lost,
            self.total_lost.saturating_add(self.total_received),
        );
        let (min_ms, avg_ms, max_ms, mdev_ms) = if self.total_received == 0 {
            (0, 0, 0, 0)
        } else {
            let min = self.min_rtt_ms.unwrap_or(0);
            let max = self.max_rtt_ms.unwrap_or(0);
            let avg = div_u64_to_u32(self.total_rtt_sum, self.total_received);
            let mdev = average_abs_deviation(recent_rtt_values(self.recent.iter()));
            (min, avg, max, mdev)
        };

        GlobalStatsSnapshot {
            loss,
            rtt: IntegerStats {
                count: self.total_received,
                min_ms,
                avg_ms,
                max_ms,
                mdev_ms,
            },
            last_rtt_ms: self.last_rtt_ms,
        }
    }

    #[must_use]
    pub fn recent_snapshot(&self) -> RecentStatsSnapshot {
        let (lost_sum, total, recent_rtt_values) = fold_recent(self.recent.iter());
        let loss = loss_stats(lost_sum, total);

        let rtt = integer_stats_from_values(&recent_rtt_values);

        RecentStatsSnapshot { loss, rtt }
    }
}

fn fold_recent<'a, I>(samples: I) -> (u64, u64, Vec<u32>)
where
    I: IntoIterator<Item = &'a RecentSample>,
{
    let mut lost_sum = 0u64;
    let mut total = 0u64;
    let mut rtt_values = Vec::new();

    for sample in samples {
        total = total.saturating_add(1);
        match sample {
            RecentSample::Reply(value) => rtt_values.push(*value),
            RecentSample::Timeout => lost_sum = lost_sum.saturating_add(1),
        }
    }

    (lost_sum, total, rtt_values)
}

fn recent_rtt_values<'a, I>(samples: I) -> impl Iterator<Item = u32> + 'a
where
    I: IntoIterator<Item = &'a RecentSample> + 'a,
{
    samples.into_iter().filter_map(|sample| match sample {
        RecentSample::Reply(value) => Some(*value),
        RecentSample::Timeout => None,
    })
}

fn integer_stats_from_values(values: &[u32]) -> IntegerStats {
    if values.is_empty() {
        return IntegerStats {
            count: 0,
            min_ms: 0,
            avg_ms: 0,
            max_ms: 0,
            mdev_ms: 0,
        };
    }

    let mut min = u32::MAX;
    let mut max = 0u32;
    let mut sum = 0u64;
    for value in values {
        min = min.min(*value);
        max = max.max(*value);
        sum = sum.saturating_add(u64::from(*value));
    }

    let count = u64::try_from(values.len()).unwrap_or(u64::MAX);
    let avg = div_u64_to_u32(sum, count);
    let mdev = average_abs_deviation(values.iter().copied());

    IntegerStats {
        count,
        min_ms: min,
        avg_ms: avg,
        max_ms: max,
        mdev_ms: mdev,
    }
}

fn loss_stats(lost: u64, total: u64) -> LossStats {
    let percent = if total == 0 {
        0
    } else {
        div_u64_to_u32(lost.saturating_mul(100), total)
    };
    LossStats {
        lost,
        total,
        percent,
    }
}

fn average_abs_deviation<I>(values: I) -> u32
where
    I: IntoIterator<Item = u32>,
{
    let data: Vec<u32> = values.into_iter().collect();
    if data.is_empty() {
        return 0;
    }

    let count = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let sum = data
        .iter()
        .fold(0u64, |acc, value| acc.saturating_add(u64::from(*value)));
    let avg = i64::from(div_u64_to_u32(sum, count));

    let total_abs_diff = data.iter().fold(0u64, |acc, value| {
        let diff = (i64::from(*value) - avg).unsigned_abs();
        acc.saturating_add(diff)
    });

    div_u64_to_u32(total_abs_diff, count)
}

fn div_u64_to_u32(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    let quotient = numerator / denominator;
    u32::try_from(quotient).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::app::AppEvent;

    use super::Stats;

    #[test]
    fn tracks_integer_stats_with_recent_window() {
        let mut stats = Stats::new(3);

        stats.apply(&AppEvent::ProbeReply {
            seq: 1,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(9),
            rtt_ms: 9,
            duplicate: false,
            late: false,
        });
        stats.apply(&AppEvent::ProbeReply {
            seq: 2,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(20),
            rtt_ms: 20,
            duplicate: false,
            late: false,
        });
        stats.apply(&AppEvent::ProbeTimeout {
            seq: 3,
            sent_at: Duration::ZERO,
            deadline: Duration::from_millis(100),
        });
        stats.apply(&AppEvent::ProbeReply {
            seq: 4,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(51),
            rtt_ms: 51,
            duplicate: false,
            late: false,
        });

        let global = stats.global_snapshot();
        assert_eq!(global.loss.lost, 1);
        assert_eq!(global.loss.total, 4);
        assert_eq!(global.loss.percent, 25);
        assert_eq!(global.rtt.count, 3);
        assert_eq!(global.rtt.min_ms, 9);
        assert_eq!(global.rtt.avg_ms, 26);
        assert_eq!(global.rtt.max_ms, 51);
        assert_eq!(global.last_rtt_ms, 51);

        let recent = stats.recent_snapshot();
        assert_eq!(recent.loss.lost, 1);
        assert_eq!(recent.loss.total, 3);
        assert_eq!(recent.loss.percent, 33);
        assert_eq!(recent.rtt.count, 2);
        assert_eq!(recent.rtt.min_ms, 20);
        assert_eq!(recent.rtt.avg_ms, 35);
        assert_eq!(recent.rtt.max_ms, 51);
    }

    #[test]
    fn duplicate_replies_do_not_skew_stats() {
        let mut stats = Stats::new(5);

        stats.apply(&AppEvent::ProbeReply {
            seq: 1,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(10),
            rtt_ms: 10,
            duplicate: false,
            late: false,
        });

        stats.apply(&AppEvent::ProbeReply {
            seq: 1,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(12),
            rtt_ms: 12,
            duplicate: true,
            late: false,
        });

        let global = stats.global_snapshot();
        assert_eq!(global.rtt.count, 1);
        assert_eq!(global.rtt.avg_ms, 10);
        assert_eq!(global.last_rtt_ms, 10);
    }
}
