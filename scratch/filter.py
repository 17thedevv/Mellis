import sys
for line in open('tests/e2e/test_vec_out.txt'):
    l = line.lower()
    if 'error' in l or 'redecl' in l or 'resolv' in l:
        print(line, end='')
