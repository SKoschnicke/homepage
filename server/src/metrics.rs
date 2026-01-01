use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use parking_lot::RwLock;
use serde::Serialize;

const HISTOGRAM_SIZE: usize = 1000;

pub struct Metrics {
    total_requests: AtomicU64,
    current_connections: AtomicUsize,
    websocket_clients: AtomicUsize,
    start_time: SystemTime,
    last_snapshot_time: RwLock<Instant>,
    last_snapshot_requests: AtomicU64,
    latency_histogram: Arc<RwLock<LatencyHistogram>>,
}

struct LatencyHistogram {
    samples: Vec<u64>,
    write_idx: usize,
}

#[derive(Serialize, Clone)]
pub struct MetricsSnapshot {
    pub requests_per_sec: f64,
    pub websocket_clients: usize,
    pub uptime_secs: u64,
    pub p50_micros: u64,
    pub p95_micros: u64,
    pub p99_micros: u64,
    pub total_requests: u64,
}

impl LatencyHistogram {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(HISTOGRAM_SIZE),
            write_idx: 0,
        }
    }

    fn add_sample(&mut self, micros: u64) {
        if self.samples.len() < HISTOGRAM_SIZE {
            self.samples.push(micros);
        } else {
            self.samples[self.write_idx] = micros;
            self.write_idx = (self.write_idx + 1) % HISTOGRAM_SIZE;
        }
    }

    fn percentile(&self, p: f64) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }

        let mut sorted = self.samples.clone();
        sorted.sort_unstable();

        let idx = ((p / 100.0) * (sorted.len() as f64)) as usize;
        let idx = idx.min(sorted.len() - 1);
        sorted[idx]
    }
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_requests: AtomicU64::new(0),
            current_connections: AtomicUsize::new(0),
            websocket_clients: AtomicUsize::new(0),
            start_time: SystemTime::now(),
            last_snapshot_time: RwLock::new(Instant::now()),
            last_snapshot_requests: AtomicU64::new(0),
            latency_histogram: Arc::new(RwLock::new(LatencyHistogram::new())),
        })
    }

    pub fn record_request(&self, duration: Duration) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let micros = duration.as_micros() as u64;
        self.latency_histogram.write().add_sample(micros);
    }

    pub fn increment_connections(&self) {
        self.current_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_connections(&self) {
        self.current_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn increment_ws_clients(&self) {
        self.websocket_clients.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_ws_clients(&self) {
        self.websocket_clients.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let now = Instant::now();
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let websocket_clients = self.websocket_clients.load(Ordering::Relaxed);

        // Calculate requests per second since last snapshot
        let mut last_time = self.last_snapshot_time.write();
        let elapsed = now.duration_since(*last_time).as_secs_f64();
        let last_requests = self.last_snapshot_requests.swap(total_requests, Ordering::Relaxed);
        let requests_delta = total_requests.saturating_sub(last_requests);
        let requests_per_sec = if elapsed > 0.0 {
            requests_delta as f64 / elapsed
        } else {
            0.0
        };
        *last_time = now;

        // Calculate uptime
        let uptime_secs = self.start_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        // Calculate percentiles
        let histogram = self.latency_histogram.read();
        let p50_micros = histogram.percentile(50.0);
        let p95_micros = histogram.percentile(95.0);
        let p99_micros = histogram.percentile(99.0);

        MetricsSnapshot {
            requests_per_sec,
            websocket_clients,
            uptime_secs,
            p50_micros,
            p95_micros,
            p99_micros,
            total_requests,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            current_connections: AtomicUsize::new(0),
            websocket_clients: AtomicUsize::new(0),
            start_time: SystemTime::now(),
            last_snapshot_time: RwLock::new(Instant::now()),
            last_snapshot_requests: AtomicU64::new(0),
            latency_histogram: Arc::new(RwLock::new(LatencyHistogram::new())),
        }
    }
}
