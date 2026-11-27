//! Rate limiting for sign-in (plan section 12: "Rate-limited auth"). A token
//! bucket per client address, in memory: no new crate, and forgotten on restart,
//! which is fine for a limiter.
//!
//! Behind a reverse proxy every device shares the proxy's address, so they share
//! one bucket. Forwarded-for headers are not trusted: any client can write them.
//! An IPv6 client is limited by its /64, the block one home or one host gets, so a
//! new address in the same block is not a fresh bucket.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct RateLimiter {
    capacity: f64,
    per_second: f64,
    buckets: Mutex<Buckets>,
}

#[derive(Debug)]
struct Buckets {
    by_client: HashMap<IpAddr, (f64, Instant)>,
    swept: Option<Instant>,
}

/// The address a bucket is kept for: an IPv4 address (also when written as IPv6), or
/// an IPv6 address's /64.
fn client_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(std::net::Ipv6Addr::from(
                u128::from(v6) & !((1u128 << 64) - 1),
            )),
        },
    }
}

impl RateLimiter {
    /// `capacity` attempts at once, refilled at `capacity` per `period`.
    pub(crate) fn new(capacity: u32, period: Duration) -> RateLimiter {
        RateLimiter {
            capacity: f64::from(capacity),
            per_second: f64::from(capacity) / period.as_secs_f64(),
            buckets: Mutex::new(Buckets {
                by_client: HashMap::new(),
                swept: None,
            }),
        }
    }

    /// Take one attempt for `ip` at `now`; Err(seconds to wait) when there is none.
    pub(crate) fn take(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let mut guard = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        let b = &mut *guard;
        // Forget full buckets, so a flood of addresses can't grow this forever: at most
        // once a minute, so a big map isn't walked on every request.
        if b.by_client.len() > 10_000
            && b.swept
                .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(60))
        {
            let (cap, rate) = (self.capacity, self.per_second);
            b.by_client.retain(|_, (tokens, at)| {
                *tokens + now.duration_since(*at).as_secs_f64() * rate < cap
            });
            b.swept = Some(now);
        }
        let (tokens, at) = b
            .by_client
            .entry(client_key(ip))
            .or_insert((self.capacity, now));
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

    #[test]
    fn an_ipv6_block_shares_one_bucket() {
        let l = RateLimiter::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        l.take("2001:db8:1:2::1".parse().unwrap(), t0).unwrap();
        l.take("2001:db8:1:2::ffff".parse().unwrap(), t0).unwrap();
        assert!(
            l.take("2001:db8:1:2:abcd::9".parse().unwrap(), t0).is_err(),
            "the same /64"
        );
        l.take("2001:db8:1:3::1".parse().unwrap(), t0).unwrap();
        // An IPv4 address written as IPv6 is that IPv4 address.
        l.take("192.168.1.7".parse().unwrap(), t0).unwrap();
        l.take("::ffff:192.168.1.7".parse().unwrap(), t0).unwrap();
        assert!(l.take("192.168.1.7".parse().unwrap(), t0).is_err());
    }
}
