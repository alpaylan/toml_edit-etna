# toml_edit — ETNA Tasks

Total tasks: 20

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `inline_table_no_value_panic_b91d460c_1` | proptest | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_inline_table_no_value` |
| 002 | `inline_table_no_value_panic_b91d460c_1` | quickcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_inline_table_no_value` |
| 003 | `inline_table_no_value_panic_b91d460c_1` | crabcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_inline_table_no_value` |
| 004 | `inline_table_no_value_panic_b91d460c_1` | hegel | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_inline_table_no_value` |
| 005 | `lex_close_paren_loop_cc68ae4f_1` | proptest | `ParseTerminates` | `witness_parse_terminates_case_close_paren_atom` |
| 006 | `lex_close_paren_loop_cc68ae4f_1` | quickcheck | `ParseTerminates` | `witness_parse_terminates_case_close_paren_atom` |
| 007 | `lex_close_paren_loop_cc68ae4f_1` | crabcheck | `ParseTerminates` | `witness_parse_terminates_case_close_paren_atom` |
| 008 | `lex_close_paren_loop_cc68ae4f_1` | hegel | `ParseTerminates` | `witness_parse_terminates_case_close_paren_atom` |
| 009 | `malformed_array_outer_span_1b0bd028_1` | proptest | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_array_outer_span` |
| 010 | `malformed_array_outer_span_1b0bd028_1` | quickcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_array_outer_span` |
| 011 | `malformed_array_outer_span_1b0bd028_1` | crabcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_array_outer_span` |
| 012 | `malformed_array_outer_span_1b0bd028_1` | hegel | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_array_outer_span` |
| 013 | `malformed_inline_table_outer_span_57ea4b4f_1` | proptest | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` |
| 014 | `malformed_inline_table_outer_span_57ea4b4f_1` | quickcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` |
| 015 | `malformed_inline_table_outer_span_57ea4b4f_1` | crabcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` |
| 016 | `malformed_inline_table_outer_span_57ea4b4f_1` | hegel | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` |
| 017 | `missing_value_no_span_panic_79681201_1` | proptest | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_missing_value_no_span` |
| 018 | `missing_value_no_span_panic_79681201_1` | quickcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_missing_value_no_span` |
| 019 | `missing_value_no_span_panic_79681201_1` | crabcheck | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_missing_value_no_span` |
| 020 | `missing_value_no_span_panic_79681201_1` | hegel | `ParseDoesNotPanic` | `witness_parse_does_not_panic_case_missing_value_no_span` |

## Witness Catalog

- `witness_parse_does_not_panic_case_inline_table_no_value` — base passes, variant fails
- `witness_parse_terminates_case_close_paren_atom` — base passes, variant fails
- `witness_parse_terminates_case_lonely_close_paren` — base passes, variant fails
- `witness_parse_does_not_panic_case_malformed_array_outer_span` — base passes, variant fails
- `witness_parse_does_not_panic_case_malformed_inline_table_outer_span` — base passes, variant fails
- `witness_parse_does_not_panic_case_missing_value_no_span` — base passes, variant fails
