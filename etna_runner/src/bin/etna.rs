// ETNA workload runner for toml_edit.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: <PascalCase property name from etna.toml> | All

use crabcheck::quickcheck as crabcheck_qc;
use hegel::{generators as hgen, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestError, TestRunner};
use quickcheck::{QuickCheck, ResultStatus, TestResult};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use toml_edit::etna::{
    property_parse_does_not_panic, property_parse_terminates, PropertyResult,
};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &["ParseTerminates", "ParseDoesNotPanic"];

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    let mut final_status: Result<(), String> = Ok(());
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if r.is_err() && final_status.is_ok() {
            final_status = r;
        }
    }
    (final_status, total)
}

// ============================================================================
// Input alphabet / generators
//
// Both invariants under test (parse-does-not-panic, parse-terminates) are
// concerned with malformed-but-still-UTF-8 inputs. We restrict to a
// "TOML-syntactic" alphabet so random fuzzing has a meaningful probability
// of hitting the trigger sequences (`)`, `={=<=`, `==[._`, …).
// ============================================================================

const TOML_ALPHABET: &[u8] = b"abcxyz012_-.= []{}\n\t,()\"'\\:#";

fn bytes_to_toml_str(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for b in bytes {
        s.push(TOML_ALPHABET[(*b as usize) % TOML_ALPHABET.len()] as char);
    }
    s
}

// ----- Canonical witnesses (tool=etna) --------------------------------------
//
// Each property is shared across multiple mutations; running a single
// witness would only trigger one of them in the etna mode. Each `check_*`
// runs every distinct witness for its property and fails if any one of
// them violates the contract.

fn check_parse_terminates() -> Result<(), String> {
    // Mirrors crates/toml_edit/src/etna.rs `witness_parse_terminates_*`.
    let inputs = ["key = a)b\n", ")\n"];
    for s in inputs {
        to_err(property_parse_terminates(s.to_string()))?;
    }
    Ok(())
}

fn check_parse_does_not_panic() -> Result<(), String> {
    // Mirrors crates/toml_edit/src/etna.rs `witness_parse_does_not_panic_*`.
    // Each input targets a distinct mutation; etna mode must fail if any
    // of them panics, so we feed every witness through the property.
    let inputs = [
        "={=<=u==",        // inline_table_no_value_panic_b91d460c_1
        "==\n[._[._",      // missing_value_no_span_panic_79681201_1
        "a=[{[]-]{\na.",   // malformed_array_outer_span_1b0bd028_1
        "={[]\r].",        // malformed_inline_table_outer_span_57ea4b4f_1
    ];
    for s in inputs {
        to_err(property_parse_does_not_panic(s.to_string()))?;
    }
    Ok(())
}

// ============================================================================
// etna driver — single-shot deterministic check.
// ============================================================================

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let prop_name = property.to_string();
    let status: Result<(), String> = std::panic::catch_unwind(AssertUnwindSafe(|| match property {
        "ParseTerminates" => check_parse_terminates(),
        "ParseDoesNotPanic" => check_parse_does_not_panic(),
        _ => Err(format!("Unknown property for etna: {prop_name}")),
    }))
    .unwrap_or_else(|p| {
        let msg = p
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| format!("panic in {prop_name}"));
        Err(format!("(panic: {msg})"))
    });
    if matches!(status.as_ref(), Err(e) if e.starts_with("Unknown property for etna")) {
        return (status, Metrics::default());
    }
    (
        status,
        Metrics {
            inputs: 1,
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ============================================================================
// proptest driver
// ============================================================================

fn run_proptest_one<F>(body: F, counter: Arc<AtomicU64>) -> Result<(), String>
where
    F: Fn(String) -> PropertyResult + 'static,
{
    // Cases set to u32::MAX so the test budget is effectively unbounded —
    // etna's per-trial wall-clock timeout governs trial duration. Without
    // this, 200-case runs exhaust their budget without finding the bug
    // and report `passed`, which is misleading for canary mutations.
    let mut runner = TestRunner::new(ProptestConfig {
        cases: u32::MAX,
        max_global_rejects: u32::MAX,
        ..ProptestConfig::default()
    });
    let strategy = proptest::collection::vec(any::<u8>(), 0..1024)
        .prop_map(|v| bytes_to_toml_str(&v));
    runner
        .run(&strategy, move |s: String| {
            counter.fetch_add(1, Ordering::Relaxed);
            let cex = format!("({:?})", s);
            let res = std::panic::catch_unwind(AssertUnwindSafe(|| body(s.clone())));
            match res {
                Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                _ => Err(TestCaseError::fail(cex)),
            }
        })
        .map_err(|e| match e {
            TestError::Fail(reason, _) => reason.to_string(),
            other => other.to_string(),
        })
}

fn run_proptest_property(property: &str) -> Outcome {
    #[allow(unused_imports)]
    use proptest::test_runner::TestRunner;
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let status: Result<(), String> = match property {
        "ParseTerminates" => run_proptest_one(
            |s: String| property_parse_terminates(s),
            counter.clone(),
        ),
        "ParseDoesNotPanic" => run_proptest_one(
            |s: String| property_parse_does_not_panic(s),
            counter.clone(),
        ),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            );
        }
    };
    (
        status,
        Metrics {
            inputs: counter.load(Ordering::Relaxed),
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ============================================================================
// quickcheck driver
// ============================================================================

#[derive(Clone, Debug)]
struct TomlByteVec(Vec<u8>);

impl std::fmt::Display for TomlByteVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl quickcheck::Arbitrary for TomlByteVec {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        // Mirror the rstar-style guard: ensure a non-zero range so the fork
        // never hits `random_range(0..0)` on the very first test.
        let s = g.size().max(1);
        let n = g.random_range(0..s.max(1));
        let v: Vec<u8> = (0..n).map(|_| <u8 as quickcheck::Arbitrary>::arbitrary(g)).collect();
        TomlByteVec(v)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(self.0.shrink().map(TomlByteVec))
    }
}

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_parse_terminates(xs: TomlByteVec) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let s = bytes_to_toml_str(&xs.0);
    match property_parse_terminates(s) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Fail(_) => TestResult::failed(),
        PropertyResult::Discard => TestResult::discard(),
    }
}

