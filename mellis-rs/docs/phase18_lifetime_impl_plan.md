# Phase 18: Lifetime System — Implementation Plan (Còn lại)

## Mục tiêu
Hoàn thành implementation của lifetime system theo Implementation Contract đã được freeze.

## Progress hiện tại

```
✓ 1. AST nodes for LifetimeExpr
✓ 2. Parser for life_from, |
✓ 3. Parser for where outlives  
✓ 4. Resolver for lifetime expressions
✓ 5. Provenance representation
✓ 6. Type consistency validation
✓ 7. Constraint generation
✓ 8. Basic solver (SAT/UNSAT)
✓ 9. SCC handling

Còn lại:
○ 10. Function call checking
○ 11. Return checking
○ 12. Diagnostics
○ 13. MVIR boundary check
○ 14. Tests
```

---

## Step 10: Function Call Checking

### Mục tiêu
Verify caller satisfies callee constraints khi gọi hàm có explicit lifetime signature.

### Example
```mellis
fn callee(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)

fn caller() {
    let x: i32 = 1;
    let y: i32 = 2;
    let r = callee(&x, &y);  // OK: x outlives y (both static, x declared first)
}
```

### Implementation

**File**: `mellis-rs/crates/mellis-semantic/src/lifetime.rs`

```rust
/// Check that caller arguments satisfy callee's lifetime constraints.
pub fn check_fn_call_constraints(
    callee_decl: &mellis_ast::Decl,
    caller_args: &[mellis_ast::ExprId],
    ctx: &mut SemanticContext,
) -> Result<(), LifetimeError> {
    let Decl::Function { params, lifetime_signature, .. } = callee_decl else {
        return Ok(()); // Not a function or no lifetime signature
    };

    for constraint in &lifetime_signature.constraints {
        // outlives(a, b) → 'b ≤ 'a
        // Verify: lifetime of b's argument ≥ lifetime of a's argument
        let first_param_idx = find_param_index(params, &constraint.first)?;
        let second_param_idx = find_param_index(params, &constraint.second)?;

        let first_arg = &caller_args[first_param_idx];
        let second_arg = &caller_args[second_param_idx];

        // Check: second_arg's lifetime ≥ first_arg's lifetime
        if !ctx.tables.expr_lifetimes.contains_key(second_arg) {
            return Err(LifetimeError::UnresolvedLifetime(
                format!("cannot determine lifetime of argument {}", second_param_idx)
            ));
        }

        // This is a simplified check - actual implementation needs
        // lifetime ordering comparison
    }

    Ok(())
}
```

**Thêm vào semantic_tables.rs**:
```rust
/// Maps expressions to their inferred lifetimes
pub expr_lifetimes: BTreeMap<ExprId, LifetimeId>,
```

### Deliverables
- [ ] Function `check_fn_call_constraints` trong lifetime.rs
- [ ] Integration vào typechecker khi handle function calls
- [ ] Error messages cho constraint violations

---

## Step 11: Return Checking

### Mục tiêu
Verify actual return matches declared lifetime signature.

### Example
```mellis
fn foo(x: &i32) -> &i32 life_from(x) {
    return x;  // OK: provenance of x matches life_from(x)
}

fn bar(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    if condition {
        return a;  // OK: a is in provenance set
    } else {
        return b;  // OK: b is in provenance set
    }
}
```

### Implementation

**File**: `mellis-rs/crates/mellis-semantic/src/lifetime.rs`

```rust
/// Check that a return statement's provenance matches the declared lifetime signature.
pub fn check_return_provenance(
    decl: &mellis_ast::Decl,
    return_expr: &mellis_ast::ExprId,
    ctx: &mut SemanticContext,
) -> Result<(), LifetimeError> {
    let Decl::Function { lifetime_signature, .. } = decl else {
        return Ok(());
    };

    let Some(provenance) = &lifetime_signature.provenance else {
        return Ok(()); // No explicit lifetime signature
    };

    match provenance {
        LifetimeExpr::Provenance(ident) => {
            // Return must be from the declared parameter
            // Verify: return_expr's provenance includes the declared parameter
        }
        LifetimeExpr::ProvenanceSet(idents) => {
            // Return must be from one of the declared parameters
            // Verify: return_expr's provenance is a subset of the declared set
        }
    }

    Ok(())
}
```

