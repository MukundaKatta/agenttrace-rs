use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// One LLM call captured under a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRecord {
    /// Model identifier (provider-specific).
    pub model: String,
    /// USD cost of this call. Compute it however you like; agenttrace just sums.
    pub cost_usd: f64,
    /// Wall-clock latency.
    pub latency: Duration,
    /// Recorded at this moment.
    pub timestamp: SystemTime,
}

/// One completed run: a name plus the calls it contains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// User-supplied run name (e.g. `"answer_user_question"`).
    pub name: String,
    /// Calls recorded under this run.
    pub calls: Vec<CallRecord>,
    /// When the run started.
    pub started_at: SystemTime,
    /// When `finish()` was called.
    pub ended_at: SystemTime,
}

impl RunRecord {
    /// Sum of all call costs.
    pub fn total_cost_usd(&self) -> f64 {
        self.calls.iter().map(|c| c.cost_usd).sum()
    }

    /// Sum of all call latencies (sequential — caller decides if parallel).
    pub fn total_latency(&self) -> Duration {
        self.calls.iter().map(|c| c.latency).sum()
    }

    /// Number of calls in this run.
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }
}

/// In-flight run handle. Drop or call [`finish`](Self::finish) to seal it.
///
/// Dropping the handle without calling [`finish`](Self::finish) seals the run
/// just the same — the recorded calls are pushed to the parent tracer's history
/// so they are never silently lost. The only difference is that `finish`
/// returns the [`RunRecord`] to the caller.
pub struct RunHandle {
    name: String,
    started_at: SystemTime,
    calls: Mutex<Vec<CallRecord>>,
    parent: Arc<TracerInner>,
    sealed: AtomicBool,
}

impl RunHandle {
    /// Record one LLM call against this run.
    pub fn record(&self, model: impl Into<String>, cost_usd: f64, latency: Duration) {
        self.calls.lock().push(CallRecord {
            model: model.into(),
            cost_usd,
            latency,
            timestamp: SystemTime::now(),
        });
    }

    /// Build the [`RunRecord`] and push it to the parent, exactly once.
    ///
    /// Returns `None` if the run was already sealed (e.g. `finish` was called
    /// and then the handle dropped), so the caller never double-records.
    fn seal(&self) -> Option<RunRecord> {
        if self.sealed.swap(true, Ordering::AcqRel) {
            return None;
        }
        let calls = std::mem::take(&mut *self.calls.lock());
        let rec = RunRecord {
            name: self.name.clone(),
            calls,
            started_at: self.started_at,
            ended_at: SystemTime::now(),
        };
        self.parent.runs.lock().push(rec.clone());
        Some(rec)
    }

    /// Seal the run and ship it to the parent tracer's history.
    pub fn finish(self) -> RunRecord {
        // `seal` returns `Some` because a live, owned handle cannot have been
        // sealed yet; the subsequent `Drop` is a no-op thanks to the flag.
        self.seal()
            .expect("a freshly owned RunHandle is sealed exactly once by finish")
    }
}

impl Drop for RunHandle {
    fn drop(&mut self) {
        // If the handle is dropped without `finish`, still seal the run so its
        // recorded calls reach the parent instead of being silently discarded.
        let _ = self.seal();
    }
}

/// Per-model rollup.
#[derive(Debug, Clone, Default)]
pub struct ModelBreakdown {
    /// Total calls across runs for this model.
    pub calls: usize,
    /// Total cost across runs for this model.
    pub cost_usd: f64,
    /// Sum of latencies for this model.
    pub total_latency: Duration,
}

/// Cross-run aggregate stats.
#[derive(Debug, Clone, Default)]
pub struct Aggregate {
    /// Number of finished runs.
    pub runs: usize,
    /// Total LLM calls across all finished runs.
    pub calls: usize,
    /// Total cost across all runs.
    pub total_cost_usd: f64,
    /// p50 latency across all calls.
    pub p50_latency: Duration,
    /// p95 latency across all calls.
    pub p95_latency: Duration,
    /// Per-model breakdown.
    pub by_model: HashMap<String, ModelBreakdown>,
}

struct TracerInner {
    runs: Mutex<Vec<RunRecord>>,
}

/// Records runs and exposes aggregate stats.
#[derive(Clone)]
pub struct Tracer {
    inner: Arc<TracerInner>,
}

impl Default for Tracer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracer {
    /// Construct an empty tracer.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TracerInner {
                runs: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Start a new run with `name`. Drop or call [`RunHandle::finish`] to seal.
    pub fn run(&self, name: impl Into<String>) -> RunHandle {
        RunHandle {
            name: name.into(),
            started_at: SystemTime::now(),
            calls: Mutex::new(Vec::new()),
            parent: self.inner.clone(),
            sealed: AtomicBool::new(false),
        }
    }

    /// All sealed runs, oldest first.
    pub fn runs(&self) -> Vec<RunRecord> {
        self.inner.runs.lock().clone()
    }

    /// Drop all recorded runs.
    pub fn reset(&self) {
        self.inner.runs.lock().clear();
    }

    /// Cross-run aggregate.
    pub fn aggregate(&self) -> Aggregate {
        let runs = self.runs();
        if runs.is_empty() {
            return Aggregate::default();
        }
        let calls: Vec<&CallRecord> = runs.iter().flat_map(|r| r.calls.iter()).collect();
        let mut latencies_us: Vec<u128> = calls.iter().map(|c| c.latency.as_micros()).collect();
        latencies_us.sort_unstable();
        let p = |percent: f64| -> Duration {
            if latencies_us.is_empty() {
                return Duration::ZERO;
            }
            let idx = ((latencies_us.len() as f64 - 1.0) * percent).round() as usize;
            Duration::from_micros(latencies_us[idx] as u64)
        };

        let mut by_model: HashMap<String, ModelBreakdown> = HashMap::new();
        for c in &calls {
            let entry = by_model.entry(c.model.clone()).or_default();
            entry.calls += 1;
            entry.cost_usd += c.cost_usd;
            entry.total_latency += c.latency;
        }

        Aggregate {
            runs: runs.len(),
            calls: calls.len(),
            total_cost_usd: calls.iter().map(|c| c.cost_usd).sum(),
            p50_latency: p(0.50),
            p95_latency: p(0.95),
            by_model,
        }
    }
}
