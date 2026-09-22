# Luna Standard Library Gap Registry (SSOT)

This document is the canonical single source of truth for identified, practical gaps in Luna's standard library. Each entry records the capability status, language feasibility, compiler dependencies, likely provider owner, and empirical evidence from probe execution.

---

## Active Gap Registry

### STDLIB-GAP-01: CLI Argument Access (`main(args: [str])`)
- **Status**: RESOLVED & SUPERSEDED BY COMPILER CONTRACT (Stage 7 / C-GAP-11)
- **Language Support**: Canonical language contract `fn main(args: [str]) -> i32`
- **Compiler Change Required**: Yes (C-GAP-11 resolved in `luna-backend::llvm_codegen`)
- **Owner**: Language / Runtime Entrypoint ABI (No public `process::*` stdlib abstraction)
- **Evidence**: Test suite in `crates/luna-driver/tests/compiler_gap_c_gap_11_main_args_tests.rs` and probe `tests/stdlib_gap_probes/cli_args.ln`.
- **Resolution**: Process startup and CLI argument delivery are runtime/compiler entrypoint concerns, not a stdlib abstraction. The obsolete public `process::*` provider was removed. CLI arguments are delivered exclusively via `fn main(args: [str]) -> i32`.

---

### STDLIB-GAP-02: Stdin Console Input (`io::read_line`)
- **Status**: RESOLVED (Stage 7 Phase 1)
- **Language Support**: Sufficient (hosted C runtime `__luna_read_line`)
- **Compiler Change Required**: No
- **Owner**: `io/io` provider
- **Evidence**: Probe `tests/stdlib_gap_probes/console_read_line.ln` compiles and runs successfully.
- **Resolution**: Implemented in `io/io.ln` with `io::read_line()` returning `Result<Option<String>, IoError>`.

---

### STDLIB-GAP-03: Whole-File Read & Write Primitives
- **Status**: RESOLVED (Stage 7 Phase 1)
- **Language Support**: Sufficient (hosted C runtime `__luna_read_file`, `__luna_write_file`)
- **Compiler Change Required**: No
- **Owner**: `io/io` provider
- **Evidence**: Probes `file_read_all.ln` and `file_write.ln` compile and run successfully.
- **Resolution**: Implemented in `io/io.ln` with `read_bytes`, `read_to_string`, `write_bytes`, `write_string`.

---

### STDLIB-GAP-04: `Hash`, `Eq`, and `PartialEq` Implementations for `String` and `[u8]`
- **Status**: RESOLVED (Stage 7 Phase 1)
- **Language Support**: Sufficient (`Hash`, `Eq`, `PartialEq` traits in `core/hash.ln` and `core/cmp.ln`)
- **Compiler Change Required**: No
- **Owner**: `alloc/string` and `core/slice` providers
- **Evidence**: Probe `tests/stdlib_gap_probes/word_frequency.ln` compiles and runs successfully.
- **Resolution**: Implemented in `alloc/string.ln` with `impl Eq for String`, `impl PartialEq for String`, and `impl Hash for String`.

---

### STDLIB-GAP-05: Text Splitting, Searching, and Line Iteration
- **Status**: RESOLVED (Stage 7 Phase 2)
- **Language Support**: Sufficient (implemented via byte slicing and `Vec<String>`)
- **Compiler Change Required**: No
- **Owner**: `alloc/string` provider
- **Evidence**: Probes `text_split.ln` and `text_find.ln` compile and run successfully.
- **Resolution**: Implemented `split(char)`, `split_str(str)`, `split_string(&String)`, `lines()`, `split_whitespace()`, `contains`, `find`, `starts_with`, `ends_with`, `trim`, `trim_string`, `trim_start`, `trim_end`.

---

### STDLIB-GAP-06: Primitive-to-String Formatting and Numeric Parsing
- **Status**: RESOLVED (Stage 7 Phase 2)
- **Language Support**: Sufficient (pure Luna decimal conversion & checked arithmetic)
- **Compiler Change Required**: Yes (`C-GAP-08`: 64-bit integer literal bitwidth preservation in LLVM codegen)
- **Owner**: `alloc/string` provider
- **Evidence**: Probes `format_values.ln` and `parse_integer.ln` compile and run successfully.
- **Resolution**: Implemented `string_from_bool`, `string_from_char`, `string_from_u8`..`string_from_u64`, `string_from_usize`, `string_from_i8`..`string_from_i64`, `string_from_isize`, as well as `parse_bool`, `parse_u8`..`parse_u64`, `parse_usize`, `parse_i8`..`parse_i64`, `parse_isize`.

---

### STDLIB-GAP-07: Basic Path Joining and Inspection
- **Status**: ACTIVE / SCHEDULED (Stage 7 Phase 3)
- **Language Support**: Sufficient
- **Compiler Change Required**: No
- **Likely Owner**: `io/path` provider
- **Evidence**: Probe `path_join.ln` fails with `external component "path" not found in sysroot`.
- **Impact**: Portable path manipulation (handling `/` vs `\\`, extensions, filenames) is unsupported.

