use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn setup_options() -> CompilerOptions {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------
// SEM-GAP-16: Place Overlap and Precision (Phase 2D-A)
// -----------------------------------------------------------------------------

#[test]
fn test_sem_gap_16_disjoint_sibling_fields() {
    let source = r#"
    module test {
        struct Point {
            x: i32,
            y: i32,
        };
        
        export fn main() {
            dec rw p = Point { x: 1, y: 2 };
            // Borrow one field
            dec r = &p.x;
            // Mutate sibling field. This must be allowed.
            p.y = 10;
            
            // Use r to keep it alive during mutation
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_disjoint_sibling_fields", source.to_string(), &options);
    assert!(result.is_ok(), "Sibling field disjointness incorrectly rejected: {result:?}");
}

#[test]
fn test_sem_gap_16_overlapping_same_field() {
    let source = r#"
    module test {
        struct Point {
            x: i32,
            y: i32,
        };
        
        export fn main() {
            dec rw p = Point { x: 1, y: 2 };
            dec r = &p.x;
            // Mutate SAME field. This must be rejected.
            p.x = 10;
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_overlapping_same_field", source.to_string(), &options);
    assert!(result.is_err(), "Same field overlap incorrectly accepted");
}

#[test]
fn test_sem_gap_16_prefix_overlap_aggregate_borrow_subplace_mutate() {
    let source = r#"
    module test {
        struct Point {
            x: i32,
            y: i32,
        };
        
        export fn main() {
            dec rw p = Point { x: 1, y: 2 };
            dec r = &p;
            // Mutate sub-place. This must be rejected because whole aggregate is borrowed.
            p.x = 10;
            dec val = r.x;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_prefix_overlap_aggregate_borrow_subplace_mutate", source.to_string(), &options);
    assert!(result.is_err(), "Prefix overlap incorrectly accepted");
}

#[test]
fn test_sem_gap_16_prefix_overlap_subplace_borrow_aggregate_mutate() {
    let source = r#"
    module test {
        struct Point {
            x: i32,
            y: i32,
        };
        
        export fn main() {
            dec rw p = Point { x: 1, y: 2 };
            dec r = &p.x;
            // Mutate whole aggregate. This must be rejected because sub-place is borrowed.
            p = Point { x: 10, y: 20 };
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_prefix_overlap_subplace_borrow_aggregate_mutate", source.to_string(), &options);
    assert!(result.is_err(), "Prefix overlap (aggregate mutate) incorrectly accepted");
}

#[test]
fn test_sem_gap_16_nested_disjoint_fields() {
    let source = r#"
    module test {
        struct Inner {
            a: i32,
            b: i32,
        };
        struct Outer {
            inner: Inner,
            c: i32,
        };
        
        export fn main() {
            dec rw o = Outer { inner: Inner { a: 1, b: 2 }, c: 3 };
            dec r = &o.inner.a;
            // Mutate sibling at nested level
            o.inner.b = 10;
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_nested_disjoint_fields", source.to_string(), &options);
    assert!(result.is_ok(), "Nested disjoint fields incorrectly rejected: {result:?}");
}

#[test]
fn test_sem_gap_16_nested_overlapping_fields() {
    let source = r#"
    module test {
        struct Inner {
            a: i32,
            b: i32,
        };
        struct Outer {
            inner: Inner,
            c: i32,
        };
        
        export fn main() {
            dec rw o = Outer { inner: Inner { a: 1, b: 2 }, c: 3 };
            dec r = &o.inner.a;
            // Mutate parent of borrowed field
            o.inner = Inner { a: 10, b: 20 };
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_nested_overlapping_fields", source.to_string(), &options);
    assert!(result.is_err(), "Nested overlapping fields incorrectly accepted");
}

#[test]
fn test_sem_gap_16_unknown_alias_conservative_conflict() {
    let source = r#"
    module test {
        struct Point {
            x: int,
            y: int,
        };
        
        fn danger(ref1: &rw Point, ref2: &rw Point) {
            dec r = &ref1.x;
            ref2.y = 10;
            dec val = *r;
        }
    }
    "#;
    let options = setup_options();
    let result = check("test_sem_gap_16_unknown_alias_conservative_conflict", source.to_string(), &options);
    assert!(result.is_err(), "Unknown alias incorrectly assumed disjoint");
}
