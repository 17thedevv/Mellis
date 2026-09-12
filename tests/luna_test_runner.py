import os
import sys
import subprocess
import argparse
import re
from pathlib import Path

# ANSI escape sequence regex
ANSI_ESCAPE = re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')

def strip_ansi(text):
    return ANSI_ESCAPE.sub('', text)

def run_luna(luna_exe, test_file):
    result = subprocess.run(
        [luna_exe, str(test_file)],
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace'
    )
    return {
        'exit_code': result.returncode,
        'stdout': strip_ansi(result.stdout).strip(),
        'stderr': strip_ansi(result.stderr).strip()
    }

def format_snapshot(result):
    return f"=== EXIT CODE ===\n{result['exit_code']}\n=== STDOUT ===\n{result['stdout']}\n=== STDERR ===\n{result['stderr']}\n"

def main():
    parser = argparse.ArgumentParser(description="Luna Snapshot Test Runner")
    parser.add_argument('--update', action='store_true', help='Update the expected snapshots')
    parser.add_argument('--test', action='store_true', help='Run tests and compare against snapshots')
    parser.add_argument('--exe', default=r'..\mellis-rs\target\debug\luna.exe', help='Path to luna executable')
    
    args = parser.parse_args()
    
    if not args.update and not args.test:
        print("Please specify either --update or --test")
        sys.exit(1)

    luna_exe = os.path.abspath(args.exe)
    if not os.path.exists(luna_exe):
        for candidate in [
            r'mellis-rs\target\debug\luna.exe',
            r'build\compiler\Release\luna.exe',
            r'build_release\compiler\Release\luna.exe',
            r'build_release\compiler\Release\mellis.exe',
            r'build\compiler\Release\mellis.exe',
        ]:
            if os.path.exists(os.path.abspath(candidate)):
                luna_exe = os.path.abspath(candidate)
                break
        if not os.path.exists(luna_exe):
            print(f"Error: Could not find luna executable at {luna_exe}")
            sys.exit(1)

    test_dirs = ['valid', 'invalid', 'borrow', 'type', 'closure', 'generics', 'modules', 'codegen']
    base_dir = Path(__file__).parent
    
    test_files = []
    for d in test_dirs:
        dir_path = base_dir / d
        if dir_path.exists():
            test_files.extend(list(dir_path.rglob('*.ln')))
            test_files.extend([f for f in dir_path.rglob('*.ms') if not f.with_suffix('.ln').exists()])
            
    if not test_files:
        print("No test files found in the semantic directories.")
        # Try to find loose .ln files to help with migration
        test_files.extend([f for f in base_dir.rglob('*.ln') if 'scratch' not in str(f) and f.name != 'lib.ln'])
        print(f"Found {len(test_files)} unorganized tests. Running them instead.")

    passed = 0
    failed = []

    for test_file in test_files:
        print(f"Running {test_file.name}...", end=' ')
        result = run_luna(luna_exe, test_file)
        actual_output = format_snapshot(result)
        
        expected_file = test_file.with_suffix('.expected')
        
        if args.update:
            with open(expected_file, 'w', encoding='utf-8') as f:
                f.write(actual_output)
            print("UPDATED")
            passed += 1
        else:
            if not expected_file.exists():
                print(f"FAIL (Missing .expected file)")
                failed.append(test_file)
                continue
                
            with open(expected_file, 'r', encoding='utf-8') as f:
                expected_output = f.read()
                
            if actual_output == expected_output:
                print("PASS")
                passed += 1
            else:
                print("FAIL")
                print("--- EXPECTED ---")
                print(expected_output)
                print("--- ACTUAL ---")
                print(actual_output)
                failed.append(test_file)

    print("\n" + "="*40)
    print(f"Summary: {passed} passed, {len(failed)} failed")
    if failed:
        sys.exit(1)

if __name__ == "__main__":
    main()
