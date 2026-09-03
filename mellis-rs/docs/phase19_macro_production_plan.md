# Phase 19: Production-Ready Macro System

## Mục tiêu
Nâng cấp hệ thống macro của Mellis từ prototype lên production-ready, đạt parity với Rust declarative macros và bổ sung features độc đáo của Mellis.

---

## 1. Current State Assessment

### ✅ Đã có
- Basic declarative macro syntax (`macro name { pattern => body }`)
- 6 fragment kinds: `expr`, `ident`, `ty`, `stmt`, `block`, `item`
- Multiple rules với backtracking
- Nested delimiter groups
- Syntax context cho hygiene
- Recursion limit guard (128)
- Macro-to-macro expansion

### ❌ Còn thiếu / Hardcoded

| Feature | Status | Priority |
|---------|--------|----------|
| Repetition `$(...)*` | ❌ Not implemented | P0 |
| Repetition `+` (at least 1) | ❌ Not implemented | P0 |
| Repetition `?` (optional) | ❌ Not implemented | P0 |
| Separator handling | ❌ Not implemented | P0 |
| Fragment: `tt` (token tree) | ❌ Missing | P1 |
| Fragment: `path` | ❌ Missing | P1 |
| Fragment: `lifetime` | ❌ Missing | P1 |
| Fragment: `meta` | ❌ Missing | P2 |
| Fragment: `pat` (pattern) | ❌ Missing | P1 |
| Fragment: `type_binding` | ❌ Missing | P2 |
| Hygiene: hygiene pass optimization | ⚠️ Partial | P1 |
| Error messages: match failures | ⚠️ Basic | P1 |
| Error messages: transcription errors | ❌ Missing | P1 |
| Debugging: macro expansion trace | ❌ Missing | P2 |
| Performance: incremental expansion | ❌ Missing | P2 |
| Testing: fuzzing | ❌ Missing | P1 |

---

## 2. Target Feature Set

### 2.1 Core Syntax (Must Have)

```mellis
// Basic macro
macro! print {
    (@x: expr) => {
        printLn(@x);
    }
}

// Repetition - THE KEY MISSING FEATURE
macro! vector {
    // $(item)* — zero or more
    ($($x: expr),*) => {
        Vec![$($x),*]
    }
    
    // $(item)+ — one or more
    ($($x: expr),+) => {
        if $x.is_empty() { Vec![] } else { Vec![$($x),+] }
    }
    
    // $(item)? — optional
    ($($x: expr)?) => {
        match $x {
            Some(v) => v,
            None => 0,
        }
    }
}

// Separator handling
macro! sum {
    ($($x: expr $(, $y: expr)?),*) => {
        $( $x + $y ?? 0 ),*
    }
}

// Nested repetition
macro! table {
    ($(($k: expr, $v: expr)),*) => {
        Map![ $( $k => $v ),* ]
    }
}
```

### 2.2 Fragment Kinds (Must Have)

```mellis
// Current (6)
@x: expr      // Expression
@x: ident     // Identifier  
@x: ty        // Type
@x: stmt      // Statement
@x: block     // Block
@x: item      // Item/declaration

// New (9 more = 15 total)
@x: tt        // Arbitrary token tree
@x: pat       // Pattern
@x: path      // Qualified path (module::Item)
@x: lifetime  // Lifetime annotation ('a, 'static)
@x: meta      // Attribute/meta items (@attr, @[attr(args)])
@x: type_expr // Type expression for generics
@x: field     // Struct field initializer
@x: variant   // Enum variant
@x: arm       // Match arm
```

### 2.3 Advanced Features (Nice to Have)

```mellis
// Conditional matching (Rust has this)
macro! if_let {
    ($($pat: pat) if $cond: expr) => {
        if let $pat = $cond { true } else { false }
    }
}

// Type-driven dispatch
macro! typed {
    (@x: i32) => { handle_i32(@x) }
    (@x: str) => { handle_str(@x) }
    (@x: $t: ty) => { generic_handler::<$t>(@x) }
}
```

---

## 3. Implementation Phases