**Thêm vào typechecker** khi xử lý return statements:
```rust
// Trong typechecker khi handle Stmt::Return
if let Some(expr) = return_expr {
    crate::lifetime::check_return_provenance(decl, expr, self.ctx)?;
}
```

### Deliverables
- [ ] Function `check_return_provenance` trong lifetime.rs
- [ ] Integration vào typechecker cho return statements
- [ ] Error messages cho provenance mismatches

---

## Step 12: Diagnostics

### Mục tiêu
Cải thiện error messages cho lifetime-related diagnostics.

### Error Types cần handle

```rust
#[derive(Debug, Clone)]
pub enum LifetimeError {
    /// Constraint cannot be satisfied.
    /// Example: "lifetime 'b must outlive 'a, but 'b is shorter"
    UnsatisfiableConstraint {
        constraint: LifetimeConstraintExpr,
        reason: String,
    },

    /// Type inconsistency in alternatives.
    /// Example: "cannot use '&str' and '&i32' in same life_from"
    TypeInconsistency {
        expected_type: String,
        found_type: String,
        span: Span,
    },

    /// Unresolved lifetime identifier.
    /// Example: "lifetime 'x does not refer to any parameter"
    UnresolvedLifetime {
        name: String,
        span: Span,
    },

    /// Analysis timeout.
    AnalysisTimeout,

    /// Return provenance mismatch.
    /// Example: "return value does not have expected provenance 'a"
    ReturnProvenanceMismatch {
        expected: String,
        found: String,
        span: Span,
    },

    /// Function call constraint violation.
    /// Example: "argument 2 does not outlive argument 1 as required"
    ConstraintViolation {
        callee: String,
        constraint: String,
        span: Span,
    },
}
```

### Implementation

**File**: `mellis-rs/crates/mellis-common/src/diagnostic.rs`

Thêm helper functions:
```rust
impl Diagnostic {
    pub fn lifetime_error(error: LifetimeError) -> Self {
        match error {
            LifetimeError::UnsatisfiableConstraint { constraint, reason } => {
                Diagnostic::error(format!(
                    "lifetime constraint '{} ≤ '{}' cannot be satisfied: {}",
                    constraint.shorter.0, constraint.longer.0, reason
                ))
            }
            LifetimeError::TypeInconsistency { expected_type, found_type, span } => {
                Diagnostic::error(format!(
                    "type mismatch in lifetime alternatives: expected {}, found {}",
                    expected_type, found_type
                )).with_span(span)
            }
            LifetimeError::UnresolvedLifetime { name, span } => {
                Diagnostic::error(format!(
                    "lifetime '{}' does not refer to any parameter in scope",
                    name
                )).with_span(span)
            }
            LifetimeError::AnalysisTimeout => {
                Diagnostic::error("lifetime analysis exceeded resource limit")
            }
            LifetimeError::ReturnProvenanceMismatch { expected, found, span } => {
                Diagnostic::error(format!(
                    "return value provenance mismatch: expected '{}', found '{}'",
                    expected, found
                )).with_span(span)
            }
            LifetimeError::ConstraintViolation { callee, constraint, span } => {
                Diagnostic::error(format!(
                    "function '{}' requires {}, but condition not satisfied",
                    callee, constraint
                )).with_span(span)
            }
        }
    }
}
```

### Deliverables
- [ ] Enum variants đầy đủ cho all error cases
- [ ] Diagnostic helpers với meaningful messages
- [ ] Span information cho all errors
- [ ] Integration vào existing diagnostic system

---

## Step 13: MVIR Boundary Check

### Mục tiêu
Verify no unresolved lifetime variables reach MVIR/LLVM codegen.

### Contract Requirement
```
// MVIR/LLVM MUST NOT require runtime lifetime values.
fn verify_before_codegen(sig: &FnLifetimeSignature, ctx: &SemanticCtx) -> Result<(), Error> {
    if has_unresolved_lifetimes(sig, ctx) {
        return Err(Error::UnresolvedLifetime);
    }
    // MVIR receives verified types, provenance-independent semantics
    Ok(())
}
```

### Implementation

**File**: `mellis-rs/crates/mellis-semantic/src/lifetime.rs`