fn qc_parse_does_not_panic(xs: TomlByteVec) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let s = bytes_to_toml_str(&xs.0);
    match property_parse_does_not_panic(s) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Fail(_) => TestResult::failed(),
        PropertyResult::Discard => TestResult::discard(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let result = match property {
        "ParseTerminates" => QuickCheck::new()
            .tests(u64::MAX)
            .max_tests(u64::MAX)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_parse_terminates as fn(TomlByteVec) -> TestResult),
        "ParseDoesNotPanic" => QuickCheck::new()
            .tests(u64::MAX)
            .max_tests(u64::MAX)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_parse_does_not_panic as fn(TomlByteVec) -> TestResult),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            );
        }
    };
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("aborted: {err:?}")),
        ResultStatus::TimedOut => Err("timed out".into()),
        ResultStatus::GaveUp => Err(format!("gave up after {} tests", result.n_tests_passed)),
    };
    (
        status,
        Metrics {
            inputs: QC_COUNTER.load(Ordering::Relaxed),
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ============================================================================
// crabcheck driver
// ============================================================================

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_parse_terminates(xs: Vec<u8>) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let s = bytes_to_toml_str(&xs);
    match property_parse_terminates(s) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_parse_does_not_panic(xs: Vec<u8>) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let s = bytes_to_toml_str(&xs);
    match property_parse_does_not_panic(s) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    use crabcheck_qc as cc;
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cfg = cc::Config { tests: u64::MAX };
    let result = match property {
        "ParseTerminates" => cc::quickcheck_with_config(
            cfg,
            cc_parse_terminates as fn(Vec<u8>) -> Option<bool>,
        ),
        "ParseDoesNotPanic" => cc::quickcheck_with_config(
            cfg,
            cc_parse_does_not_panic as fn(Vec<u8>) -> Option<bool>,
        ),
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            );
        }
    };
    let status = match result.status {
        cc::ResultStatus::Finished => Ok(()),
        cc::ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        cc::ResultStatus::TimedOut => Err("timed out".into()),
        cc::ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        cc::ResultStatus::Aborted { error } => Err(format!("aborted: {error}")),
    };
    (
        status,
        Metrics {
            inputs: CC_COUNTER.load(Ordering::Relaxed),
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ============================================================================
// hegel driver
// ============================================================================

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    use hegel::HealthCheck;
    HegelSettings::new()
        .test_cases(u64::MAX)
        .suppress_health_check(HealthCheck::all())
}

fn draw_str(tc: &TestCase) -> String {
    let raw: Vec<u8> = tc.draw(hgen::vecs(hgen::integers::<u8>()).max_size(1024));
    bytes_to_toml_str(&raw)
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let prop = property.to_string();
    let run_result = std::panic::catch_unwind(AssertUnwindSafe(move || match prop.as_str() {
        "ParseTerminates" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let s = draw_str(&tc);
                let cex = format!("({:?})", s);
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_parse_terminates(s.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    _ => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "ParseDoesNotPanic" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let s = draw_str(&tc);
                let cex = format!("({:?})", s);
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_parse_does_not_panic(s.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    _ => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        other => panic!("__unknown_property:{other}"),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, Metrics { inputs, elapsed_us })
}

// ============================================================================
// Dispatch + main
// ============================================================================

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {tool}")), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    m: Metrics,
    cex: Option<&str>,
    err: Option<&str>,
) {
    let cex = cex.map_or("null".into(), json_str);
    let err = err.map_or("null".into(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        m.inputs,
        json_str(&format!("{}us", m.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    // We deliberately do NOT install a no-op panic hook before running.
    // hegeltest 0.3.7 installs its own hook in `Hegel::new(_).run()` to
    // capture counterexample panics; replacing it with a no-op hook breaks
    // hegel's panic handling and causes the process to abort (SIGABRT)
    // even when no counterexample is hit. Frameworks already wrap each
    // test case in `catch_unwind`, so a library-under-test panic still
    // surfaces as a counterexample without leaking to stderr in practice.
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| run(tool, property)));

    let (status, m) = match caught {
        Ok(outcome) => outcome,
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "adapter panic (non-string payload)".into());
            emit_json(tool, property, "aborted", Metrics::default(), None, Some(&msg));
            return;
        }
    };
    match status {
        Ok(()) => emit_json(tool, property, "passed", m, None, None),
        Err(e) => emit_json(tool, property, "failed", m, Some(&e), None),
    }
}