### Phase 19.1: Repetition Engine (CRITICAL)

#### Goal: Implement full repetition support

**Files to modify:**
- `crates/mellis-lexer/src/lexer.rs` — Add `$` token
- `crates/mellis-parser/src/decl.rs` — Parse repetition syntax
- `crates/mellis-ast/src/decl.rs` — Ensure Repetition parsed correctly
- `crates/mellis-semantic/src/macro_engine.rs` — Implement repetition matching & transcription

**Detailed tasks:**

```
Task 19.1.1: Lexer - Add $ token
───────────────────────────────────────
- Add TokenKind::Dollar (already exists as $)
- Handle $ followed by ( ) { } [ ] for repetition markers
- Parse: $( ... )  $( ... )*  $( ... )+  $( ... )?

Task 19.1.2: Parser - Repetition syntax
───────────────────────────────────────────
- Update MatcherElement::Repetition parsing
- Parse: $(elements)  $(elements)*  $(elements)+  $(elements)?
- Support optional separator: $(elements ; separator)*
- Update TranscriberElement::Repetition parsing
- Transcribe: $( @var ),*  $( @var ),+

Task 19.1.3: Matcher - Repetition matching
───────────────────────────────────────────
- Implement greedy matching algorithm for $(...)* 
- Implement reluctant matching option for $(...)*?
- Implement + (one-or-more) validation
- Implement ? (zero-or-one) semantics
- Handle nested repetition: $((@a: expr),*)*
- Handle separator validation: require/ignore separators

Task 19.1.4: Transcription - Repetition expansion
───────────────────────────────────────────────────
- Implement token list expansion for $(...)* 
- Handle separator insertion between repetitions
- Nested transcription: $( ... $(inner)* ... )*
- Variable binding in repetition: $((@k, @v): expr) => ...

Task 19.1.5: Tests
───────────────────────────────────────────────────
- Unit tests for each repetition form
- Integration tests with real macros
- Edge case: empty repetition (*)
- Edge case: nested repetition
- Edge case: separator handling
```

**Estimated time:** 3-4 weeks

---

### Phase 19.2: Extended Fragment Kinds

#### Goal: Add missing fragment kinds

**New fragments:**

```mellis
@x: tt       // Token tree - raw tokens, most flexible
@x: pat      // Pattern - match arms, destructuring
@x: path     // Path - module::Item::Variant
@x: lifetime // Lifetime - 'a, 'static, 'foo
@x: meta     // Meta - #[attr], #[attr(args)]
```

**Files to modify:**
- `crates/mellis-ast/src/decl.rs` — Add FragmentKind variants
- `crates/mellis-parser/src/decl.rs` — Parse new fragment specifiers
- `crates/mellis-semantic/src/macro_engine.rs` — Implement capture_fragment_parser for new types

**Detailed tasks:**

```
Task 19.2.1: Fragment - tt (token tree)
───────────────────────────────────────────
- tt is the most flexible - matches any token sequence
- Must not include unbalanced delimiters (or handle them)
- Use case: macro_rules! style TT munching

Task 19.2.2: Fragment - pat (pattern)
───────────────────────────────────────────
- Matches patterns: `x`, `(a, b)`, `[head, ..]`
- Must be valid pattern syntax
- Use case: macro that generates match arms

Task 19.2.3: Fragment - path
───────────────────────────────────────────
- Matches qualified paths: `std::vec::Vec`
- May include generic args: `Vec<i32>`
- Use case: type manipulation macros

Task 19.2.4: Fragment - lifetime
───────────────────────────────────────────
- Matches lifetime annotations: 'a, 'static, 'foo
- Use case: lifetime-related macros

Task 19.2.5: Fragment - meta
───────────────────────────────────────────
- Matches attribute/meta: #[cold], #[inline(always)]
- Use case: attribute manipulation
```

**Estimated time:** 1-2 weeks

---

### Phase 19.3: Error Handling & Diagnostics

#### Goal: Production-quality error messages

**Current problems:**
- "no rule matched" is the only error for match failure
- No indication of which patterns were tried
- No suggestions for fixing