---

### STDLIB-GAP-08: Environment Variable Inspection (`std::env::var`)
- **Status**: ACTIVE / SCHEDULED (Stage 7 Phase 3)
- **Language Support**: Sufficient (C runtime `getenv`)
- **Compiler Change Required**: No
- **Likely Owner**: `core/process` or `io/env` provider
- **Evidence**: Probe `env_read.ln` fails with `external component "env" not found in sysroot`.
- **Impact**: 12-factor apps and configuration reading via environment variables cannot be written safely.

---

### STDLIB-GAP-09: Borrowed Result / Option Value Projection
- **Status**: LANGUAGE-BLOCKED
- **Blocked By**: `LANGUAGE-GAP-01` (Borrowed enum payload projection)
- **Compiler Change Required**: Yes (formal reference projection through enum tags)
- **Likely Owner**: Compiler borrow checker and semantic phase
- **Evidence**: `Result` and `Option` methods (`unwrap`, `ok`, `err`) take `self: Self` by value; attempting to inspect `&Result<T, E>` without cloning `T` or `E` is unrepresentable.
- **Impact**: Cannot inspect large non-copy payloads inside borrowed `Result` or `Option` without cloning.

---

### STDLIB-GAP-12: Simple Key-Value Config / Text Composition
- **Status**: RESOLVED (Stage 7 Phase 2)
- **Language Support**: Sufficient
- **Compiler Change Required**: No
- **Owner**: `alloc/string` + `collections/hashmap`
- **Evidence**: Probe `tests/stdlib_gap_probes/simple_config_parse.ln` compiles and runs successfully end-to-end.
- **Resolution**: Validated end-to-end integration of `lines()`, `split('=')`, `trim_string()`, `parse_i32()`, `parse_bool()`, and `HashMap<String, String>`.

---

## Compiler Gaps Resolved During Stdlib Development

### C-GAP-05: Generic Substitution Leakage in `?` / `Try` Desugaring
- **Status**: RESOLVED (Stage 7 Phase 1)
- **Subsystem**: Monomorphizer (`luna-semantic::mono`)
- **Verification**: `compiler_gap_c_gap_07_try_mono_tests.rs`

### C-GAP-08: 64-bit Integer Literal Truncation in LLVM Backend
- **Status**: RESOLVED (Stage 7 Phase 2)
- **Subsystem**: Codegen (`luna-backend::llvm_codegen`)
- **Problem**: `Operand::Number(n)` parsed literals and truncated them into `i32_type`, silently corrupting values > 32 bits (`18446744073709551615 as u64`, `9223372036854775807 as i64`). Furthermore, integer casts defaulted to sign-extending unsigned types, and literals directly embedded in binary operations, comparisons, calls, or returns were truncated.
- **Resolution**: Refactored backend integer literal lowering into a centralized, type-directed mechanism (`generate_typed_operand`). The backend now constructs LLVM integer constants with full 64-bit precision matching the expected LLVM type (`data.ty`, parameter type, or return type) across all instructions (`Assign`, `Cast`, `Store`, `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Eq`, `NotEq`, `LessThan`, `LessOrEq`, `GreaterThan`, `GreaterOrEq`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `CallDirect`, `Ret`, `CondBr`). Unsigned casting correctly uses zero-extension (`build_int_cast_sign_flag`).
- **Verification**: `compiler_gap_c_gap_08_integer_literal_bitwidth_tests.rs` (covers all 10 end-to-end paths + direct MVIR `Instruction::Add` lowering test).

### C-GAP-09: Floating-Point Backend & Semantic Lowering Incompleteness
- **Status**: ACTIVE / SCHEDULED (Stage 7 Phase 4C)
- **Subsystem**: Codegen (`luna-backend::llvm_codegen`) & Comptime (`luna-mvir::interp`)
- **Problem**: Float literals fail parsing in `generate_typed_operand` due to premature `u64`/`i64` parsing. Arithmetic instructions (`Add`, `Sub`, `Mul`, `Div`, `Rem`) unconditionally call `coerce_int_pair` and integer builder methods, panicking on float operands. Comparisons unconditionally build integer comparisons without IEEE 754 predicates (`OEQ`, `UNE`, `OLT`, etc.). Casts lack bidirectional `int <-> float` conversions. Comptime interpreter lacks float branches in `Eq`, `NotEq`, `Rem`, and `Cast`.
- **Resolution Plan**: Scheduled for Phase 4C. Implement dedicated float operand generation, float arithmetic, IEEE 754 comparison predicates, int/float casts in LLVM backend, and float branches in comptime interpreter. Documented in `docs/spec/stdlib-core-protocols-design.md`.
