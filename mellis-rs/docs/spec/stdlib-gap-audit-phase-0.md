# STDLIB-GAP-AUDIT-01 — Phase 0: Real-Software Capability Inventory

## Executive Summary

Phase 0 of `STDLIB-GAP-AUDIT-01` answers the question:
> **"If today a developer only has Luna and the current sysroot, what software can they write, and where specifically are they blocked?"**

This audit was conducted strictly against the active standard library sources (`libs/external/`) and sysroot manifest (`libs/external/sysroot.toml`). Seventeen (17) genuine Luna probe programs were authored in `tests/stdlib_gap_probes/` and executed through the Luna compiler.

The audit confirms that Luna has a solid, working foundation for pure in-memory computation (vectors, box, primitive hash maps, basic string representation, and slice operations). However, a developer attempting to build real-world command-line tools or file utilities is blocked by missing I/O, process environment, path, and string utility surfaces.

---

## The 6 Canonical Dispositions

| Disposition | Definition |
|---|---|
| **READY** | Sufficient for practical software in v1 scope without workarounds. |
| **PARTIAL** | API exists but lacks critical operations or type variants. |
| **ERGONOMIC-GAP** | Feasible, but API is excessively verbose, awkward, or lacks method chaining. |
| **STDLIB-GAP** | Language semantics are fully capable of expressing it, but the standard library has not yet implemented it. |
| **LANGUAGE-BLOCKED** | Cannot currently be implemented soundly due to an open language gap or compiler barrier. |
| **OUT-OF-SCOPE** | Intentionally outside the scope of stdlib v1 (e.g. networking, async runtime, multithreading). |

---

## Probe Execution Log & Empirical Evidence

Seventeen targeted probes were executed in `tests/stdlib_gap_probes/`:

| Probe Program | Purpose | Result | Empirical Diagnostic / Finding | Disposition |
|---|---|---|---|---|
| `console_hello.ln` | Text output to stdout | **PASS (Ergonomic Gap)** | Compiles via `io::println(s.as_bytes())`. `io::println` only accepts `&[u8]`; cannot pass `str` or `String` directly. | `ERGONOMIC-GAP` |
| `console_read_line.ln` | Stdin line input | **FAIL** | `Function or static "read_line" not found in module "io"` | `STDLIB-GAP` |
| `cli_args.ln` | Process CLI arguments (`argv`) | **FAIL** | `external component "env" not found in sysroot` | `STDLIB-GAP` |
| `env_read.ln` | Read environment variable | **FAIL** | `external component "env" not found in sysroot` | `STDLIB-GAP` |
| `file_read_all.ln` | Slurp whole file to String | **FAIL** | `Function or static "read_to_string" not found in module "io"` | `STDLIB-GAP` |
| `file_write.ln` | Write buffer to file | **FAIL** | `Function or static "write_file" not found in module "io"` | `STDLIB-GAP` |
| `file_copy.ln` | Streaming file read/write | **FAIL** | `Type "File" not found in module "io"` | `STDLIB-GAP` |
| `text_split.ln` | Split string by delimiter | **FAIL** | `Method "split" not found for type "String"` | `STDLIB-GAP` |
| `text_find.ln` | Substring search in String | **FAIL** | `Method "find" not found for type "String"` | `STDLIB-GAP` |
| `parse_integer.ln` | Parse integer from String | **PASS (Partial)** | `s.parse_i32()` succeeds; lacks `parse_i64`, `parse_u64`, `parse_bool`. | `PARTIAL` |
| `format_values.ln` | Format primitives to String | **FAIL** | `cannot find function "string_from_bool"` | `PARTIAL` |
| `word_frequency.ln` | `HashMap<String, i32>` | **FAIL** | `trait "Hash" is not implemented for "String"`, `trait "Eq" is not implemented for "String"` | `STDLIB-GAP` |
| `path_join.ln` | Path manipulation / joining | **FAIL** | `external component "path" not found in sysroot` | `STDLIB-GAP` |
| `vec_transform.ln` | Functional vector mapping | **PASS (Ergonomic Gap)** | Compiles via `iter_map(it, fn_ptr)` and `iter_collect_vec(it)`. Lacks method chaining and closures. | `ERGONOMIC-GAP` |
| `hashmap_count.ln` | Primitive `HashMap<i32, i32>` | **PASS** | `hashmap_new`, `insert`, `get` compile and run correctly. | `READY` |
| `result_error_flow.ln` | Error propagation via `?` | **FAIL** | Monomorphizer leaves unsubstituted generic parameter (`SymbolId(88)`) in lowered `Try::branch` desugaring (`C-GAP-05`). | `COMPILER-BLOCKED` |
| `simple_config_parse.ln`| Line iteration over String | **FAIL** | `Method "lines" not found for type "String"` | `STDLIB-GAP` |

---

## Critical Audit Discoveries

### 1. The `String` as `HashMap` Key Blocker
In `libs/external/core/hash.ln` and `cmp.ln`, `Hash` and `Eq` traits are implemented **exclusively for primitive scalar types** (`bool`, `char`, `i8`..`i64`, `u8`..`u64`, `usize`, `isize`). Neither `String` nor `[u8]` implements `Hash` or `Eq`. As a result, writing a simple word-counting tool or dictionary (`HashMap<String, V>`) is **completely impossible** today without falling back to raw integer hashing or manual loops.

### 2. The Command-Line Argument Gap
In `D:\fdlang\runtime\src\startup\startup.c`, the hosted runtime startup hook `@__mellis_startup(int argc, char** argv)` already captures arguments and provides C helpers `__mellis_argc()` and `__mellis_argv()`. However, the Luna sysroot has no `env` or `os` provider, leaving developers with zero safe access to CLI parameters.

### 3. Missing File and Stdin Abstractions
`io/io.ln` exposes only `print`, `println`, and `eprintln` taking raw byte slices `&[u8]`. There is no abstraction for `stdin`, no line reader (`read_line`), no file handle (`File`), and no whole-file helper (`read_to_string`).

### 4. String Ergonomics and Missing Utility
`String` in `alloc/string.ln` is soundly backed by `Vec<u8>` with UTF-8 validation, but lacks:
- Splitting (`split`, `split_whitespace`, `lines`)
- Searching (`find`, `contains`, `starts_with`, `ends_with`)
- String conversions for `bool`, `char`, `u64`, `i64`, `f64`
- Trait implementations (`Eq`, `Hash`, `Clone`)

### 5. Compiler Implementation Bug: Generic `?` Substitution Leakage (`C-GAP-05`)
Probe `result_error_flow.ln` parsed, resolved, and typechecked correctly under `Try` and `FromResidual` semantics, but monomorphization left an unsubstituted generic parameter (`SymbolId(88)`) in the lowered `Try::branch` desugaring. This is classified as `C-GAP-05` (Compiler Bug in `luna-semantic::mono`). It must be resolved generically in the compiler monomorphization machinery with a permanent reproducer before completing Phase 1.