**Target errors:**

```mellis
// Current: cryptic
error: no rule in macro `foo` matched the invocation arguments

// Target: helpful
error: macro `vec!` expects pattern like `vec![1, 2, 3]` or `vec![head; count]`
  --> file.rs:5:12
   |
5  | vec![42]
   |        ^ unexpected token, expected expression followed by comma
   |
   = help: macro `vec!` has 2 rules:
     1. ($($x: expr),*) => ...
     2. ($($x: expr); $count: expr) => ...
```

**Files to modify:**
- `crates/mellis-semantic/src/macro_engine.rs` — Collect match attempt info
- `crates/mellis-common/src/diagnostic.rs` — Add macro-specific diagnostics

**Detailed tasks:**

```
Task 19.3.1: Match failure diagnostics
───────────────────────────────────────────
- Track which patterns were tried
- Track which tokens caused mismatch
- Generate "expected X" message based on pattern
- Suggest similar patterns if applicable

Task 19.3.2: Transcription error handling
───────────────────────────────────────────
- Detect unbound metavariables in transcription
- Detect type errors in transcribed code
- Report line numbers from original macro definition

Task 19.3.3: Debug flag for expansion trace
───────────────────────────────────────────
- Add #[macro_trace] or CLI flag
- Show each step of macro expansion
- Format: "--> expanding macro `foo!`"
           "    --> captured @x = `42 + 1`"
           "    --> expanding to `42 * 2`"
```

**Estimated time:** 1 week

---

### Phase 19.4: Hygiene System

#### Goal: Robust hygiene without performance penalty

**Current implementation:**
- Uses `SyntaxContext` (expansion ID) to mark tokens
- Simple approach but may have edge cases

**Issues:**
- Nested macro expansion may leak identifiers
- Generic parameters not handled (found in testing)
- Cross-module hygiene not tested

**Files to modify:**
- `crates/mellis-semantic/src/macro_engine.rs` — Hygiene logic
- `crates/mellis-semantic/src/resolver.rs` — Scope integration

**Detailed tasks:**

```
Task 19.4.1: Fix generic parameter hygiene
───────────────────────────────────────────
- Captured generics should retain their meaning
- struct @name<T> should have T bound correctly
- type @alias<U> should have U resolved

Task 19.4.2: Cross-module hygiene
───────────────────────────────────────────
- Macro imported from other module
- Hygiene should still apply
- Test: macro defined in module A, used in module B

Task 19.4.3: Hygiene performance optimization
───────────────────────────────────────────
- Current: every token gets new SyntaxContext
- Optimize: only identifiers need hygiene marks
- Benchmark before/after expansion time
```

**Estimated time:** 1 week

---

### Phase 19.5: Testing & Validation

#### Goal: Comprehensive test coverage

**Test categories:**

```mellis
// Unit tests - Fragment kinds
test_fragment_tt();
test_fragment_pat();
test_fragment_path();
test_fragment_lifetime();

// Unit tests - Repetition
test_repetition_star_empty();    // macro!(@x*) with no args
test_repetition_star_single();  // macro!(@x*) with one arg  
test_repetition_star_many();    // macro!(@x*) with multiple
test_repetition_plus();         // macro!(@x+) requires at least 1
test_repetition_question();     // macro!(@x?) optional
test_repetition_separator();    // macro!(@x,+) with commas
test_repetition_nested();       // macro!($(@x: expr),*)

// Unit tests - Hygiene
test_hygiene_basic();
test_hygiene_nested();
test_hygiene_cross_module();
test_hygiene_generic_params();

// Integration tests
test_vec_macro();           // Realistic vector! macro
test_format_macro();       // format!("{} {}", x, y)
test_match_macro();        // Match arm generation
test_derive_macro();      // Procedural macro pattern

// Fuzzing
fuzz_repetition_matching();
fuzz_nested_macros();
fuzz_hygiene_collision();
```