```rust
/// Checks that all lifetime information is resolved before codegen.
/// This is the final verification pass for the lifetime system.
pub struct LifetimeVerifier<'a> {
    ctx: &'a SemanticContext,
}

impl<'a> LifetimeVerifier<'a> {
    pub fn new(ctx: &'a SemanticContext) -> Self {
        Self { ctx }
    }

    /// Verifies that a function's lifetime signature is fully resolved.
    pub fn verify_fn_signature(&self, decl: &mellis_ast::Decl) -> Result<(), LifetimeError> {
        let Decl::Function { lifetime_signature, .. } = decl else {
            return Ok(());
        };

        // Check provenance is resolved
        if let Some(provenance) = &lifetime_signature.provenance {
            self.verify_provenance(provenance)?;
        }

        // Check all constraints are satisfied
        for constraint in &lifetime_signature.constraints {
            self.verify_constraint(constraint)?;
        }

        Ok(())
    }

    fn verify_provenance(&self, provenance: &LifetimeExpr) -> Result<(), LifetimeError> {
        match provenance {
            LifetimeExpr::Provenance(span) => {
                // Verify the identifier resolves to a parameter
                let name = extract_name_from_span(span);
                if !self.is_valid_parameter(&name) {
                    return Err(LifetimeError::UnresolvedLifetime {
                        name,
                        span: *span,
                    });
                }
            }
            LifetimeExpr::ProvenanceSet(idents) => {
                for span in idents {
                    let name = extract_name_from_span(span);
                    if !self.is_valid_parameter(&name) {
                        return Err(LifetimeError::UnresolvedLifetime {
                            name,
                            span: *span,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn verify_constraint(&self, constraint: &LifetimeConstraint) -> Result<(), LifetimeError> {
        let first_name = extract_name_from_span(&constraint.first);
        let second_name = extract_name_from_span(&constraint.second);

        if !self.is_valid_parameter(&first_name) {
            return Err(LifetimeError::UnresolvedLifetime {
                name: first_name,
                span: constraint.first,
            });
        }
        if !self.is_valid_parameter(&second_name) {
            return Err(LifetimeError::UnresolvedLifetime {
                name: second_name,
                span: constraint.second,
            });
        }
        Ok(())
    }

    fn is_valid_parameter(&self, name: &str) -> bool {
        // Check if name refers to a parameter in current scope
        // Implementation depends on resolver integration
        true
    }
}

/// Final check before passing to MVIR.
pub fn verify_before_codegen(
    decl: &mellis_ast::Decl,
    ctx: &SemanticContext,
) -> Result<(), LifetimeError> {
    let verifier = LifetimeVerifier::new(ctx);
    verifier.verify_fn_signature(decl)
}
```

**Integration**: Gọi trong `mellis-driver` trước khi lower xuống MVIR.

### Deliverables
- [ ] `LifetimeVerifier` struct
- [ ] `verify_fn_signature` method
- [ ] `verify_before_codegen` entry point
- [ ] Integration vào driver pipeline

---

## Step 14: Tests

### Mục tiêu
Comprehensive test coverage cho tất cả lifetime functionality.

### Test Categories

#### 14.1 Basic Cases (Section 18.1 trong Contract)

```mellis
// Single ref with explicit annotation
fn single_origin(x: &i32) -> &i32 life_from(x)

// Alternative origins
fn multi_origin(a: &i32, b: &i32) -> &i32 life_from(a | b)
fn triple_origin(a: &i32, b: &i32, c: &i32) -> &i32 life_from(a | b | c)
```

**File**: `mellis-rs/crates/mellis-semantic/tests/lifetime_basic_tests.rs`

```rust
#[test]
fn test_single_origin() {
    let source = r#"
        fn foo(x: &i32) -> &i32 life_from(x) {
            return x;
        }
    "#;
    let result = compile(source);
    assert!(result.is_ok());
}

#[test]
fn test_multi_origin() {
    let source = r#"
        fn bar(a: &i32, b: &i32) -> &i32 life_from(a | b) {
            if true {
                return a;
            } else {
                return b;
            }
        }
    "#;
    let result = compile(source);
    assert!(result.is_ok());
}
```

#### 14.2 Constraints (Section 18.2)

```mellis
fn with_constraint(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)
```

#### 14.3 Type Checking Errors (Section 18.3)

```mellis
// Invalid: type mismatch in alternatives
fn invalid_alt_types(a: &i32, b: &str) -> &i32 life_from(a | b)
// Expected error: "cannot use '&str' in life_from with '&i32' return"
```

