//! Rate limiting for sign-in (plan section 12: "Rate-limited auth"). A token
//! bucket per client address, in memory: no new crate, and forgotten on restart,
//! which is fine for a limiter.
//!
//! Behind a reverse proxy every device shares the proxy's address, so they share
//! one bucket. Forwarded-for headers are not trusted: any client can write them.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct RateLimiter {
    capacity: f64,
    per_second: f64,
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
}

impl RateLimiter {
    /// `capacity` attempts at once, refilled at `capacity` per `period`.
    pub(crate) fn new(capacity: u32, period: Duration) -> RateLimiter {
        RateLimiter {
            capacity: f64::from(capacity),
            per_second: f64::from(capacity) / period.as_secs_f64(),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Take one attempt for `ip` at `now`; Err(seconds to wait) when there is none.
    pub(crate) fn take(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let mut b = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        if b.len() > 10_000 {
            // Forget full buckets, so a flood of addresses can't grow this forever.
            let (cap, rate) = (self.capacity, self.per_second);
            b.retain(|_, (tokens, at)| {
                *tokens + now.duration_since(*at).as_secs_f64() * rate < cap
            });
        }
        let (tokens, at) = b.entry(ip).or_insert((self.capacity, now));
        *tokens =
            (*tokens + now.duration_since(*at).as_secs_f64() * self.per_second).min(self.capacity);
        *at = now;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            Ok(())
        } else {
            Err(((1.0 - *tokens) / self.per_second).ceil() as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_is_allowed_then_refused_then_refilled() {
        let l = RateLimiter::new(10, Duration::from_secs(60));
        let ip: IpAddr = "192.168.1.5".parse().unwrap();
        let other: IpAddr = "192.168.1.6".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..10 {
            l.take(ip, t0).unwrap();
        }
        let wait = l.take(ip, t0).unwrap_err();
        assert_eq!(wait, 6, "one attempt back every 6 s");
        l.take(other, t0).unwrap();
        l.take(ip, t0 + Duration::from_secs(6)).unwrap();
        assert!(l.take(ip, t0 + Duration::from_secs(6)).is_err());
    }
}