**Files to create:**
- `crates/mellis-semantic/tests/macro_repetition_tests.rs`
- `crates/mellis-semantic/tests/macro_fragment_tests.rs`
- `crates/mellis-semantic/tests/macro_hygiene_tests.rs` (enhance existing)
- `crates/mellis-driver/tests/macro_integration_tests.rs`

**Estimated time:** 2 weeks

---

### Phase 19.6: Documentation & Examples

#### Goal: User-facing documentation

**Documentation structure:**

```
docs/
├── MacroSystem.md              # Main documentation
├── MacroTutorial.md            # Getting started
├── MacroCookbook.md           # Common patterns
├── MacroReference.md          # Complete syntax reference
└── MacroDebugging.md         # Troubleshooting guide
```

**Key topics to document:**

1. **Getting Started**
   - Basic syntax
   - Your first macro
   - Expansion order

2. **Fragment Specifiers**
   - When to use each
   - Examples
   - Common mistakes

3. **Repetition**
   - `*`, `+`, `?` semantics
   - Separator handling
   - Nested repetition

4. **Hygiene**
   - What it prevents
   - When hygiene applies
   - Hygiene escape hatches (if any)

5. **Advanced Patterns**
   - Token tree munching
   - Conditional matching
   - Macro composition

6. **Debugging**
   - Expansion tracing
   - Common errors
   - Performance tips

**Estimated time:** 1 week

---

## 4. Performance Considerations

### Current Performance Profile

```
Macro expansion time (rough estimates):
- Simple macro (< 10 tokens): < 1ms
- Complex macro (100 tokens): ~5ms
- Nested expansion (3 levels): ~15ms
- Pathological case (1000+ tokens): may be slow
```

### Optimization Targets

| Metric | Current | Target |
|--------|---------|--------|
| Simple macro expansion | ~1ms | ~0.5ms |
| Complex macro | ~5ms | ~2ms |
| Memory per expansion | ~10KB | ~5KB |
| Incremental (cached) | N/A | ~0.1ms |

### Optimization Strategies

```
Task 19.7.1: Caching
───────────────────────────────────────────
- Cache expanded macro results by input hash
- Invalidate cache when macro definition changes
- Use in-memory LRU cache for frequently used macros

Task 19.7.2: Lazy expansion
───────────────────────────────────────────
- Don't expand until needed
- Track dependencies for incremental compilation
- Only re-expand changed macros

Task 19.7.3: Memory optimization
───────────────────────────────────────────
- Reuse arena allocations where possible
- Avoid cloning tokens unnecessarily
- Stream output instead of building in memory
```

**Estimated time:** 1 week (if prioritized)

---

## 5. Production Checklist

### Pre-Release Requirements

- [ ] All Phase 19.1 tests pass
- [ ] All Phase 19.2 fragment tests pass  
- [ ] Error messages are helpful (user tested)
- [ ] Hygiene works in all documented cases
- [ ] Performance is acceptable (< 10ms for typical macros)
- [ ] Documentation is complete
- [ ] Examples cover common use cases
- [ ] Breaking changes documented (if any)
- [ ] Migration guide for existing macros (if syntax changed)

### Quality Gates

```
Gate 1: Compilation
├── cargo build --all
├── No warnings in macro_engine.rs
└── All existing tests pass

Gate 2: Unit Tests  
├── cargo test -p mellis-semantic macro
├── 100% fragment kind coverage
├── 100% repetition form coverage
└── Hygiene tests: no regressions

Gate 3: Integration
├── cargo test -p mellis-driver macro
├── Real-world macro examples work
├── Cross-module macros work
└── Performance benchmarks pass

Gate 4: Documentation
├── docs/MacroSystem.md complete
├── docs/MacroCookbook.md has 10+ examples
├── All API documented
└── User tested (alpha testers)

Gate 5: Release
├── Version bump (semver)
├── CHANGELOG.md updated
├── Migration guide if needed
└── Announcement prepared
```

---

## 6. Timeline & Resources

### Estimated Schedule

