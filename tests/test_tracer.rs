use agenttrace::Tracer;
use std::time::Duration;

#[test]
fn empty_tracer_aggregate() {
    let t = Tracer::new();
    let agg = t.aggregate();
    assert_eq!(agg.runs, 0);
    assert_eq!(agg.calls, 0);
    assert_eq!(agg.total_cost_usd, 0.0);
}

#[test]
fn single_run_with_two_calls() {
    let t = Tracer::new();
    let run = t.run("answer");
    run.record("claude-sonnet-4", 0.001, Duration::from_millis(100));
    run.record("claude-sonnet-4", 0.002, Duration::from_millis(200));
    let rec = run.finish();
    assert_eq!(rec.name, "answer");
    assert_eq!(rec.call_count(), 2);
    assert!((rec.total_cost_usd() - 0.003).abs() < 1e-9);
    assert_eq!(rec.total_latency(), Duration::from_millis(300));

    let agg = t.aggregate();
    assert_eq!(agg.runs, 1);
    assert_eq!(agg.calls, 2);
    assert!((agg.total_cost_usd - 0.003).abs() < 1e-9);
}

#[test]
fn percentiles_match_sorted_order() {
    let t = Tracer::new();
    let run = t.run("p");
    // Five calls: 100, 200, 300, 400, 500 ms
    for ms in [100, 200, 300, 400, 500] {
        run.record("m", 0.0, Duration::from_millis(ms));
    }
    run.finish();
    let agg = t.aggregate();
    // p50 of 5 elems (idx 2) = 300ms
    assert_eq!(agg.p50_latency, Duration::from_millis(300));
    // p95 of 5 elems (round(0.95*4)=4) = 500ms
    assert_eq!(agg.p95_latency, Duration::from_millis(500));
}

#[test]
fn by_model_breakdown() {
    let t = Tracer::new();
    let run = t.run("mixed");
    run.record("sonnet", 0.001, Duration::from_millis(100));
    run.record("haiku", 0.0001, Duration::from_millis(50));
    run.record("sonnet", 0.002, Duration::from_millis(200));
    run.finish();

    let agg = t.aggregate();
    let s = agg.by_model.get("sonnet").unwrap();
    let h = agg.by_model.get("haiku").unwrap();
    assert_eq!(s.calls, 2);
    assert!((s.cost_usd - 0.003).abs() < 1e-9);
    assert_eq!(h.calls, 1);
}

#[test]
fn multiple_runs_aggregate() {
    let t = Tracer::new();
    for i in 0..5 {
        let r = t.run(format!("run_{i}"));
        r.record("m", 0.001, Duration::from_millis(100));
        r.finish();
    }
    let agg = t.aggregate();
    assert_eq!(agg.runs, 5);
    assert_eq!(agg.calls, 5);
    assert!((agg.total_cost_usd - 0.005).abs() < 1e-9);
}

#[test]
fn reset_clears_history() {
    let t = Tracer::new();
    let r = t.run("x");
    r.record("m", 0.01, Duration::from_millis(10));
    r.finish();
    assert_eq!(t.aggregate().runs, 1);
    t.reset();
    assert_eq!(t.aggregate().runs, 0);
}

#[test]
fn cloning_shares_history() {
    let t = Tracer::new();
    let t2 = t.clone();
    let r = t.run("x");
    r.record("m", 0.0, Duration::from_millis(10));
    r.finish();
    assert_eq!(t2.aggregate().runs, 1);
}

#[test]
fn run_record_serializes_to_json() {
    let t = Tracer::new();
    let r = t.run("ser");
    r.record("m", 0.0042, Duration::from_millis(123));
    let rec = r.finish();
    let s = serde_json::to_string(&rec).unwrap();
    assert!(s.contains("\"name\":\"ser\""));
    assert!(s.contains("\"model\":\"m\""));
}
