# agenttrace

[![crates.io](https://img.shields.io/crates/v/agenttrace.svg)](https://crates.io/crates/agenttrace)
[![docs.rs](https://docs.rs/agenttrace/badge.svg)](https://docs.rs/agenttrace)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Cost + latency aggregation for LLM agent runs. Group calls into named runs, get totals, p50/p95, per-model breakdown.

```toml
[dependencies]
agenttrace = "0.1"
```

## Why

`cachebench` tells you per-call hit rate. `claude-cost` tells you per-call dollars. `agenttrace` tells you what one *agent run* (one user turn, one workflow) cost end-to-end and how long it took. That's the unit your product team and finance care about.

## Quick start

```rust
use agenttrace::Tracer;
use std::time::Duration;

let tracer = Tracer::new();

// One agent run = one user request:
let run = tracer.run("answer_user_question");
run.record("claude-sonnet-4-20250514", 0.0042, Duration::from_millis(1230));
run.record("claude-sonnet-4-20250514", 0.0017, Duration::from_millis(540));
run.finish();

// After many runs:
let agg = tracer.aggregate();
println!("{} runs, ${:.4} total, p95 = {:?}", agg.runs, agg.total_cost_usd, agg.p95_latency);
for (model, stats) in &agg.by_model {
    println!("  {}: {} calls, ${:.4}", model, stats.calls, stats.cost_usd);
}
```

## Composes with `cachebench` and `claude-cost`

```rust,ignore
use agenttrace::Tracer;
use claude_cost::{cost, Pricing};
use cachebench::{CacheTracker, Provider};

let cache = CacheTracker::new(Provider::Anthropic);
let trace = Tracer::new();
let run = trace.run("answer");

let resp = call_claude(...);  // your code
cache.record(prefix_id, usage, elapsed);
let usd = cost("claude-sonnet-4-20250514", &usage, &Pricing::default());
run.record("claude-sonnet-4-20250514", usd, elapsed);

run.finish();
```

## API

```rust
Tracer::new()
tracer.run(name) -> RunHandle
run.record(model, cost_usd, latency)
run.finish() -> RunRecord
tracer.runs() -> Vec<RunRecord>
tracer.aggregate() -> Aggregate { runs, calls, total_cost_usd, p50_latency, p95_latency, by_model }
tracer.reset()
```

`RunRecord` and `CallRecord` derive `Serialize`/`Deserialize` — easy to dump to JSON, ship to your warehouse, etc.

## What it doesn't do

- Doesn't compute cost — you supply it. Use `claude-cost` or your own pricing.
- Doesn't make HTTP calls.
- Not OTel — for traces, ship your own span emitter or watch for a future `agenttrace-otel` companion.

## Sibling: JS `@mukundakatta/agenttrace`

JS users: see [@mukundakatta/agenttrace](https://github.com/MukundaKatta/agenttrace).

## License

MIT
