# Luna Standard Library Capability Matrix (v1 Surface)

This matrix classifies the current standard library surface across domains A through L using the 6 canonical dispositions:
`READY`, `PARTIAL`, `ERGONOMIC-GAP`, `STDLIB-GAP`, `LANGUAGE-BLOCKED`, and `OUT-OF-SCOPE`.

---

## Domain Breakdown

### A. Console / Basic I/O

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Byte slice output | `io::print`, `io::println`, `io::eprintln` on `&[u8]` | **READY** | Direct output via runtime `fwrite(..., stdout/stderr)`. |
| String / str output | Requires `io::println(s.as_bytes())` | **ERGONOMIC-GAP** | No overloaded `print(&String)` or `print(str)`. |
| Stdin byte read | None | **STDLIB-GAP** | No `stdin` stream or handle in stdlib. |
| Line input | None | **STDLIB-GAP** | No `read_line(&rw String)` or `lines()` iterator. |
| Formatted console print | None | **ERGONOMIC-GAP** | No `print!` / `println!` formatting macros. |

---

### B. Filesystem

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Whole-file read | None | **STDLIB-GAP** | No `read_to_string` or `read_to_vec` helper. |
| Whole-file write | None | **STDLIB-GAP** | No `write_file` helper. |
| File open / close | None | **STDLIB-GAP** | No `File` struct or RAII handle. |
| Streaming read / write | None | **STDLIB-GAP** | No `Read` / `Write` traits or buffer stream. |
| File existence / metadata | None | **STDLIB-GAP** | No `exists`, `is_file`, `is_dir`, `metadata`. |
| Directory iteration | None | **STDLIB-GAP** | No `read_dir` abstraction. |
| I/O Error representation | None | **STDLIB-GAP** | No `IoError` or `ErrorKind` enum. |

---

### C. CLI / Process Environment

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| CLI arguments (`argv`) | Delivered via entrypoint `fn main(args: [str]) -> i32` | **READY** | C-GAP-11 resolved; stack-allocated `[str]` slice. No public process/env module. |
| Environment variables | None | **STDLIB-GAP** | Future platform capability (if needed). |
| Exit code from `main` | `fn main() -> i32` | **READY** | Native return code passed to exit. |
| Explicit process exit | None (runtime trap via panic/abort) | **BY DESIGN** | No public `process::exit`; Luna exposes no public `process::*` namespace. |
| Current working directory | None | **STDLIB-GAP** | Future platform capability (if needed). |

---

### D. Paths

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Path representation | None | **STDLIB-GAP** | Missing `Path` / `PathBuf` types. |
| Path joining (`join`) | None | **STDLIB-GAP** | Requires manual string concatenation with separator `/` or `\\`. |
| Path components | None | **STDLIB-GAP** | Missing `parent()`, `file_name()`, `extension()`. |
| Path normalization | None | **STDLIB-GAP** | Missing path canonicalization. |

---

### E. Text

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| UTF-8 byte container | `String` in `alloc/string.ln` | **READY** | Backed by `Vec<u8>`, validated UTF-8. |
| Length & capacity | `len()`, `capacity()`, `is_empty()` | **READY** | Byte-level length and capacity queries. |
| Mutation | `push_char`, `push_str`, `clear`, `truncate` | **READY** | Full dynamic string growth. |
| Trimming | `trim()`, `trim_string()`, `trim_start()`, `trim_end()` | **READY** | Both borrowed `&[u8]` and owned `String` variants. |
| Splitting | `split(char)`, `split_str(str)`, `split_string(&String)`, `split_whitespace()` | **READY** | Lossless splitting into owned `Vec<String>`. |
| Line iteration | `lines() -> Vec<String>` | **READY** | Normalizes CRLF and LF. |
| Substring search | `find`, `find_string`, `find_char`, `contains`, `starts_with`, `ends_with` | **READY** | UTF-8 byte offset search. |
| Substring replacement | None | **STDLIB-GAP** | Deferred beyond v1. |

---

### F. Formatting

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Integer to String | `string_from_i8`..`string_from_i64`, `string_from_u8`..`string_from_u64`, `usize`, `isize` | **READY** | Pure Luna formatting without runtime sprintf. |
| Bool / Char to String | `string_from_bool`, `string_from_char` | **READY** | Verified. |
| Float to String | None | **CORE-GAP** | Scheduled for Phase 4 (`C-GAP-09` + `core/math`). |
| Display formatting trait | None | **CORE-GAP** | `trait Display` scheduled for Phase 4 (`CORE-GAP-16`). |
| String interpolation | None | **ERGONOMIC-GAP** | Macro or builder candidate. |

---

### G. Parsing

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Integer parsing | `parse_i8`..`parse_i64`, `parse_u8`..`parse_u64`, `usize`, `isize` | **READY** | Checked decimal parsing with overflow rejection. |
| Bool parsing | `parse_bool()` | **READY** | "true"/"false" parsing. |
| Parse error details | Option::None on error | **READY** | Standard convention for v1. |
| FromStr trait | None | **CORE-GAP** | Scheduled for Phase 4 (`CORE-GAP-12`). |