```
Week 1-2:   Phase 19.1 - Repetition Engine (CRITICAL)
Week 3:     Phase 19.2 - Extended Fragments  
Week 4:     Phase 19.3 - Error Handling
Week 5:     Phase 19.4 - Hygiene System
Week 6-7:   Phase 19.5 - Testing
Week 8:     Phase 19.6 - Documentation
Week 9-10:  Polish & Release

Total: 10 weeks (full-time)
```

### Dependencies

- Phase 19.1 must complete before 19.2-19.6 can fully validate
- Phase 19.3-19.4 can be done in parallel after 19.1
- Phase 19.5 testing depends on 19.1-19.4

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Repetition parsing is harder than expected | Medium | High | Prototype first, spec carefully |
| Hygiene edge cases discovered late | Medium | Medium | Extensive testing in 19.5 |
| Performance issues with complex macros | Medium | Medium | Benchmark early, optimize if needed |
| Documentation takes longer than estimated | Low | Low | Start early, parallelize |

---

## 7. Success Metrics

### Quantitative

- [ ] 100% of documented fragment kinds implemented
- [ ] 100% of repetition forms implemented and tested
- [ ] Macro expansion < 10ms for macros with < 500 tokens
- [ ] Zero known crashes in macro expansion
- [ ] Documentation covers 90% of user questions (tracked via issues)

### Qualitative

- [ ] Users can write `vec![]` style macros
- [ ] Users can write `format!` style macros with repetition
- [ ] Error messages help users fix their macros
- [ ] Hygiene prevents common bugs
- [ ] Performance is not a concern for typical use cases

---

## 8. Future Enhancements (Post-Production)

### Phase 20 Ideas

1. **Procedural macros** (custom Derive, attribute macros)
2. **Macro by example 2.0** (improved syntax, better errors)
3. **Macro debugging UI** (IDE integration)
4. **Macro profiling** (which macros are slow)
5. **Built-in macro stdlib** (`vec!`, `format!`, `assert!`)

---

## Appendix A: Grammar Reference (Target)

```ebnf
macro_definition ::= "macro" identifier "{" macro_rule* "}"
macro_rule       ::= pattern "=>" transcriber
pattern          ::= pattern_item*
pattern_item     ::= literal
                  | metavariable
                  | group
                  | repetition
literal          ::= any_token_except_special
metavariable     ::= "@" identifier ":" fragment_specifier
fragment_specifier ::= "expr" | "ident" | "ty" | "stmt" | "block" 
                    | "item" | "tt" | "pat" | "path" | "lifetime" 
                    | "meta"
group            ::= "(" pattern* ")"
                  | "[" pattern* "]"
                  | "{" pattern* "}"
repetition       ::= "$(" pattern* ")" repetition_suffix
repetition_suffix ::= "" | "*" | "+" | "?" 
                  | ("*" | "+" | "?") ":" separator
separator       ::= literal

transcriber      ::= transcriber_item*
transcriber_item ::= literal
                  | metavar_reference
                  | group
                  | repetition_expansion
metavar_reference ::= "@" identifier
group            ::= "(" transcriber* ")"
                  | "[" transcriber* "]"
                  | "{" transcriber* "}"
repetition_expansion ::= "$(" transcriber* ")" repetition_suffix
```

---

## Appendix B: Rust Parity Checklist

| Rust Feature | Mellis Status |
|--------------|---------------|
| `macro_rules!` syntax | ✅ Equivalent |
| `$()*` repetition | ❌ (Phase 19.1) |
| `$()+` repetition | ❌ (Phase 19.1) |
| `$()?` repetition | ❌ (Phase 19.1) |
| Separator `$(,)` | ❌ (Phase 19.1) |
| Nested `$()` | ❌ (Phase 19.1) |
| `tt` fragment | ❌ (Phase 19.2) |
| `pat` fragment | ❌ (Phase 19.2) |
| `path` fragment | ❌ (Phase 19.2) |
| `$:pat if $cond` | ❌ Future |
| Recursive macros | ✅ |
| Hygiene | ✅ (with fixes) |
| `$crate::` | ❌ Future |
| Modular hygiene | ⚠️ Partial |

**Target: ~90% Rust macro expressiveness after Phase 19**
