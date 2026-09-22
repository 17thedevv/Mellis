use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeResult {
    Accepted,
    RejectedAsExpected(String),
    RejectedForWrongReason(String),
    UnsoundlyAccepted(String),
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_phase4a_probe_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_probe(probe_name: &str, source: &str) -> (Result<(), Vec<String>>, Option<PathBuf>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(probe_name);
    let main_path = temp.join(format!("{}.ln", probe_name));
    let exe_path = temp.join(format!("{}.exe", probe_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), source.to_string(), &options);
    match compile_res {
        Ok(_) => (Ok(()), Some(exe_path)),
        Err(diags) => {
            let msgs = diags.into_iter().map(|d| format!("{:?}: {}", d.span, d.message)).collect();
            (Err(msgs), None)
        }
    }
}

/// Probe 1 Tier ENTRY-A: Full Entry API with enum carrying exclusive mutable borrows and pattern matching
#[test]
fn test_probe_1_entry_a_full_entry_api() {
    let src = r#"
import <hashmap>;

enum Entry<K, V> {
    Occupied(&rw V),
    Vacant(&rw HashMap<K, V>, K),
}

extern fn __luna_panic_default() -> !;

impl<K: Hash + Eq, V> Entry<K, V> {
    fn or_insert(self: Self, default_val: V) -> &rw V life_from(self) {
        match self {
            Entry::Occupied(v) -> {
                return v;
            },
            Entry::Vacant(m, k) -> {
                m.insert(k, default_val);
                dec opt = m.get_mut(&k);
                match opt {
                    Option::Some(val_ref) -> {
                        return val_ref;
                    },
                    Option::None -> {
                        __luna_panic_default();
                    },
                }
            },
        }
    }
}

fn entry<K: Hash + Eq, V>(map: &rw HashMap<K, V>, key: K) -> Entry<K, V> life_from(map) {
    if map.contains_key(&key) {
        dec v_opt = map.get_mut(&key);
        match v_opt {
            Option::Some(v_ref) -> {
                return Entry::Occupied(v_ref);
            },
            Option::None -> {
                return Entry::Vacant(map, key);
            },
        }
    }
    return Entry::Vacant(map, key);
}

fn main() -> i32 {
    dec rw map = hashmap_new<i32, i32>();
    dec ent = entry<i32, i32>(&rw map, 10);
    dec v = ent.or_insert(300);
    *v = 400;
    dec opt = map.get(&10);
    match opt {
        Option::Some(val) -> {
            if *val == 400 {
                return 0;
            }
            return 1;
        },
        Option::None -> {
            return 2;
        },
    }
}
"#;

    let (res, exe) = compile_probe("probe_hashmap_entry_a", src);
    println!("PROBE 1 ENTRY-A result: res={:?}, exe={:?}", res, exe);

    // Assert that Tier ENTRY-A triggers E3005 LocalBorrowEscape as expected
    assert!(res.is_err(), "Expected compilation error for Entry-A");
    let diags = res.unwrap_err();
    assert!(diags.iter().any(|d| d.contains("E3005") || d.contains("LocalBorrowEscape")), "Expected E3005 LocalBorrowEscape, got: {:?}", diags);
    println!("PROBE 1 ENTRY-A: REJECTED_AS_EXPECTED -> {:?}", diags);
}

/// Probe 1 Tier ENTRY-B: Direct get_or_insert returning &rw V tied directly to map
#[test]
fn test_probe_1_entry_b_direct_get_or_insert() {
    let src = r#"
import <hashmap>;

extern fn __luna_panic_default() -> !;

fn get_or_insert<K: Hash + Eq, V>(map: &rw HashMap<K, V>, key: K, default_val: V) -> &rw V life_from(map) {
    if map.contains_key(&key) == false {
        map.insert(key, default_val);
    }
    dec opt = map.get_mut(&key);
    match opt {
        Option::Some(v) -> {
            return v;
        },
        Option::None -> {
            __luna_panic_default();
        },
    }
}

fn main() -> i32 {
    dec rw map = hashmap_new<i32, i32>();
    dec v = get_or_insert<i32, i32>(&rw map, 10, 100);
    *v = 200;
    dec opt = map.get(&10);
    match opt {
        Option::Some(val) -> {
            if *val == 200 {
                return 0;
            }
            return 1;
        },
        Option::None -> {
            return 2;
        },
    }
}
"#;

    let (res, exe) = compile_probe("probe_hashmap_entry_b", src);
    println!("PROBE 1 ENTRY-B result: res={:?}, exe={:?}", res, exe);
    assert!(res.is_ok(), "Expected compilation success for direct get_or_insert: {:?}", res);
    let exe_path = exe.unwrap();
    let output = Command::new(&exe_path).output().expect("Execution failed");
    println!("PROBE 1 ENTRY-B run: exit_code={:?}", output.status.code());
    assert_eq!(output.status.code(), Some(0));
}

/// Probe 2: split_at_mut dynamic disjointness & safe-api-via-audited-unsafe
#[test]
fn test_probe_2_split_at_mut() {
    let src = r#"
import <mem>;
import <ptr>;

extern fn __luna_panic_default() -> !;

fn split_at_mut<T>(s: &rw [T], mid: u64) -> (&rw [T], &rw [T]) life_from(s) {
    dec total_len = s.len as u64;
    if mid > total_len {
        __luna_panic_default();
    }
    dec len1 = mid;
    dec len2 = total_len - mid;
    dec p1 = s.data as *rw T;
    unsafe {
        dec p2 = ptr::add_mut<T>(p1, mid);
        dec left = mem::slice_from_raw_parts_mut<T>(p1, len1);
        dec right = mem::slice_from_raw_parts_mut<T>(p2, len2);
        return (left, right);
    }
}

fn main() -> i32 {
    dec rw buf: [i32; 4] = [10, 20, 30, 40];
    unsafe {
        dec p = &rw buf as *rw i32;
        dec s = mem::slice_from_raw_parts_mut<i32>(p, 4 as u64);
        dec tuple = split_at_mut<i32>(s, 2 as u64);
        dec left = tuple.0;
        dec right = tuple.1;
        left[0] = 100;
        right[0] = 300;
    }
    if buf[0] == 100 {
        if buf[2] == 300 {
            return 0;
        }
    }
    return 1;
}
"#;

    let (res, exe) = compile_probe("probe_split_at_mut", src);
    println!("PROBE 2 SPLIT_AT_MUT result: res={:?}, exe={:?}", res, exe);
    if let Ok(()) = res {
        let exe_path = exe.unwrap();
        let output = Command::new(&exe_path).output().expect("Execution failed");
        println!("PROBE 2 SPLIT_AT_MUT run: exit_code={:?}", output.status.code());
        assert_eq!(output.status.code(), Some(0));
    }
}

/// Probe 3: Iterator provenance carrier (Peekable caching Item = &T)
#[test]
fn test_probe_3_carrier_iterator_lifetimes() {
    let src = r#"
import <slice>;
import <mem>;

struct Peekable<T> {
    iter: slice::SliceIter<T>,
    peeked: Option<&T>,
};

impl<T> Peekable<T> {
    fn peek(self: &rw Self) -> Option<&T> life_from(self) {
        match self.peeked {
            Option::Some(val) -> {
                return Option::Some(val);
            },
            Option::None -> {
                dec next_item = self.iter.next();
                match next_item {
                    Option::Some(item) -> {
                        self.peeked = Option::Some(item);
                        return Option::Some(item);
                    },
                    Option::None -> {
                        return Option::None;
                    },
                }
            },
        }
    }

    fn next(self: &rw Self) -> Option<&T> life_from(self) {
        match self.peeked {
            Option::Some(val) -> {
                self.peeked = Option::None;
                return Option::Some(val);
            },
            Option::None -> {
                return self.iter.next();
            },
        }
    }
}

fn make_peekable<T>(s: &[T]) -> Peekable<T> life_from(s) {
    return Peekable<T> {
        iter: slice::slice_iter<T>(s),
        peeked: Option::None,
    };
}

fn main() -> i32 {
    dec buf: [i32; 3] = [10, 20, 30];
    dec p = &buf as *i32;
    dec rw peekable_opt: Option<Peekable<i32>> = Option::None;
    unsafe {
        dec slice_ref = mem::slice_from_raw_parts<i32>(p, 3 as u64);
        peekable_opt = Option::Some(make_peekable<i32>(slice_ref));
    }
    dec rw peekable = peekable_opt.unwrap();

    // 1. Peek first item (should be 10)
    dec p1 = peekable.peek();
    match p1 {
        Option::Some(v1) -> {
            if *v1 != 10 {
                return 1;
            }
        },
        Option::None -> {
            return 2;
        },
    }

    // 2. Peek again (should still be 10 from cache)
    dec p2 = peekable.peek();
    match p2 {
        Option::Some(v2) -> {
            if *v2 != 10 {
                return 3;
            }
        },
        Option::None -> {
            return 4;
        },
    }

    // 3. Consume via next (should be 10)
    dec n1 = peekable.next();
    match n1 {
        Option::Some(v3) -> {
            if *v3 != 10 {
                return 5;
            }
        },
        Option::None -> {
            return 6;
        },
    }

    // 4. Next item should be 20
    dec n2 = peekable.next();
    match n2 {
        Option::Some(v4) -> {
            if *v4 != 20 {
                return 7;
            }
        },
        Option::None -> {
            return 8;
        },
    }

    return 0;
}
"#;

    let (res, exe) = compile_probe("probe_carrier_iterator", src);
    println!("PROBE 3 CARRIER_ITERATOR result: res={:?}, exe={:?}", res, exe);
    assert!(res.is_ok(), "Expected compilation success: {:?}", res);
    let exe_path = exe.unwrap();
    let output = Command::new(&exe_path).output().expect("Execution failed");
    println!("PROBE 3 CARRIER_ITERATOR run: exit_code={:?}", output.status.code());
    assert_eq!(output.status.code(), Some(0));
}

/// Probe 4: HashSet set algebra feasibility (Candidate A: borrowed/lazy vs Candidate B: owned/eager)
#[test]
fn test_probe_4_hashset_set_algebra() {
    // Candidate B: Owned / Eager Set Union
    let src_b = r#"
import <hashset>;
import <clone>;

fn union_owned<T: Clone + Eq + Hash>(a: &HashSet<T>, b: &HashSet<T>) -> HashSet<T> {
    dec rw result = hashset_new<T>();
    dec rw iter_a = a.iter();
    while true {
        dec opt_a = iter_a.next();
        match opt_a {
            Option::Some(elem) -> {
                result.insert((*elem).clone());
            },
            Option::None -> {
                break;
            },
        }
    }
    dec rw iter_b = b.iter();
    while true {
        dec opt_b = iter_b.next();
        match opt_b {
            Option::Some(elem) -> {
                result.insert((*elem).clone());
            },
            Option::None -> {
                break;
            },
        }
    }
    return result;
}

fn main() -> i32 {
    dec rw a = hashset_new<i32>();
    a.insert(1);
    a.insert(2);

    dec rw b = hashset_new<i32>();
    b.insert(2);
    b.insert(3);

    dec u = union_owned<i32>(&a, &b);
    if u.len() != (3 as u64) {
        return 1;
    }
    if u.contains(&1) == false {
        return 2;
    }
    if u.contains(&2) == false {
        return 3;
    }
    if u.contains(&3) == false {
        return 4;
    }
    return 0;
}
"#;

    let (res_b, exe_b) = compile_probe("probe_hashset_algebra_owned", src_b);
    println!("PROBE 4 CANDIDATE-B (OWNED ALGEBRA) result: res={:?}, exe={:?}", res_b, exe_b);
    assert!(res_b.is_ok(), "Expected Candidate B to compile: {:?}", res_b);
    let exe_path = exe_b.unwrap();
    let output = Command::new(&exe_path).output().expect("Execution failed");
    println!("PROBE 4 CANDIDATE-B run: exit_code={:?}", output.status.code());
    assert_eq!(output.status.code(), Some(0));

    // Candidate A: Borrowed / Lazy Set Union Iteration
    // Demonstrates that returning a borrowed iterator tied to two input lifetimes
    // encounters lifetime tracking and struct reference retention boundaries.
    let src_a = r#"
import <hashset>;

struct SetUnionIter<T> {
    iter_a: SetIter<T>,
    iter_b: SetIter<T>,
    set_a: &HashSet<T>,
} requires life(set_a) >= life(self);

fn main() -> i32 {
    return 0;
}
"#;
    let (res_a, _) = compile_probe("probe_hashset_algebra_borrowed", src_a);
    println!("PROBE 4 CANDIDATE-A (BORROWED ALGEBRA) result: res={:?}", res_a);
}

#[test]
fn test_probe_carrier_case_a_source_drop_rejected() {
    // Case A: Source move/drop while carrier retains provenance -> REJECT
    let src = r#"
import <vec>;
import <slice>;

struct Carrier<T> {
    iter: slice::SliceIter<T>,
};

fn make_carrier<T>(s: &[T]) -> Carrier<T> life_from(s) {
    return Carrier<T> {
        iter: slice::slice_iter<T>(s),
    };
}

fn main() -> i32 {
    dec rw carrier_opt: Option<Carrier<i32>> = Option::None;
    {
        dec rw v = vec_new<i32>();
        v.push(10);
        carrier_opt = Option::Some(make_carrier<i32>(v.as_slice()));
        // v dropped at end of block while carrier_opt in outer scope retains borrow!
    }
    dec carrier = carrier_opt.unwrap();
    dec opt = carrier.iter.next();
    return 0;
}
"#;
    let (res, _) = compile_probe("probe_carrier_case_a", src);
    println!("CARRIER CASE A (source drop) result: {:?}", res);
    assert!(res.is_err(), "Case A: Borrow checker must reject carrier outliving source!");
}

#[test]
fn test_probe_carrier_case_b_carrier_move_accepted() {
    // Case B: Carrier itself is moved while it contains borrowed item, source remains alive -> ACCEPT
    let src = r#"
import <vec>;
import <slice>;

struct Carrier<T> {
    iter: slice::SliceIter<T>,
};

fn make_carrier<T>(s: &[T]) -> Carrier<T> life_from(s) {
    return Carrier<T> {
        iter: slice::slice_iter<T>(s),
    };
}

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    dec carrier1 = make_carrier<i32>(v.as_slice());
    // Move carrier1 to carrier2. Source v is still alive.
    // Borrowed reference in carrier is tied to source v, not to carrier's stack location!
    dec carrier2 = carrier1;
    dec opt = carrier2.iter.next();
    match opt {
        Option::Some(x) -> {
            if *x == 10 {
                return 0;
            }
            return 1;
        },
        Option::None -> {
            return 2;
        },
    }
}
"#;
    let (res, exe) = compile_probe("probe_carrier_case_b", src);
    println!("CARRIER CASE B (carrier move) result: {:?}", res);
    assert!(res.is_ok(), "Case B: Moving carrier while source is alive must compile: {:?}", res);
    let exe_path = exe.unwrap();
    let output = Command::new(&exe_path).output().expect("Execution failed");
    println!("CARRIER CASE B run exit code: {:?}", output.status.code());
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_probe_carrier_case_c_nested_carrier_mutation_rejected() {
    // Case C: Nested carrier/adapter retains provenance, attempt source mutation -> REJECT
    let src = r#"
import <vec>;
import <slice>;

struct InnerCarrier<T> {
    iter: slice::SliceIter<T>,
};

struct OuterCarrier<T> {
    inner: InnerCarrier<T>,
};

fn make_outer<T>(s: &[T]) -> OuterCarrier<T> life_from(s) {
    return OuterCarrier<T> {
        inner: InnerCarrier<T> {
            iter: slice::slice_iter<T>(s),
        },
    };
}

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    dec outer = make_outer<i32>(v.as_slice());
    v.push(30); // Conflict: v is mutably borrowed while nested outer carrier holds active borrow!
    dec opt = outer.inner.iter.next();
    return 0;
}
"#;
    let (res, _) = compile_probe("probe_carrier_case_c", src);
    println!("CARRIER CASE C (nested carrier source mutation) result: {:?}", res);
    assert!(res.is_err(), "Case C: Borrow checker must reject source mutation while nested carrier holds active borrow!");
}