#### 14.4 Solver Tests (Section 18.5)

```rust
#[test]
fn test_solver_cycle_mutual_outlives() {
    let mut solver = LifetimeSolver::new();
    solver.register_lifetime("a", LifetimeIdent(0));
    solver.register_lifetime("b", LifetimeIdent(1));

    // 'a: outlives('b), 'b: outlives('a) → 'a = 'b
    solver.add_constraint(LifetimeConstraintExpr {
        longer: LifetimeIdent(1),
        shorter: LifetimeIdent(0),
    });
    solver.add_constraint(LifetimeConstraintExpr {
        longer: LifetimeIdent(0),
        shorter: LifetimeIdent(1),
    });

    match solver.solve() {
        SolveResult::Sat(assignment) => {
            // Both should map to same canonical lifetime
            assert_eq!(
                assignment.lifetimes.get(&LifetimeIdent(0)),
                assignment.lifetimes.get(&LifetimeIdent(1))
            );
        }
        _ => panic!("expected Sat"),
    }
}

#[test]
fn test_solver_unsat_bottleneck() {
    let mut solver = LifetimeSolver::new();
    solver.register_lifetime("a", LifetimeIdent(0));

    // Cannot have 'a ≤ ⊥
    solver.add_constraint(LifetimeConstraintExpr {
        longer: LifetimeIdent(0),
        shorter: LifetimeIdent(u32::MAX), // ⊥
    });

    match solver.solve() {
        SolveResult::Unsat(_) => {}
        _ => panic!("expected Unsat"),
    }
}
```

#### 14.5 Integration Tests

**File**: `mellis-rs/crates/mellis-driver/tests/lifetime_integration_tests.rs`

```rust
#[test]
fn test_lifetime_fn_call() {
    let source = r#"
        fn callee(x: &i32, y: &i32) -> &i32
            life_from(x)
            where outlives(x, y)

        fn caller() -> i32 {
            let a = 1;
            let b = 2;
            let r = callee(&a, &b);
            return *r;
        }
    "#;
    let result = compile_and_run(source);
    assert_eq!(result, 1);
}
```

### Test Files to Create

1. `mellis-rs/crates/mellis-semantic/tests/lifetime_basic_tests.rs`
2. `mellis-rs/crates/mellis-semantic/tests/lifetime_constraint_tests.rs`
3. `mellis-rs/crates/mellis-semantic/tests/lifetime_error_tests.rs`
4. `mellis-rs/crates/mellis-driver/tests/lifetime_integration_tests.rs`

### Deliverables
- [ ] Basic functionality tests
- [ ] Constraint tests
- [ ] Error case tests
- [ ] Solver unit tests
- [ ] Integration tests với full compilation pipeline

---

## Implementation Order

```
1. Step 10: Function call checking (depends on: resolver, solver)
2. Step 11: Return checking (depends on: resolver, provenance)
3. Step 12: Diagnostics (depends on: all above)
4. Step 13: MVIR boundary check (depends on: all above)
5. Step 14: Tests (depends on: all above)
```

## Estimated Effort

| Step | Complexity | Estimated Time |
|------|------------|----------------|
| 10. Function call checking | Medium | 2-3 hours |
| 11. Return checking | Medium | 2-3 hours |
| 12. Diagnostics | Low | 1-2 hours |
| 13. MVIR boundary check | Low | 1 hour |
| 14. Tests | Medium | 3-4 hours |
| **Total** | | **9-13 hours** |

## Risks and Mitigations

### Risk 1: Integration với existing borrow checker
- **Concern**: Lifetime system có thể conflict với existing borrow checker logic
- **Mitigation**: Contract specify rõ ràng boundary; MVIR boundary check ensures separation

### Risk 2: Elision interaction
- **Concern**: Khi elision được implement sau, có thể conflict với explicit lifetimes
- **Mitigation**: Contract exclude elision; separate phase cho elision

### Risk 3: Test coverage gaps
- **Concern**: Edge cases có thể miss
- **Mitigation**: Focus on contract-specified test matrix; add fuzz tests

---

## Sign-off Checklist

- [ ] All lifetime tests pass
- [ ] No compiler crashes on lifetime-related code
- [ ] Error messages are helpful and accurate
- [ ] MVIR boundary is enforced
- [ ] Documentation updated
