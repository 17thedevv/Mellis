use luna_driver::CompilerOptions;
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_phase_d_utilities_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn compile_and_run(test_name: &str, source: &str) -> (i32, String, String) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    let exe_path = temp.join(format!("{}.exe", test_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = luna_driver::compile_and_render(main_path.to_str().unwrap(), source.to_string(), &options);
    if let Err(rendered) = &compile_res {
        eprintln!("RENDERED DIAGNOSTIC:\n{}", rendered);
    }
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// 1. Slice algorithms: slice_contains, slice_position, slice_binary_search, slice_reverse, slice_sort
#[test]
fn test_slice_algorithms_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <cmp>;
import <result>;

fn is_target(val: &i32) -> bool {
    return *val == 42;
}

fn is_missing(val: &i32) -> bool {
    return *val == 999;
}

fn main() -> i32 {
    dec rw arr: [i32; 5] = [10, 20, 42, 55, 100];

    // 1. slice_contains
    dec target = 42;
    dec missing = 99;
    if slice_contains<i32>(&arr, &target) == false { return 1; }
    if slice_contains<i32>(&arr, &missing) == true { return 2; }

    // 2. slice_position
    dec pos1 = slice_position<i32>(&arr, is_target);
    match pos1 {
        Option::Some(idx) -> {
            if idx != (2 as u64) { return 3; }
        },
        Option::None -> { return 4; },
    }
    dec pos2 = slice_position<i32>(&arr, is_missing);
    if pos2.is_some() == true { return 5; }

    // 3. slice_binary_search on sorted array (found)
    dec bs_res = slice_binary_search<i32>(&arr, &target);
    match bs_res {
        Result::Ok(idx) -> {
            if idx != (2 as u64) { return 6; }
        },
        Result::Err(e) -> { return 7; },
    }

    // 4. slice_binary_search for missing element before all
    dec missing_low = 5;
    dec bs_miss_low = slice_binary_search<i32>(&arr, &missing_low);
    match bs_miss_low {
        Result::Ok(idx) -> { return 8; },
        Result::Err(idx) -> {
            if idx != (0 as u64) { return 9; }
        },
    }

    // 5. slice_binary_search for missing element in middle
    dec missing_mid = 30;
    dec bs_miss_mid = slice_binary_search<i32>(&arr, &missing_mid);
    match bs_miss_mid {
        Result::Ok(idx) -> { return 10; },
        Result::Err(idx) -> {
            if idx != (2 as u64) { return 11; }
        },
    }

    // 6. slice_binary_search for missing element after all
    dec missing_high = 200;
    dec bs_miss_high = slice_binary_search<i32>(&arr, &missing_high);
    match bs_miss_high {
        Result::Ok(idx) -> { return 12; },
        Result::Err(idx) -> {
            if idx != (5 as u64) { return 13; }
        },
    }

    // 7. slice_reverse
    slice_reverse<i32>(&rw arr);
    // arr is now [100, 55, 42, 20, 10]
    if arr[0] != 100 { return 14; }
    if arr[1] != 55 { return 15; }
    if arr[2] != 42 { return 16; }
    if arr[3] != 20 { return 17; }
    if arr[4] != 10 { return 18; }

    // 8. slice_sort
    // sort [100, 55, 42, 20, 10] back to [10, 20, 42, 55, 100]
    slice_sort<i32>(&rw arr);
    if arr[0] != 10 { return 19; }
    if arr[1] != 20 { return 20; }
    if arr[2] != 42 { return 21; }
    if arr[3] != 55 { return 22; }
    if arr[4] != 100 { return 23; }

    // Edge cases: reverse and sort on 1 element slice
    dec rw single: [i32; 1] = [77];
    slice_reverse<i32>(&rw single);
    if single[0] != 77 { return 24; }
    slice_sort<i32>(&rw single);
    if single[0] != 77 { return 25; }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_slice_algorithms_e2e", src);
    assert_eq!(code, 0, "test_slice_algorithms_e2e failed with code {}, stderr: {}", code, stderr);
}

/// 2. Range & RangeInclusive Iterators
#[test]
fn test_range_iterators_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <iter_adapters>;
import <iter_collect>;

fn main() -> i32 {
    // 1. Half-open range: range(0, 5) -> 0, 1, 2, 3, 4
    dec rw r = range(0, 5);
    dec rw sum = 0;
    dec rw count = 0;
    while true {
        dec item = r.next();
        match item {
            Option::Some(val) -> {
                sum = sum + val;
                count = count + 1;
            },
            Option::None -> {
                break;
            },
        }
    }
    if count != 5 { return 1; }
    if sum != 10 { return 2; }

    // Subsequent calls to exhausted range return None
    dec exhausted = r.next();
    if exhausted.is_some() == true { return 3; }

    // 2. Empty range: range(5, 5) and range(5, 2)
    dec rw empty_r = range(5, 5);
    if empty_r.next().is_some() == true { return 4; }

    dec rw inverted_r = range(5, 2);
    if inverted_r.next().is_some() == true { return 5; }

    // 3. Inclusive range: range_inclusive(1, 5) -> 1, 2, 3, 4, 5
    dec rw ri = range_inclusive(1, 5);
    dec rw sum_i = 0;
    dec rw count_i = 0;
    while true {
        dec item = ri.next();
        match item {
            Option::Some(val) -> {
                sum_i = sum_i + val;
                count_i = count_i + 1;
            },
            Option::None -> {
                break;
            },
        }
    }
    if count_i != 5 { return 6; }
    if sum_i != 15 { return 7; }

    // 4. Collect range into Vec<i32> via iter_collect_vec
    dec collected_vec = iter_collect_vec<Range, i32>(range(10, 14));
    if collected_vec.len() != (4 as u64) { return 8; }
    dec s = collected_vec.as_slice();
    if s[0] != 10 { return 9; }
    if s[1] != 11 { return 10; }
    if s[2] != 12 { return 11; }
    if s[3] != 13 { return 12; }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_range_iterators_e2e", src);
    assert_eq!(code, 0, "test_range_iterators_e2e failed with code {}, stderr: {}", code, stderr);
}

/// 3. Adversarial slice sorting/reversing & negative range iteration
#[test]
fn test_slice_adversarial_and_negative_ranges_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <iter_adapters>;
import <iter_consumers>;

fn add_fold(acc: i32, item: i32) -> i32 {
    return acc + item;
}

fn main() -> i32 {
    // 1. Sort with duplicates and reverse elements
    dec rw dups: [i32; 6] = [9, 1, 4, 1, 9, 2];
    slice_sort<i32>(&rw dups);
    if dups[0] != 1 { return 1; }
    if dups[1] != 1 { return 2; }
    if dups[2] != 2 { return 3; }
    if dups[3] != 4 { return 4; }
    if dups[4] != 9 { return 5; }
    if dups[5] != 9 { return 6; }

    // 2. Even-length reverse
    dec rw ev: [i32; 4] = [10, 20, 30, 40];
    slice_reverse<i32>(&rw ev);
    if ev[0] != 40 { return 7; }
    if ev[1] != 30 { return 8; }
    if ev[2] != 20 { return 9; }
    if ev[3] != 10 { return 10; }

    // 3. Negative ranges
    dec rw neg_r = range(-3, 2); // -3, -2, -1, 0, 1
    dec rw neg_sum = 0;
    dec rw neg_count = 0;
    while true {
        dec item = neg_r.next();
        match item {
            Option::Some(v) -> {
                neg_sum = neg_sum + v;
                neg_count = neg_count + 1;
            },
            Option::None -> {
                break;
            },
        }
    }
    if neg_count != 5 { return 11; }
    if neg_sum != -5 { return 12; }

    // 4. Negative inclusive ranges
    dec rw neg_ri = range_inclusive(-2, 2); // -2, -1, 0, 1, 2
    dec rw neg_sum_i = 0;
    dec rw neg_count_i = 0;
    while true {
        dec item = neg_ri.next();
        match item {
            Option::Some(v) -> {
                neg_sum_i = neg_sum_i + v;
                neg_count_i = neg_count_i + 1;
            },
            Option::None -> {
                break;
            },
        }
    }
    if neg_count_i != 5 { return 13; }
    if neg_sum_i != 0 { return 14; }

    // 5. Iterator fold consumer integration
    dec r_fold = range(1, 6); // 1, 2, 3, 4, 5
    dec folded_sum = iter_fold<Range, i32, i32>(r_fold, 0, add_fold);
    if folded_sum != 15 { return 15; }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_slice_adversarial_and_negative_ranges_e2e", src);
    assert_eq!(code, 0, "test_slice_adversarial_and_negative_ranges_e2e failed with code {}, stderr: {}", code, stderr);
}
