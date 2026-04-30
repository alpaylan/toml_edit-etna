# toml_edit — Injected Bugs

toml_edit — format-preserving TOML parser/editor for Rust. ETNA workload mining bug-fix commits in toml-rs/toml.

Total mutations: 5

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `inline_table_no_value_panic_b91d460c_1` | `inline_table_no_value_panic` | `crates/toml_parser/src/parser/document.rs:1239` | `marauders` | `b91d460cc8584110c95d8eb7fcb2f45f86b6b14a` |
| 2 | `lex_close_paren_loop_cc68ae4f_1` | `lex_close_paren_loop` | `crates/toml_parser/src/lexer/mod.rs:623` | `marauders` | `cc68ae4f426d48eb69be9178c28440585c0c32fc` |
| 3 | `malformed_array_outer_span_1b0bd028_1` | `malformed_array_outer_span` | `crates/toml_edit/src/parser/array.rs:80` | `marauders` | `1b0bd028f6695ad8314de816787eb041553685eb` |
| 4 | `malformed_inline_table_outer_span_57ea4b4f_1` | `malformed_inline_table_outer_span` | `crates/toml_edit/src/parser/inline_table.rs:87` | `marauders` | `57ea4b4f2adbb12f1fbb30ed7c45ac60e5493a31` |
| 5 | `missing_value_no_span_panic_79681201_1` | `missing_value_no_span_panic` | `crates/toml_edit/src/parser/value.rs:58` | `marauders` | `796812017df0f118130423e5109803a1742b62c5` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `inline_table_no_value_panic_b91d460c_1` | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_inline_table_no_value` |
| `lex_close_paren_loop_cc68ae4f_1` | `ParseTerminates` | `witness_parse_terminates_case_close_paren_atom`, `witness_parse_terminates_case_lonely_close_paren` |
| `malformed_array_outer_span_1b0bd028_1` | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_array_outer_span` |
| `malformed_inline_table_outer_span_57ea4b4f_1` | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` |
| `missing_value_no_span_panic_79681201_1` | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_missing_value_no_span` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `ParseDoesNotPanic` | ✓ | ✓ | ✓ | ✓ |
| `ParseTerminates` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. inline_table_no_value_panic

- **Variant**: `inline_table_no_value_panic_b91d460c_1`
- **Location**: `crates/toml_parser/src/parser/document.rs:1239` (inside `on_inline_table_open`)
- **Property**: `ParseDoesNotPanic`
- **Witness(es)**:
  - `witness_parse_does_not_panic_case_inline_table_no_value`
- **Source**: fix(edit): Don't panic on inline table keys without values
  > When an unclosed inline table reached EOF in `NeedsEquals` or `NeedsValue` state, the parser jumped straight to `inline_table_close` without first emitting the missing key-val-sep / scalar events. Downstream the toml_edit value receiver tripped a `set_prefix` assertion (`setting a value should set a prefix`) and panicked. The fix synthesises stub events for whichever state we're in before reporting the unclosed-table error.
- **Fix commit**: `b91d460cc8584110c95d8eb7fcb2f45f86b6b14a` — fix(edit): Don't panic on inline table keys without values
- **Invariant violated**: Parsing any string input — well-formed or malformed TOML — returns a `Result`, never panics. In particular, the parser must not abort the process on truncated inline tables that end mid-`{ key =`.
- **How the mutation triggers**: The mutation discards the state-dispatch match that synthesises the missing key/value events. Once an inline table reaches EOF in `NeedsValue` (e.g. `={=<=u==`), `inline_table_close` is emitted without a preceding scalar; toml_edit's `on_value` receiver panics with `setting a value should set a prefix`.

### 2. lex_close_paren_loop

- **Variant**: `lex_close_paren_loop_cc68ae4f_1`
- **Location**: `crates/toml_parser/src/lexer/mod.rs:623` (inside `lex_atom`)
- **Property**: `ParseTerminates`
- **Witness(es)**:
  - `witness_parse_terminates_case_close_paren_atom`
  - `witness_parse_terminates_case_lonely_close_paren`
- **Source**: [#1003](https://github.com/toml-rs/toml/pull/1003), [#1002](https://github.com/toml-rs/toml/issues/1002) — fix(lex): Don't loop over ')' for forever
  > A `)` byte was accidentally included in the lexer's `TOKEN_START` set. When `lex_atom` saw a `)`, `offset_for` returned 0 (the current position is already a token-start), nothing was consumed, and the outer iterator re-entered `lex_atom` on the same byte forever. The fix drops `)` from `TOKEN_START`.
- **Fix commit**: `cc68ae4f426d48eb69be9178c28440585c0c32fc` — fix(lex): Don't loop over ')' for forever
- **Invariant violated**: Lexing any string input — well-formed or malformed TOML — terminates in time bounded by the input length. In particular, parsing a document that contains a `)` byte must complete and either return `Ok(_)` or report a parse error.
- **How the mutation triggers**: The mutation re-adds `)` to `TOKEN_START`. Once the byte stream reaches a `)`, `offset_for(|b| TOKEN_START.contains_token(b))` returns 0 (the current byte is already a token-start), `next_slice(0)` is a no-op, and `Lexer::next` re-invokes `lex_atom` on the same `)` indefinitely — the parser never returns.

