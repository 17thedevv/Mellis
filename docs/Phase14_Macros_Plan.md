# Phase 14: Macros — Declarative Metaprogramming Implementation Plan

> **Document Purpose**: Standalone export for external agent implementation. Contains Phase 14 plan, sub-phases, grammar analysis, implementation details, and verification criteria.

---

## 1. Overview

Phase 14 implements **Declarative Macros** in the Mellis language and compiler. This allows compile-time code generation through pattern matching and substitution, similar to Rust's `macro_rules!`.

### Key Features
- **Macro definitions**: Named macros with pattern-matching rules
- **Fragment specs**: `expr`, `ident`, `ty`, `stmt`, `block`, `item`
- **Macro invocation**: `name!()` / `name![]` / `name!{}`
- **Hygiene**: Scoped hygiene to prevent unintended variable capture
- **Repetition**: `$(...)*` and `$(...)+` patterns

### Grammar (from grammar.md)
```ebnf
// --- MACRO DEFINITION ---
macro_decl ::= annotation* "export"? KW_MACRO IDENTIFIER "(" macro_params? ")" block_stmt

macro_params ::= macro_param ("," macro_param)* ("," macro_variadic_param)?
               | macro_variadic_param

macro_param ::= AT IDENTIFIER ":" macro_frag_spec

macro_variadic_param ::= AT IDENTIFIER ":" macro_frag_spec "..."

macro_frag_spec ::= "expr" | "ident" | "ty" | "stmt" | "block" | "item"

// --- MACRO INVOCATION ---
macro_call_expr ::= IDENTIFIER "!" "(" macro_call_args? ")"
                  | IDENTIFIER "!" "[" macro_call_args? "]"

macro_call_stmt ::= IDENTIFIER "!" "(" macro_call_args? ")" ";"
                  | IDENTIFIER "!" "[" macro_call_args? "]" ";"
                  | IDENTIFIER "!" "{" macro_call_args? "}"

macro_call_args ::= macro_call_arg ("," macro_call_arg)* ","?

macro_call_arg  ::= expression 
                  | type 
                  | block_stmt
                  | IDENTIFIER

// --- MACRO EXPANSION LOOP ---
macro_expand_for ::= AT KW_FOR AT IDENTIFIER KW_IN AT IDENTIFIER block_stmt
```

### Syntax Examples
```mellis
// Macro definition
macro println($expr: expr) {
    print($expr);
    print("\n");
}

// Macro with repetition
macro vec!($($elem: expr),*) {
    [ $($elem,)* ]
}

// Macro invocation
println!(42);
let v = vec![1, 2, 3];

// Statement macro
println!("Hello");

// Block macro
some_macro! { 
    let x = 1; 
    x + 1 
}
```

### Dependencies
```
Phase 13 (dyn Trait) ──→ Phase 14 (Macros) ──→ Phase 15 (Comptime)
                                              └──→ async/await (Phase 16) [@ placeholder]
```

---

## 2. Feature Breakdown

| # | Feature | Priority | Description |
|---|---|---|---|
| 14A | AST Nodes | P0 | `Decl::Macro`, `Expr::MacroCall`, fragment specs |
| 14B | Parser | P0 | Parse macro definitions and invocations |
| 14C | Symbol Table | P0 | Macro name resolution |
| 14D | Macro Expansion Engine | P0 | Pattern matching, substitution, repetition |
| 14E | Hygiene System | P0 | Prevent variable capture, scope tracking |
| 14F | Type Integration | P1 | Macros generating types (`ty` fragment) |
| 14G | MVIR Generation | P1 | Expanded code goes to MVIR generator |
| 14H | Cross-module Macros | P2 | `export macro` and module imports |

---

## 3. Sub-Phase Details

### 14A — AST Nodes (P0)

