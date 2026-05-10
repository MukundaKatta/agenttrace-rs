//! Cost + latency aggregation for LLM agent runs.
//!
//! Groups LLM calls into named *runs* (e.g. one user request, one agent
//! turn) and emits per-run + global stats: total cost, p50/p95 latency,
//! per-model breakdown. Composes with `cachebench` — feed it
//! `CacheTracker` metrics and you get the full observability stack.
//!
//! # Quick start
//!
//! ```
//! use agenttrace::Tracer;
//! use std::time::Duration;
//!
//! let tracer = Tracer::new();
//! let run = tracer.run("answer_user_question");
//! run.record("claude-sonnet-4", 0.0042, Duration::from_millis(1230));
//! run.record("claude-sonnet-4", 0.0017, Duration::from_millis(540));
//! run.finish();
//!
//! let agg = tracer.aggregate();
//! assert_eq!(agg.runs, 1);
//! assert!(agg.total_cost_usd > 0.0);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

mod tracer;

pub use crate::tracer::{
    Aggregate, CallRecord, ModelBreakdown, RunHandle, RunRecord, Tracer,
};