### 3. malformed_array_outer_span

- **Variant**: `malformed_array_outer_span_1b0bd028_1`
- **Location**: `crates/toml_edit/src/parser/array.rs:80` (inside `on_array`)
- **Property**: `ParseDoesNotPanic`
- **Witness(es)**:
  - `witness_parse_does_not_panic_case_malformed_array_outer_span`
- **Source**: fix(edit): Preserve outer spans for malformed arrays
  > When `on_array` exited the parsing loop without seeing a real `ArrayClose` event (e.g. nested malformed inline-table inside an array), `result.span` was left as `None`. Downstream code that asserts `all items have spans` then panicked. The fix tracks the most recently consumed event's span and synthesises an outer span covering open..last when no clean close was observed.
- **Fix commit**: `1b0bd028f6695ad8314de816787eb041553685eb` — fix(edit): Preserve outer spans for malformed arrays
- **Invariant violated**: Parsing any string input — well-formed or malformed — returns a `Result`, never panics. In particular, a malformed array followed by another malformed token must not leave the surrounding `Array::span` unset, since downstream consumers assume every emitted item has a span.
- **How the mutation triggers**: The mutation drops the `if result.span.is_none() { result.span = Some(open..close) }` fallback. On `a=[{[]-]{\na.` the parser exits `on_array` after a nested malformed inline table without ever seeing `]`; `result.span` stays `None` and the table builder later asserts `all items have spans`, panicking the parse.

### 4. malformed_inline_table_outer_span

- **Variant**: `malformed_inline_table_outer_span_57ea4b4f_1`
- **Location**: `crates/toml_edit/src/parser/inline_table.rs:87` (inside `on_inline_table`)
- **Property**: `ParseDoesNotPanic`
- **Witness(es)**:
  - `witness_parse_does_not_panic_case_malformed_inline_table_outer_span`
- **Source**: fix(edit): Preserve outer spans for malformed inline tables
  > Same shape as the malformed-array fix but for inline tables: when `on_inline_table` exited the parsing loop without seeing a clean `InlineTableClose` (e.g. an inline table that contained a malformed array and a CRLF), `result.span` was left as `None`. The downstream `all items have spans` assertion then panicked. The fix tracks the most recently consumed event's span and synthesises an outer span when no clean close was observed.
- **Fix commit**: `57ea4b4f2adbb12f1fbb30ed7c45ac60e5493a31` — fix(edit): Preserve outer spans for malformed inline tables
- **Invariant violated**: Parsing any string input — well-formed or malformed — returns a `Result`, never panics. A malformed inline table that aborts mid-parse must not leave `InlineTable::span` unset, since downstream consumers assume every emitted item has a span.
- **How the mutation triggers**: The mutation drops the `if result.span.is_none() { result.span = Some(open..close) }` fallback. On `={[]\r].` the parser exits `on_inline_table` after a nested malformed array without ever seeing `}`; `result.span` stays `None` and the table builder later asserts `all items have spans`, panicking the parse.

### 5. missing_value_no_span_panic

- **Variant**: `missing_value_no_span_panic_79681201_1`
- **Location**: `crates/toml_edit/src/parser/value.rs:58` (inside `value`)
- **Property**: `ParseDoesNotPanic`
- **Witness(es)**:
  - `witness_parse_does_not_panic_case_missing_value_no_span`
- **Source**: [#1101](https://github.com/toml-rs/toml/pull/1101), [#1100](https://github.com/toml-rs/toml/issues/1100) — fix(edit): On missing value, ensure a span is used
  > When the value parser reached EOF without consuming any event, it returned `Value::from(0)` as a placeholder. That value has no `Repr`, so downstream code relying on `set_prefix` / `.span()` (e.g. document.rs:547) panicked with 'all items have spans'. The fix synthesises an empty-span `RawString::with_span(0..0)` and wraps the placeholder integer with it.
- **Fix commit**: `796812017df0f118130423e5109803a1742b62c5` — fix(edit): On missing value, ensure a span is used
- **Invariant violated**: Parsing any string input — well-formed or malformed — returns a `Result`, never panics. Even when the value parser produces a placeholder fallback for a malformed key/value pair, the placeholder must carry a `Repr` so callers that read its span do not assertion-panic.
- **How the mutation triggers**: The mutation reverts the placeholder to `Value::from(0)`, which omits the `Repr`. On `==\n[._[._` the parser hits the EOF branch in `value()` and returns a span-less integer; the table builder later asserts `all items have spans` (document.rs:547) and panics.

## Dropped Candidates

- `85761c40` (fix(parser): Avoid panic) — no observable invariant via DocumentMut: the hex/oct/bin decoder panic is reached only by toml_parser::parse_document tests; toml_edit's value path returns Err before tripping the assertion
- `a77c61e0` (fix(parser): Stack overflow on repeated '=') — irreducibly nondeterministic abort: the bug aborts the worker process via SIGSEGV on stack overflow, which catch_unwind cannot intercept and which kills the whole runner; safely detecting it would require a per-call subprocess harness