#### Deliverables
- [ ] **`Decl::Macro`**: New declaration variant for macro definitions
- [ ] **`Expr::MacroCall`**: New expression variant for macro invocations
- [ ] **Fragment Spec Enum**: `MacroFragment::Expr | Ident | Ty | Stmt | Block | Item`
- [ ] **Macro Pattern**: Pattern matching rule with matcher and transcriber
- [ ] **Macro Rule**: Collection of patterns with precedence

#### Data Structures
```rust
// Fragment specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroFragment {
    Expr,   // $e:expr  - expressions
    Ident,  // $i:ident - identifiers
    Ty,     // $t:ty    - types
    Stmt,   // $s:stmt  - statements
    Block,  // $b:block - block statements
    Item,   // $i:item  - declarations
}

// Macro pattern matcher
#[derive(Debug, Clone)]
pub struct MacroMatcher {
    pub fragment: MacroFragment,
    pub name: SymbolId,        // $name binding
    pub repetition: Option<(char, RepetitionKind)>, // *, +, ?
}

// Macro transcriber (output)
#[derive(Debug, Clone)]
pub struct MacroTranscriber {
    pub kind: TranscriberKind,
    pub tokens: Vec<Token>,     // Raw tokens with $name references
    pub repetition: Option<(char, RepetitionKind)>,
}

// A single rule in a macro
#[derive(Debug, Clone)]
pub struct MacroRule {
    pub matchers: Vec<MacroMatcher>,
    pub transcriber: MacroTranscriber,
}

// Macro definition declaration
#[derive(Debug, Clone)]
pub struct MacroDecl {
    pub name: Ident,
    pub rules: Vec<MacroRule>,
    pub is_export: bool,
    pub repetition_hygiene: bool, // true = Rust-like
}

// Macro invocation expression
#[derive(Debug, Clone)]
pub struct MacroCall {
    pub path: Path,              // macro name
    pub delimiter: Delimiter,    // (), [], {}
    pub args: Vec<TokenStream>, // Raw token stream
    pub span: Span,
}
```

#### Key Files to Modify
- `crates/mellis-ast/src/decl.rs` — `Decl::Macro`
- `crates/mellis-ast/src/expr.rs` — `Expr::MacroCall`
- `crates/mellis-ast/src/lib.rs` — exports and arena integration

---

### 14B — Parser (P0)

#### Deliverables
- [ ] **Macro definition parsing**: Parse `macro foo($x: expr) { ... }`
- [ ] **Macro invocation parsing**: Parse `foo!(...)` in expressions
- [ ] **Fragment parsing**: Handle `$name:frag` patterns
- [ ] **Repetition parsing**: Handle `$(...)*` and `$(...)+`
- [ ] **Token stream capture**: Capture raw tokens for transcriber