---

### H. Collections

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| `Vec<T>` operations | `push`, `pop`, `get`, `get_mut`, `clear`, `truncate`, `insert`, `remove` | **READY** | Basic dynamic array implementation. |
| `Vec<T>` extended methods | Missing `swap_remove`, `retain`, `dedup`, `extend`, `resize` | **CORE-GAP** | Essential systems methods (`CORE-GAP-01`). |
| `Vec<T>` traits | Missing `Clone`, `Eq` | **CORE-GAP** | Cannot clone/compare Vec (`CORE-GAP-02`). |
| `Vec<T>` iteration | `as_slice()`, `into_iter()`, `iter()`, `iter_mut()` | **READY** | Borrowed and owned iterators. |
| `HashMap<K, V>` | Primitives and `String` supported (`Hash` + `Eq`) | **READY** | Verified in Phase 1 & 2. |
| `HashMap<K, V>` Entry API | None | **CORE-GAP** | Missing `entry(k)` abstraction (`CORE-GAP-03`). |
| `HashSet<T>` | Set wrapper around `RawTable` | **READY** | Verified. |
| `HashSet<T>` set algebra | Missing `union`, `intersection`, `difference` | **CORE-GAP** | Scheduled for Phase 4 (`CORE-GAP-04`). |
| Secondary containers | `VecDeque`, `BTreeMap`, `BinaryHeap` | **DEFERRED-BEYOND-V1** | Package-level candidates. |

---

### I. Error Handling

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| `Result<T, E>` value methods | `is_ok`, `is_err`, `ok`, `err`, `unwrap`, `expect`, `map`, `and_then` | **READY** | Complete value-level representation. |
| `Result` / `Option` combinators | Missing `unwrap_or_else`, `or_else`, `flatten`, `transpose` | **CORE-GAP** | Scheduled for Phase 4 (`CORE-GAP-11`). |
| `?` operator propagation | `Try` / `FromResidual` | **READY** | Verified in Phase 1 (`C-GAP-07`). |
| Borrowed Result / Option view | Cannot project reference to payload | **LANGUAGE-BLOCKED** | Blocked by `LANGUAGE-GAP-01`. |
| Standard Error trait | None | **CORE-GAP** | `trait Error` scheduled for Phase 4 (`CORE-GAP-16`). |

---

### J. Conversions & Protocols

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Conversion traits | None (bespoke function names) | **CORE-GAP** | `From`, `Into`, `TryFrom`, `TryInto` (`CORE-GAP-12`). |
| Default value trait | None | **CORE-GAP** | `trait Default` (`CORE-GAP-13`). |

---

### K. Memory / Low-Level Utility

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| `Box<T>` smart pointer | Full allocation, deref, move, drop | **READY** | Verified ownership container. |
| `Rc<T>` shared pointer | None | **CORE-GAP** | Feasible in pure Luna under `BOX-FROZEN` (`CORE-GAP-17`). |
| Global allocator | `__luna_alloc`, `__luna_dealloc`, `__luna_realloc` | **READY** | Backed by C runtime allocator. |
| Memory operations | `size_of`, `align_of`, `copy`, `set`, `zero` | **READY** | Safe and unsafe memory primitives. |
| Memory mutation | Missing `swap`, `replace`, `take` | **CORE-GAP** | Scheduled for Phase 4 (`CORE-GAP-15`). |
| Raw pointer utilities | `read`, `write`, `drop_in_place`, `add`, `offset`, `diff`, `copy` | **READY** | Element-stride pointer mechanics. |
| Slice utilities | `slice_from_raw_parts`, `slice_from_raw_parts_mut` | **READY** | Safe reference constructors from raw pointers. |

---

### L. Numerics & Floats

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Integer operators | Basic arithmetic & bitwise operators | **READY** | Full primitive operators. |
| Integer utilities | Missing `abs`, `min`, `max`, `clamp`, `checked_*`, `saturating_*` | **CORE-GAP** | Scheduled for Phase 4 (`CORE-GAP-14`). |
| Float codegen | Backend panics on float Add/Sub/Mul/Div | **COMPILER-GAP** | `C-GAP-09` scheduled for Phase 4. |
| Float math | Missing classification (`is_nan`, etc.) and math functions | **CORE-GAP** | `core/math` scheduled for Phase 4 (`CORE-GAP-18`). |

---

### M. Time / Clocks

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Instant / Monotonic clock | None | **SCHEDULED** | Scheduled for Phase 7. |
| Duration | None | **SCHEDULED** | Scheduled for Phase 7. |
| Sleep / delay | None | **SCHEDULED** | Scheduled for Phase 7. |

---

### N. Concurrency & Platform

| Capability | Current State | Disposition | Notes / Blocker |
|---|---|---|---|
| Thread spawning / joining | C runtime primitives exist | **SCHEDULED** | Stdlib wrappers scheduled for Phase 7. |
| Mutex / Condvar | C runtime primitives exist | **SCHEDULED** | Stdlib RAII wrappers scheduled for Phase 7. |
| Atomics | None in compiler | **SCHEDULED** | Atomics scheduled for Phase 7. |

