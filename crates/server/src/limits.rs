//! Ochrona przed zalewaniem serwera: limity połączeń (na adres IP i łącznie) i wiadomości
//! (na połączenie). Adres klienta za CloudFront to `CloudFront-Viewer-Address` – ufamy mu tylko,
//! gdy żądanie przeszło kontrolę nagłówka originu (inaczej każdy mógłby go podrobić).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use tokio::time::Instant;

/// Najwięcej jednoczesnych połączeń z jednego adresu (gracz z kilkoma kartami, kilka osób za NAT).
pub const PER_IP: u32 = 12;
/// Najwięcej połączeń łącznie (t4g.micro: z zapasem na pamięć i CPU).
pub const TOTAL: u32 = 2000;

/// Liczniki otwartych połączeń. `Mutex` tylko na księgowość połączeń – nie w logice gry.
pub struct ConnLimits {
    per_ip: u32,
    total: u32,
    open: Mutex<(u32, BTreeMap<String, u32>)>,
}

/// Zajęte miejsce na połączenie – zwalnia się samo przy końcu połączenia (`Drop`).
pub struct ConnGuard {
    limits: Arc<ConnLimits>,
    ip: Option<String>,
}

impl ConnLimits {
    pub fn new(per_ip: u32, total: u32) -> Arc<Self> {
        Arc::new(ConnLimits { per_ip, total, open: Mutex::new((0, BTreeMap::new())) })
    }

    /// Miejsce na nowe połączenie; `None` = limit przekroczony. `ip = None` (bez CloudFront,
    /// lokalnie) – liczy się tylko limit łączny.
    pub fn acquire(self: &Arc<Self>, ip: Option<&str>) -> Option<ConnGuard> {
        let mut open = self.open.lock().unwrap();
        if open.0 >= self.total {
            return None;
        }
        if let Some(ip) = ip {
            let n = open.1.entry(ip.to_string()).or_insert(0);
            if *n >= self.per_ip {
                return None;
            }
            *n += 1;
        }
        open.0 += 1;
        Some(ConnGuard { limits: self.clone(), ip: ip.map(String::from) })
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut open = self.limits.open.lock().unwrap();
        open.0 -= 1;
        if let Some(ip) = &self.ip
            && let Some(n) = open.1.get_mut(ip)
        {
            *n -= 1;
            if *n == 0 {
                open.1.remove(ip);
            }
        }
    }
}

/// Wiadro żetonów na wiadomości jednego połączenia: `rate` na sekundę, zapas `burst`.
/// Zwykły klient wysyła ~1 wiadomość na sekundę (hash co 10 tur) i pojedyncze intencje.
pub struct MessageBucket {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
}

impl MessageBucket {
    pub fn new(rate: f64, burst: f64) -> Self {
        MessageBucket { tokens: burst, rate, burst, last: Instant::now() }
    }

    /// Czy wiadomość mieści się w limicie (zużywa żeton).
    pub fn take(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.last).as_secs_f64() * self.rate).min(self.burst);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Adres IP z `CloudFront-Viewer-Address` (`adres:port`, także IPv6 – port po ostatnim `:`).
pub fn viewer_ip(value: &str) -> &str {
    value.rsplit_once(':').map_or(value, |(ip, _port)| ip)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn connections_are_limited_per_ip_and_in_total() {
        let limits = ConnLimits::new(2, 3);
        let a1 = limits.acquire(Some("1.1.1.1")).unwrap();
        let _a2 = limits.acquire(Some("1.1.1.1")).unwrap();
        assert!(limits.acquire(Some("1.1.1.1")).is_none(), "trzecie z tego samego adresu");
        let _b = limits.acquire(Some("2.2.2.2")).unwrap();
        assert!(limits.acquire(None).is_none(), "limit łączny");
        drop(a1);
        assert!(limits.acquire(Some("1.1.1.1")).is_some(), "zwolnione miejsce wraca");
    }

    #[tokio::test(start_paused = true)]
    async fn message_bucket_allows_bursts_and_refills() {
        let mut bucket = MessageBucket::new(10.0, 5.0);
        assert!((0..5).all(|_| bucket.take()));
        assert!(!bucket.take(), "zapas wyczerpany");
        tokio::time::advance(Duration::from_millis(250)).await;
        assert!(bucket.take() && bucket.take(), "po 0,25 s ~2,5 żetonu");
        assert!(!bucket.take());
    }

    #[test]
    fn viewer_address_without_port() {
        assert_eq!(viewer_ip("198.51.100.10:46532"), "198.51.100.10");
        assert_eq!(viewer_ip("2001:db8::1:46532"), "2001:db8::1");
    }
}