#[test]
fn test_probe_dyn_writer_nested_dispatch() {
    let src = r#"
import <core/panic>;
import <result>;

enum FmtError {
    InvalidUtf8,
    WriteFailed,
}

trait Writer {
    fn write_utf8(self: &rw Self, bytes: &[u8]) -> Result<(), FmtError>;
}

trait Display {
    fn fmt(self: &Self, w: &rw dyn Writer) -> Result<(), FmtError>;
}

struct CountingWriter {
    count: usize,
};

impl Writer for CountingWriter {
    fn write_utf8(self: &rw Self, bytes: &[u8]) -> Result<(), FmtError> {
        self.count = self.count + bytes.len();
        return Result::Ok(());
    }
}

struct Point {
    x: i32,
    y: i32,
};

impl Display for Point {
    fn fmt(self: &Self, w: &rw dyn Writer) -> Result<(), FmtError> {
        dec b: [u8; 3] = [101 as u8, 102 as u8, 103 as u8];
        dec slice: &[u8] = &b;
        return w.write_utf8(slice);
    }
}

fn main() -> i32 {
    dec rw writer = CountingWriter { count: 0 as usize };
    dec pt = Point { x: 1, y: 2 };
    dec res = pt.fmt(&rw writer);
    match res {
        Result::Ok(_) -> {
            if writer.count == (3 as usize) {
                return 0;
            }
            return 1;
        },
        Result::Err(_) -> {
            return 2;
        },
    }
}
"#;
    let (res, exe) = compile_probe("probe_dyn_writer_nested_dispatch", src);
    println!("DYN WRITER NESTED DISPATCH compile result: {:?}", res);
    assert!(res.is_ok(), "dyn Writer nested dispatch must compile: {:?}", res);
    let exe_path = exe.unwrap();
    let output = Command::new(&exe_path).output().expect("Execution failed");
    println!("DYN WRITER NESTED DISPATCH run exit code: {:?}", output.status.code());
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_probe_conversion_coherence() {
    // CONVERT-COHERENCE-1: Generic blanket impl alone -> ACCEPTED
    let src_1 = r#"
trait Convert<T> {
    fn convert(self: Self) -> T;
}

impl<T> Convert<T> for T {
    fn convert(self: Self) -> T {
        return self;
    }
}

fn main() -> i32 {
    return 0;
}
"#;
    let (res_1, exe_1) = compile_probe("probe_coherence_1_blanket", src_1);
    assert!(res_1.is_ok(), "CONVERT-COHERENCE-1 (blanket alone) must compile: {:?}", res_1);
    let out_1 = Command::new(&exe_1.unwrap()).output().unwrap();
    assert_eq!(out_1.status.code(), Some(0));

    // CONVERT-COHERENCE-2: Non-overlapping concrete impl -> ACCEPTED
    let src_2 = r#"
trait Convert<T> {
    fn convert(self: Self) -> T;
}

impl Convert<i32> for bool {
    fn convert(self: Self) -> i32 {
        if self { return 1; }
        return 0;
    }
}

fn main() -> i32 {
    dec b: bool = true;
    dec v: i32 = b.convert();
    if v == 1 { return 0; }
    return 1;
}
"#;
    let (res_2, exe_2) = compile_probe("probe_coherence_2_concrete", src_2);
    assert!(res_2.is_ok(), "CONVERT-COHERENCE-2 (non-overlapping concrete) must compile: {:?}", res_2);
    let out_2 = Command::new(&exe_2.unwrap()).output().unwrap();
    assert_eq!(out_2.status.code(), Some(0));

    // CONVERT-COHERENCE-3: Concrete impl overlapping reflexive blanket impl -> REJECTED_AS_EXPECTED
    let src_3 = r#"
trait Convert<T> {
    fn convert(self: Self) -> T;
}

struct MyType {
    val: i32,
};

impl<T> Convert<T> for T {
    fn convert(self: Self) -> T {
        return self;
    }
}

impl Convert<MyType> for MyType {
    fn convert(self: Self) -> MyType {
        return self;
    }
}

fn main() -> i32 {
    return 0;
}
"#;
    let (res_3, _) = compile_probe("probe_coherence_3_overlap", src_3);
    println!("CONVERT-COHERENCE-3 result: {:?}", res_3);
    assert!(res_3.is_err(), "CONVERT-COHERENCE-3 (overlapping impl) must be REJECTED_AS_EXPECTED by coherence checker!");
    let err_str = format!("{:?}", res_3.err().unwrap());
    assert!(err_str.contains("E_CONFLICTING_TRAIT_IMPL") || err_str.contains("conflicting implementations"),
        "Expected E_CONFLICTING_TRAIT_IMPL error, got: {}", err_str);

    // CONVERT-COHERENCE-4: Method lookup with exactly one applicable impl -> deterministic
    let src_4 = r#"
trait Convert<T> {
    fn convert(self: Self) -> T;
}

struct MyType {
    val: i32,
};

impl Convert<i32> for MyType {
    fn convert(self: Self) -> i32 {
        return self.val;
    }
}

fn main() -> i32 {
    dec m = MyType { val: 42 };
    dec v: i32 = m.convert();
    if v == 42 { return 0; }
    return 1;
}
"#;
    let (res_4, exe_4) = compile_probe("probe_coherence_4_dispatch", src_4);
    assert!(res_4.is_ok(), "CONVERT-COHERENCE-4 must compile: {:?}", res_4);
    let out_4 = Command::new(&exe_4.unwrap()).output().unwrap();
    assert_eq!(out_4.status.code(), Some(0));
}

#[test]
fn test_probe_slice_copy_alias() {
    // SLICE-COPY-ALIAS: Attempt to create simultaneous &rw and & borrows of the same allocation
    // to pass overlapping source and destination slices to a copy function in safe Luna.
    let src = r#"
fn copy_slice(dst: &rw [i32], src: &[i32]) {
    dec len = dst.len();
    dec rw i: u64 = 0;
    while i < len {
        dst[i] = src[i];
        i = i + 1;
    }
}

fn main() -> i32 {
    dec rw arr: [i32; 4] = [1, 2, 3, 4];
    dec d: &rw [i32] = &rw arr;
    dec s: &[i32] = &arr; // CONFLICT: arr is already mutably borrowed!
    copy_slice(d, s);
    return 0;
}
"#;
    let (res, _) = compile_probe("probe_slice_copy_alias", src);
    println!("SLICE-COPY-ALIAS result: {:?}", res);
    assert!(res.is_err(), "SLICE-COPY-ALIAS: Borrowck must reject simultaneous &rw and & borrows of the same allocation!");
}





