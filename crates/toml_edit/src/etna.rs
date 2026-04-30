//! ETNA workload properties for the toml_edit crate.
//!
//! Each `property_<name>` is pure, deterministic, and takes owned concrete
//! inputs. Framework adapters (proptest, quickcheck, crabcheck, hegel) and the
//! `etna_runner` binary all delegate to these functions.

use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::DocumentMut;

/// Three-way property outcome shared by every framework.
#[derive(Debug, Clone)]
pub enum PropertyResult {
    /// The invariant held for these inputs.
    Pass,
    /// The invariant was violated; the string carries a human-readable diagnostic.
    Fail(String),
    /// Inputs are outside the property's intended domain.
    Discard,
}

const MAX_INPUT_BYTES: usize = 4096;

fn input_in_domain(s: &str) -> bool {
    s.len() <= MAX_INPUT_BYTES
}

/// Parse a TOML document. The library must report errors via `Result::Err`,
/// never via panic. The buggy variants violate this by hitting an unguarded
/// `unwrap` / `expect` / arithmetic overflow inside the parser.
///
/// Returns `Pass` when the parse returns `Ok` or `Err` cleanly; returns
/// `Fail` only when the parse panics. `Discard` is returned for inputs
/// outside the bounded domain (oversized payload).
pub fn property_parse_does_not_panic(input: String) -> PropertyResult {
    if !input_in_domain(&input) {
        return PropertyResult::Discard;
    }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = input.parse::<DocumentMut>();
    }));
    match result {
        Ok(()) => PropertyResult::Pass,
        Err(p) => {
            let msg = if let Some(s) = p.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = p.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "parser panicked".to_string()
            };
            PropertyResult::Fail(format!("parser panicked: {msg}"))
        }
    }
}

/// Parse a TOML document on a worker thread and verify that it terminates
/// within a small bounded timeout. Detects bugs where the parser/lexer enters
/// an infinite loop (e.g. a token that the lexer never consumes) or recurses
/// to stack-overflow on small inputs.
///
/// On the base tree the parse always terminates well under the timeout; the
/// buggy variants either spin on a single character or recurse without bound.
pub fn property_parse_terminates(input: String) -> PropertyResult {
    if !input_in_domain(&input) {
        return PropertyResult::Discard;
    }
    let timeout = Duration::from_millis(2_000);
    let (tx, rx) = mpsc::sync_channel(1);
    let job_input = input;
    let stack_size = 256 * 1024; // 256KB; small enough to crash a recursive parser quickly
    let builder = thread::Builder::new().stack_size(stack_size);
    let handle = builder.spawn(move || {
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = job_input.parse::<DocumentMut>();
        }));
        let _ = tx.send(res);
    });
    let handle = match handle {
        Ok(h) => h,
        Err(e) => return PropertyResult::Fail(format!("failed to spawn worker: {e}")),
    };
    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => {
            // Drop the handle without joining — thread already finished.
            let _ = handle.join();
            PropertyResult::Pass
        }
        Ok(Err(p)) => {
            // Library panic. For this property we treat panics as failures
            // too — a parser that aborts on small input is just as broken as
            // one that hangs.
            let _ = handle.join();
            let msg = if let Some(s) = p.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = p.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "parser thread panicked".to_string()
            };
            PropertyResult::Fail(format!("parser panicked on worker: {msg}"))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // We deliberately leak the worker thread — there is no portable
            // way to kill a runaway thread in safe Rust. Process exit will
            // reap it; for in-process use the OS will eventually reclaim
            // resources when the parent exits.
            drop(handle);
            PropertyResult::Fail(format!(
                "parser failed to terminate within {} ms",
                timeout.as_millis()
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            PropertyResult::Fail("parser worker disconnected without result".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_pass(r: PropertyResult) {
        match r {
            PropertyResult::Pass => {}
            PropertyResult::Discard => {}
            PropertyResult::Fail(m) => panic!("property failed: {m}"),
        }
    }

    // Witness for cc68ae4f: a `)` outside any string used to make the lexer
    // hang because `)` was wrongly listed as a token-start character and
    // `lex_atom` left it un-consumed. The fix removes `)` from `TOKEN_START`.
    #[test]
    fn witness_parse_terminates_case_close_paren_atom() {
        let s = "key = a)b\n".to_string();
        assert_pass(property_parse_terminates(s));
    }

    // Same bug, second case: a bare `)` at column 0 still drives the lexer
    // into the same infinite loop.
    #[test]
    fn witness_parse_terminates_case_lonely_close_paren() {
        let s = ")\n".to_string();
        assert_pass(property_parse_terminates(s));
    }

    // Witness for b91d460c: an unclosed inline table that ended in
    // `NeedsValue` / `NeedsEquals` state used to panic with "setting a value
    // should set a prefix" because `inline_table_close` was emitted without
    // first emitting the missing key/value events.
    #[test]
    fn witness_parse_does_not_panic_case_inline_table_no_value() {
        let s = "={=<=u==".to_string();
        assert_pass(property_parse_does_not_panic(s));
    }

    // Witness for 79681201: when the value parser reaches EOF without
    // consuming any token, it returns a placeholder integer. The buggy
    // version returned `Value::from(0)`, which has no Repr; downstream code
    // relying on `.span()` then panicked with "all items have spans". The fix
    // synthesises an empty-span Repr alongside the integer.
    #[test]
    fn witness_parse_does_not_panic_case_missing_value_no_span() {
        let s = "==\n[._[._".to_string();
        assert_pass(property_parse_does_not_panic(s));
    }

    // Witness for 1b0bd028: a malformed array (e.g. nested `[{[]-]{`)
    // followed by an unclosed line caused `Array::span` to remain `None`,
    // tripping the same `all items have spans` assertion further up the
    // call stack. The fix tracks the most recently consumed token and uses
    // it as the array's close span when the parser bails out before seeing
    // a real `]`.
    #[test]
    fn witness_parse_does_not_panic_case_malformed_array_outer_span() {
        let s = "a=[{[]-]{\na.".to_string();
        assert_pass(property_parse_does_not_panic(s));
    }

    // Witness for 57ea4b4f: same shape as 1b0bd028 but for inline tables —
    // a malformed inline table that bails out without a real `}` left
    // `InlineTable::span` as `None`, tripping the same downstream assertion.
    #[test]
    fn witness_parse_does_not_panic_case_malformed_inline_table_outer_span() {
        let s = "={[]\r].".to_string();
        assert_pass(property_parse_does_not_panic(s));
    }
}