#### Parser Implementation
```rust
fn parse_macro_decl(&mut self) -> Result<DeclId, ()> {
    // keyword already consumed: `macro`
    let name = self.parse_ident()?;
    
    self.expect(TokenKind::LParen)?;
    let mut rules = Vec::new();
    
    // Parse rules
    loop {
        let mut matchers = Vec::new();
        
        // Parse pattern until `{`
        while !self.check(TokenKind::LBrace) {
            if self.eat(TokenKind::DollarSign) {
                let ident = self.parse_ident()?;
                self.expect(TokenKind::Colon)?;
                let frag = self.parse_fragment_spec()?;
                matchers.push(MacroMatcher {
                    name: ident,
                    fragment: frag,
                    repetition: None,
                });
            } else {
                // Literal token in pattern
                self.advance();
            }
            
            if self.check(TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        
        // Parse transcriber
        self.expect(TokenKind::LBrace)?;
        let transcriber = self.parse_token_stream_until(RBrace)?;
        self.expect(TokenKind::RBrace)?;
        
        rules.push(MacroRule { matchers, transcriber });
        
        if self.check(TokenKind::Comma) {
            self.advance();
        } else {
            break;
        }
    }
    
    self.expect(TokenKind::RParen)?;
    self.consume(TokenKind::LBrace)?; // Body
    // Parse body if any...
    
    Ok(self.arena.alloc_decl(Decl::Macro { name, rules, is_export: false }))
}
```

#### Current State: Token `KwMacro` exists in lexer
- Current parsing in `expr.rs` treats `foo!(...)` as regular `Call` (line 325-364)
- Need to add `Expr::MacroCall` to AST
- Need to distinguish macro calls from function calls during resolution

#### Key Files to Modify
- `crates/mellis-parser/src/decl.rs` — `parse_macro_decl`
- `crates/mellis-parser/src/expr.rs` — `parse_macro_call` (currently parses as Call)
- `crates/mellis-parser/src/lib.rs` — Parser struct changes

---

### 14C — Symbol Table Integration (P0)

#### Deliverables
- [ ] **Macro scope**: Macros in their own scope, not values
- [ ] **Resolution**: `foo!()` resolves to macro definition
- [ ] **Export/Import**: `export macro foo;` and `import foo;`

#### Design: Macros are NOT values
```rust
// Unlike functions, macros are NOT first-class values
// They cannot be passed as arguments or stored in variables

// Valid:
macro foo($x: expr) { ... }
foo!(42);

// Invalid (not supported in MVP):
let m = foo;  // ERROR: macros are not values
m!(42);       // ERROR

// Also invalid:
fn takes_macro(m: Macro) {}  // No macro type in MVP
```

#### Resolution Strategy
1. During name resolution, track macros separately from values
2. When encountering `name!()`, look up in macro table first
3. If found, flag as macro call
4. If not found, treat as regular call (function or method)

#### Key Files to Modify
- `crates/mellis-semantic/src/resolver.rs` — macro resolution
- `crates/mellis-semantic/src/semantic_tables.rs` — `macros: HashMap<SymbolId, MacroDecl>`

---

### 14D — Macro Expansion Engine (P0)

#### Deliverables
- [ ] **Token stream**: Represent raw tokens for macro processing
- [ ] **Pattern matching**: Match `$x:expr` against token stream
- [ ] **Repetition handling**: `$(...)*` and `$(...)+` matching
- [ ] **Transcription**: Replace patterns with captured tokens

#### Token Stream Representation
```rust
#[derive(Debug, Clone)]
pub struct TokenStream {
    pub tokens: Vec<Token>,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Parenthesis,  // ()
    Bracket,      // []
    Brace,        // {}
    None,         // Delimiter-less (e.g., macro_rules! style)
}
```

#### Pattern Matching Algorithm
```rust
fn match_pattern(
    stream: &TokenStream,
    pattern: &[MacroMatcher],
) -> Result<HashMap<SymbolId, CapturedFragment>, MatchError> {
    let mut captures = HashMap::new();
    let mut token_iter = stream.tokens.iter().peekable();
    let mut pattern_idx = 0;
    
    while pattern_idx < pattern.len() {
        let matcher = &pattern[pattern_idx];
        
        match &matcher.repetition {
            None => {
                // Simple matching
                let captured = match matcher.fragment {
                    MacroFragment::Expr => capture_expr(&mut token_iter)?,
                    MacroFragment::Ident => capture_ident(&mut token_iter)?,
                    MacroFragment::Ty => capture_type(&mut token_iter)?,
                    MacroFragment::Stmt => capture_stmt(&mut token_iter)?,
                    MacroFragment::Block => capture_block(&mut token_iter)?,
                    MacroFragment::Item => capture_item(&mut token_iter)?,
                };
                captures.insert(matcher.name, captured);
                pattern_idx += 1;
            }
            Some((sep, kind)) => {
                // Repetition matching
                let captures_list = match_repetition(
                    &mut token_iter,
                    matcher.fragment,
                    *sep,
                    *kind,
                )?;
                captures.insert(matcher.name, CapturedFragment::Repeated(captures_list));
                pattern_idx += 1;
            }
        }
    }
    
    // Ensure all tokens consumed
    if token_iter.peek().is_some() {
        return Err(MatchError::ExcessTokens);
    }
    
    Ok(captures)
}
```

#### Repetition Handling
```rust
// Pattern: $( $x:expr ),* 
// Input: 1, 2, 3
// Captures: [1], [2], [3]

fn match_repetition(
    tokens: &mut Peekable<Iter<Token>>,
    fragment: MacroFragment,
    separator: char,
    kind: RepetitionKind,
) -> Result<Vec<CapturedFragment>, MatchError> {
    let mut results = Vec::new();
    let separator_token = TokenKind::from_char(separator);
    
    loop {
        // Try to match one repetition
        match capture_fragment(tokens, fragment) {
            Ok(captured) => {
                results.push(captured);
                
                // Check for separator
                if tokens.peek() == Some(&&Token { kind: separator_token, .. }) {
                    tokens.next();
                    continue;
                } else {
                    break;
                }
            }
            Err(_) if results.is_empty() && kind == RepetitionKind::Star => {
                // Zero or more: empty is OK
                break;
            }
            Err(_) if results.is_empty() && kind == RepetitionKind::Plus => {
                // One or more: empty is error
                return Err(MatchError::ExpectedFragment(fragment));
            }
            Err(_) => {
                break;  // Done with repetitions
            }
        }
    }
    
    Ok(results)
}
```

#### Transcription (Template Expansion)
```rust
fn expand_transcriber(
    transcriber: &MacroTranscriber,
    captures: &HashMap<SymbolId, CapturedFragment>,
) -> TokenStream {
    let mut output = TokenStream::new();
    
    for token in &transcriber.tokens {
        if let TokenKind::DollarSign = token.kind {
            // Look ahead for identifier
            let next = tokens.next();
            if let Some(name_token) = next {
                if let Some(captured) = captures.get(&name_token.symbol) {
                    output.extend(expand_captured(captured));
                }
            }
        } else {
            output.push(token.clone());
        }
    }
    
    output
}
```

#### Key Files to Create/Modify
- `crates/mellis-semantic/src/macro_engine.rs` — NEW: macro expansion engine
- `crates/mellis-semantic/src/lib.rs` — export macro engine
- `crates/mellis-semantic/src/resolver.rs` — integration

---

### 14E — Hygiene System (P0)

#### Deliverables
- [ ] **Macro scope**: Each macro expansion gets a unique scope
- [ ] **Hygienic variables**: Generated identifiers don't collide with user code
- **Scope tracking**: Track which variables come from which expansion
- [ ] **Span mapping**: Map expanded tokens back to macro invocation site

#### Hygiene Implementation
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroHygiene {
    pub site_id: u32,           // Unique ID of macro invocation site
    pub scope_depth: u32,      // Nesting depth
}

#[derive(Debug, Clone)]
pub struct HygienicIdent {
    pub raw_name: String,
    pub hygiene: MacroHygiene,
    pub original_span: Span,   // For error messages
}

// When generating a new identifier:
// - Create unique suffix: "__m{site_id}_{counter}"
// - Attach macro hygiene
// - Mark as "generated" for error reporting
```

#### Scope Conflict Resolution
```rust
// When hygiene scopes overlap:
// User code: let x = 1;
// Macro generates: let x = 2; // Should NOT shadow user's x

// Solution: Gensym generated identifiers
// User x: "x" (hygiene: user scope)
// Macro x: "__m42_0" (hygiene: macro_42 scope)

// In error messages, map back:
// "error: cannot find 'x'" -> "macro-generated 'x' (from line 5)"
```

#### Key Files to Modify
- `crates/mellis-common/src/ids.rs` — `MacroHygiene`, `HygienicIdent`
- `crates/mellis-semantic/src/macro_engine.rs` — hygiene logic

---

### 14F — Type Integration (P1)

#### Deliverables
- [ ] **Type fragment**: `$t:ty` captures type syntax
- [ ] **Type integration**: Expanded types work with type checker
- [ ] **Generic expansion**: Macros generating generic code

#### Fragment Capture for Types
```rust
fn capture_type(tokens: &mut Peekable<Iter<Token>>) -> Result<TokenStream, MatchError> {
    let mut depth = 0;
    let mut end_tokens = Vec::new();
    
    while let Some(token) = tokens.peek() {
        match token.kind {
            TokenKind::LAngle => depth += 1,
            TokenKind::RAngle => {
                if depth == 0 { break; }
                depth -= 1;
            }
            TokenKind::Comma | TokenKind::Semicolon if depth == 0 => break,
            _ => {}
        }
        end_tokens.push(tokens.next().unwrap());
    }
    
    if end_tokens.is_empty() {
        Err(MatchError::ExpectedFragment(MacroFragment::Ty))
    } else {
        Ok(TokenStream::new(end_tokens))
    }
}
```

#### Key Files to Modify
- `crates/mellis-semantic/src/macro_engine.rs` — type fragment handling
- `crates/mellis-semantic/src/typechecker.rs` — expanded code type checking

---

### 14G — MVIR Generation (P1)

#### Deliverables
- [ ] **Expanded code pipeline**: Expanded AST → MVIR generator
- [ ] **Deferred expansion**: Expand macros before MVIR generation
- [ ] **Error propagation**: Diagnostics from macro expansion

#### Expansion Pipeline
```
Source (.ms)
    │
    ▼
Parser → AST
    │
    ▼
Macro Expansion (Phase 14D)
    │
    ▼
Expanded AST (without macro nodes)
    │
    ▼
Semantic Analysis (TypeChecker)
    │
    ▼
MVIR Generator
    │
    ▼
LLVM Backend → Executable
```

#### Key Files to Modify
- `crates/mellis-mvir/src/generator.rs` — receives expanded AST
- `crates/mellis-semantic/src/resolver.rs` — macro expansion ordering

---

### 14H — Cross-module Macros (P2)

#### Deliverables
- [ ] **Export macros**: `export macro foo;`
- [ ] **Import macros**: `import { foo } from "module";`
- [ ] **MLib serialization**: Store macros in `.mlib` artifacts

#### Design: Macros are NOT serialized in MVP
```mellis
// For Phase 14 MVP: Macros are NOT cross-module
// Each module compiles its own macros independently

// module_a.ms
export macro foo($x: expr) { ... }

// module_b.ms  
import "module_a"; // foo is NOT imported

// To use foo in module_b, define it there or inline
```

#### Rationale
- Macro expansion requires source tokens
- Storing expanded AST is complex
- Most macros are utility functions, not API

#### Future (Phase 15+ Comptime)
- Store macro AST (not expanded code)
- Re-expand on import
- Requires stable AST serialization

#### Key Files to Modify
- `crates/mellis-semantic/src/resolver.rs` — export/import tracking
- `crates/mellis-mlib/src/ir.rs` — (Future: macro storage)

---

## 4. Compiler Crate Map

| Crate | Phase 14 Changes |
|---|---|
| `mellis-lexer` | Already has `KwMacro` token |
| `mellis-ast` | `Decl::Macro`, `Expr::MacroCall`, fragment enums |
| `mellis-parser` | Macro definition/invocation parsing |
| `mellis-semantic` | `macro_engine.rs` (NEW), resolution, expansion |
| `mellis-mvir` | Receives expanded AST |
| `mellis-mlib` | (Future: macro serialization) |

---

## 5. Error Handling

### Macro-Specific Diagnostics
```rust
#[derive(Debug, Clone)]
pub enum MacroError {
    UndefinedMacro(String),           // "foo!(...)" but no macro named foo
    WrongArity(u32, u32),           // Expected 2 args, got 3
    MatchFailed(String),             // Pattern didn't match input
    RepetitionMismatch(String),      // $(a)* mismatch
    InvalidFragment(String),         // "expr" where "ident" expected
    HygieneConflict(String),         // Generated identifier collision
}
```

### Error Message Examples
```
error: no macro named `println` in scope
  --> file.ms:5:1
   |
5  | println!("hello");
   | ^^^^^^^

error: macro expansion failed: pattern `(expr,)` matched 0 times
  --> file.ms:3:1
   |
3  | vec![] // vec![$($elem:expr),*] requires at least 1 element
   | ^^^^^^
```

---

## 6. Test Plan

### Positive Tests
- [ ] `macro_basic.ms` — Simple macro definition and use
- [ ] `macro_expr.ms` — `$x:expr` fragment
- [ ] `macro_ident.ms` — `$x:ident` fragment
- [ ] `macro_ty.ms` — `$t:ty` generating types
- [ ] `macro_repetition_star.ms` — `$(x)*` (zero or more)
- [ ] `macro_repetition_plus.ms` — `$(x)+` (one or more)
- [ ] `macro_multiple_rules.ms` — Multiple patterns with overloading

### Negative Tests
- [ ] `macro_undefined.ms` — Calling undefined macro
- [ ] `macro_wrong_arity.ms` — Wrong number of arguments
- [ ] `macro_match_failed.ms` — Pattern doesn't match
- [ ] `macro_invalid_fragment.ms` — Wrong fragment for context

### Hygiene Tests
- [ ] `macro_hygiene_basic.ms` — Generated vars don't shadow user vars
- [ ] `macro_hygiene_nested.ms` — Nested macro expansion hygiene

### Example Test Files

**macro_basic.ms:**
```mellis
macro double($x: expr) {
    $x * 2
}

fn main() {
    let n = 21;
    let result = double!(n);
    print(result);  // Should print 42
}
```

**macro_repetition.ms:**
```mellis
macro sum($($x: expr),*) {
    $(
        $x
    )|*  // Repeat with +: $( + $x)*
}

fn main() {
    let result = sum!(1, 2, 3, 4, 5);
    print(result);  // Should print 15
}
```

---

## 7. Implementation Order

```
Week 1: 14A + 14B (AST nodes, Parser)
         - Add Decl::Macro, Expr::MacroCall
         - Parse macro definitions
         - Parse macro invocations (change from Call)

Week 2: 14C + 14D (Symbol table, Expansion engine)
         - Resolve macro names
         - Implement pattern matching
         - Implement repetition handling

Week 3: 14E + 14F (Hygiene, Type integration)
         - Implement hygiene system
         - Handle $t:ty fragment
         - Integrate with type checker

Week 4: 14G + Tests (MVIR generation, Verification)
         - Connect expansion to MVIR pipeline
         - Write and run tests
         - Fix bugs
```

---

## 8. Known Challenges

### Challenge 1: Token Stream vs AST
**Issue**: Macros work on raw tokens, but the parser produces AST.
**Solution**: Keep raw tokens in `Expr::MacroCall`, expand to AST before semantic analysis.

### Challenge 2: Fragment Ambiguity
**Issue**: `$x` could be `expr`, `ident`, or `ty` depending on context.
**Solution**: Use greedy matching; parser tries each fragment type.

### Challenge 3: Nested Macros
**Issue**: Macros calling macros: `foo!(bar!(x))`
**Solution**: Recursive expansion; expand innermost first.

### Challenge 4: Hygiene with Closures
**Issue**: Macro capturing variables + closures interaction.
**Solution**: Hygiene applies to generated identifiers only; captured vars are explicit.

---

## 9. Success Criteria

Phase 14 is complete when:
1. `macro foo($x: expr) { ... }` parses correctly
2. `foo!(42)` expands to the transcribed code
3. `$(x)*` and `$(x)+` repetition works
4. `$x:ty` generates valid types
5. Generated identifiers don't collide with user identifiers
6. All positive tests pass
7. All negative tests produce expected diagnostics
8. Expanded code passes through full compilation pipeline
