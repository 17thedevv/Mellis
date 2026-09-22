# STDLIB-GAP-AUDIT-01 / Phase 2A — Ground-Truth Text Model Audit

**Document**: `docs/spec/stdlib-text-phase2-audit.md`  
**Milestone**: `Stage 7 — Phase 2: Text, Parsing & Formatting Completion`  
**Status**: `CANONICAL AUDIT COMPLETE ✅`  
**Compliance**: Verified against `mellis-grammar`, `luna-semantic-compliance`, `luna-stdlib-compiler-boundary`, and `BOX-FROZEN`. Zero compiler magic.

---

## 1. Executive Summary

Phase 2A conducts a ground-truth audit of Luna's current text representations, type system capabilities, lifetime contracts, and language constraints to establish a sound foundation for the text, parsing, and formatting APIs in Phase 2.

This audit definitively answers the core architectural questions regarding `String`, `str`, byte slices `&[u8]`, borrowed views, and iteration feasibility in Luna v1.

---

## 2. Text Representation & Type Audit

### Q1: What is `String`'s representation?
- **Definition**: In [`libs/external/alloc/string.ln`](file:///d:/fdlang/mellis-rs/libs/external/alloc/string.ln):
  ```luna
  module std {
      export struct String {
          private vec: Vec<u8>,
      };
  }
  ```
- **Heap Layout**: `String` wraps an owned `Vec<u8>`, which consists of `(ptr: *rw u8, len: u64, cap: u64)` managed by the global memory allocator (`__luna_alloc` / `__luna_dealloc`).
- **Invariant**: `String` maintains a strict UTF-8 validity invariant across all constructors (`string_from_str`, `string_from_bytes`) and mutators (`push_char`, `truncate`). Validation is performed via standard library function `utf8::validate(&[u8])` conforming to RFC 3629.
- **Ownership**: `String` is an owned, growable container with move semantics and deterministic drop execution (`__lang_drop`).

### Q2: What exactly is `str`? Is `str` owned or borrowed?
- **Compiler Level**: In `crates/luna-semantic/src/typechecker.rs`, `str` resolves to `SemanticType::Primitive(BuiltinType::String)`.
- **Backend Level**: In `crates/luna-backend/src/llvm_codegen.rs`:
  ```rust
  SemanticType::Primitive(BuiltinType::String) => Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into())
  ```
  `str` lowers directly to an LLVM raw pointer (`i8*` / `ptr`).
- **Nature**: `str` in Luna is a **borrowed, raw pointer to static NUL-terminated UTF-8 byte constants** (string literals like `"hello"`).
- **Metadata**: `str` carries **no length field**. Its length is determined dynamically by scanning for the NUL terminator `\0`:
  ```luna
  dec s_ptr = s as *u8;
  unsafe { while (*(ptr::add<u8>(s_ptr, len))) != (0 as u8) { len = len + 1; } }
  ```
- **Verdict**: `str` is a C-string literal reference, NOT a fat pointer `(ptr, len)`. It cannot represent arbitrary substrings because substrings are generally not NUL-terminated.

### Q3: Can arbitrary `String` byte ranges become `str`?
- **No**. Arbitrary sub-slices of a `String` do not have a terminating `\0` byte in place without either:
  1. Mutating the underlying buffer to insert `\0` (unsound and aliasing-violating).
  2. Allocating a new NUL-terminated buffer on the heap (violates zero-copy borrowing).
- Therefore, in Luna v1, sub-slices cannot be typed as `str`.

### Q4: Can arbitrary `String` byte ranges become `&[u8]`? What lifetime does such a view carry?
- **Yes**. A byte slice `&[u8]` is a 128-bit fat pointer `(data: *u8, len: u64)` in Luna.
- Any valid byte range `[start, start + len)` of a `String` can be safely projected as `&[u8]` using `slice_from_raw_parts<u8>(ptr, len)` within `unsafe`.
- The returned slice carries an explicit lifetime contract:
  ```luna
  fn as_bytes(self: &Self) -> &[u8] life_from(self)
  ```
  Under Luna's region-based borrow checker (`luna-borrowck`), `life_from(self)` prevents any mutation or destruction of `self` while the returned slice borrow is active.

### Q5: Does `String` indexing operate on bytes or Unicode scalar values?
- **Bytes (`u8`)**. All existing index operations on `as_bytes()` operate strictly on byte indices:
  ```luna
  dec b = bytes[i]; // u8
  ```
- `len()` returns the byte count of the UTF-8 payload (`vec.len()`), not the count of Unicode code points.
- **Convention**: String search positions (`find`, `starts_with`, `ends_with`) MUST canonically return and consume **UTF-8 byte offsets**, not character ordinals.

### Q6: What UTF-8 primitives already exist in stdlib?
In `alloc/string.ln`:
- `utf8::validate(bytes: &[u8]) -> bool`: RFC 3629 compliant validator.
- `utf8::is_char_boundary(bytes: &[u8], index: u64) -> bool`: returns true if `index` is 0, `len`, or `(b & 0xC0) != 0x80`.
- `utf8::encode_char(ch: char, out_vec: &rw Vec<u8>)`: encodes a `char` (Unicode scalar value) into 1 to 4 UTF-8 bytes and pushes them to `out_vec`.

### Q7: Which slice operations are available?
In `core/slice.ln`:
- `slice_iter<T>(s: &[T]) -> SliceIter<T> life_from(s)`
- `slice_contains<T: Eq>(s: &[T], item: &T) -> bool`
- `slice_position<T>(s: &[T], predicate: fn(&T) -> bool) -> Option<u64>`
- `slice_binary_search<T: Ord>(s: &[T], item: &T) -> Result<u64, u64>`
- `slice_reverse<T>(s: &rw [T])`
- `slice_sort<T: Ord>(s: &rw [T])`

---

## 3. Borrowed View & Split Iteration Feasibility Analysis

### Can `Iterator<Item = borrowed-view>` be expressed soundly in Luna v1?
Let's analyze the technical constraints:

1. **No Lifetime Generic Parameters**:
   Luna's grammar (`mellis-grammar`) and type system explicitly reject Rust-style lifetime generic parameters (`'a`). Structs cannot be declared as `struct Split<'a>`.
2. **Streaming vs Independent Borrowing**:
   In Luna's canonical `Iterator` trait ([`libs/external/lang/iterator.ln`](file:///d:/fdlang/mellis-rs/libs/external/lang/iterator.ln)):
   ```luna
   export trait Iterator<Item> {
       fn next(self: &rw Self) -> Option<Item>;
   }
   ```
   Notice that `next` takes `&rw Self` and returns `Option<Item>`.
   If `Item` borrows from `self` (`Option<&[u8]> life_from(self)`), borrowck enforces that the loan of `self` remains active as long as the returned slice is in scope. Consequently, the caller cannot call `iter.next()` again while holding the previous slice (the lending iterator dilemma).
3. **`&[u8]` vs `String` Usability Mismatch**:
   Even if a borrowed byte slice `&[u8]` were produced:
   - `&[u8]` is raw bytes, not text.
   - `&[u8]` has no `split`, `find`, or `parse_i32` methods.
   - `&[u8]` cannot be inserted into `HashMap<String, V>` without copying into a `String`.
   - In practical code (such as `simple_config_parse.ln`), consumers immediately require `String` methods like `split('=')` and `parse_i32()` on the segments.
4. **Conclusion & Architectural Decision**:
   For Stage 7 Phase 2 (stdlib v1), string splitting and line extraction will canonically provide **owned results**:
   ```luna
   String::split(self: &Self, delimiter: char) -> Vec<String>
   String::split_str(self: &Self, delimiter: str) -> Vec<String>
   String::lines(self: &Self) -> Vec<String>
   String::split_whitespace(self: &Self) -> Vec<String>
   ```
   - **Cost Tradeoff**: Allocates a `Vec<String>` containing cloned `String` segments. For command-line software, configuration parsing, text splitting, and file handling, this allocation cost is completely standard and safe.
   - **Soundness**: 100% sound under Luna's current memory and ownership model. Zero compiler magic, zero unsafe lifetime escapes, zero coupling to `LANGUAGE-GAP-01`.

---

## 4. Capability Matrix & Disposition

| Capability ID | Capability Name | Target API | Disposition | Rationale / Tradeoff |
|---|---|---|---|---|
| **`TEXT-CAP-01`** | String search & predicates | `find`, `contains`, `starts_with`, `ends_with` | **SUPPORTED & VERIFIED** | Pure Luna over UTF-8 bytes; returns byte offset `Option<u64>`. |
| **`TEXT-CAP-02`** | Borrowed substring view | `&str` / `SubstringView` | **LANGUAGE-BLOCKED** | Blocked by absence of fat borrowed string slice type (`str` is a raw C-string pointer) and lending iterator lifetime expressiveness (`LANGUAGE-GAP-04`). Note: `LANGUAGE-GAP-01` is Borrowed Enum Payload Projection, which is independent. |
| **`TEXT-CAP-03`** | String split | `split(char) -> Vec<String>`, `split_str(str) -> Vec<String>`, `split_string(&String) -> Vec<String>` | **SUPPORTED-WITH-OWNED-RESULT** | Owned `Vec<String>` provides complete, ergonomic string manipulation in v1 without lifetime hazards. |
| **`TEXT-CAP-04`** | Lines extraction | `lines() -> Vec<String>` | **SUPPORTED-WITH-OWNED-RESULT** | Owned `Vec<String>`; normalizes CRLF and LF; preserves internal blank lines; trailing terminator does not manufacture a spurious empty element. |
| **`TEXT-CAP-05`** | Whitespace splitting | `split_whitespace() -> Vec<String>` | **SUPPORTED-WITH-OWNED-RESULT** | Owned `Vec<String>`; splits on ASCII whitespace (`0x20`, `0x09`, `0x0A`, `0x0D`, `0x0B`, `0x0C`). |
| **`TEXT-CAP-06`** | Primitive parsing | `parse_i8`..`parse_i64`, `parse_u8`..`parse_u64`, `parse_bool` | **SUPPORTED & VERIFIED** | Checked decimal parsing with overflow prevention returning `Option<T>`. |
| **`TEXT-CAP-07`** | Primitive formatting | `string_from_bool`, `string_from_char`, `string_from_i8`..`string_from_u64` | **SUPPORTED & VERIFIED** | Pure Luna formatting without runtime C `sprintf` or compiler magic; handles signed `MIN` values. |
| **`TEXT-CAP-08`** | Text composition ergonomics | `push_string`, `push_char`, `clear`, `trim` | **SUPPORTED & VERIFIED** | Core String mutators and helpers. `trim_string() -> String` and `trim() -> &[u8]` for clean composability. |

---

## 5. Frozen Semantic Contracts

### 5.1 Empty Delimiter Contract (`split_str`, `split_string`)
- **Behavior**:
  - `split_str("")` & `split_string(empty_string)` -> returns `Vec<String>` with exactly 1 element: `[self.clone()]`.
  - `split(delimiter: char)` -> empty-delimiter case is not applicable (`char` is always a valid single codepoint and cannot be empty).
- **Rationale**: An empty string delimiter does NOT trigger an infinite loop, crash, or speculative grapheme/character segmentation in v1.0. It predictably treats the entire input string as a single undivided chunk.

### 5.2 Integer Parsing Grammar Contract (`parse_i*`, `parse_u*`)
- **Base / Radix**: Strictly decimal (`'0'..'9'`). Hex (`0x`), octal (`0o`), and binary (`0b`) prefixes are NOT supported and return `None`.
- **Whitespace**: No implicit trimming. Any leading, trailing, or interior whitespace returns `None`. Callers desiring whitespace tolerance must explicitly call `s.trim_string().parse_*()`.
- **Sign**:
  - Signed parsers (`parse_i8`..`parse_i64`): optional leading `-` (`0x2D`) is accepted. A bare `"-"` without digits returns `None`.
  - Unsigned parsers (`parse_u8`..`parse_u64`, `parse_usize`): leading `-` is strictly REJECTED and returns `None`.
  - Leading `+` (`0x2B`): strictly REJECTED for both signed and unsigned parsers (returns `None`).
- **Leading Zeros**: Accepted (e.g. `"007"` parses to `7`, `"000"` parses to `0`).
- **Negative Zero (`"-0"`)**: Fully accepted by signed parsers and evaluates to `Some(0)`.
- **Empty Input**: Returns `None`.
- **Overflow & Underflow**: Exact arithmetic checks against bitwidth thresholds before accumulation prevent intermediate truncation; overflowing inputs return `None`.

---

## 6. Summary of Language Gaps Relevant to Text

- **`LANGUAGE-GAP-01`**: Borrowed enum payload projection (`&Option<T>` / `&Result<T, E>`). Does not block value-oriented parsing (`Option<T>`) or owned iteration (`Vec<String>`).
- **`LANGUAGE-GAP-02`**: Range expression syntax (`s[start..end]`). Slicing is accomplished via explicit helper methods or pointer arithmetic within safe wrappers.
- **`LANGUAGE-GAP-03`**: Open-generic-dependent comptime under current pre-mono staging.
- **`LANGUAGE-GAP-04`**: Borrowed/lending iterator item lifetime expressiveness (`Iterator<Item = &[u8] life_from(self)>`). Standard library uses owned `Vec<String>` allocations (`SUPPORTED-WITH-OWNED-RESULT`) for v1, which is a soundly supported v1 API avoiding generic associated types (GATs) or lifetime-parameterized trait contracts, while borrowed/lending variants remain tracked in `docs/spec/language-gap-registry.md`.


