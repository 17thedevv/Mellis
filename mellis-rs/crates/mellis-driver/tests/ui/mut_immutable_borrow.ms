fn main() {
    dec x = 5;
    dec ptr = &rw x; //~ ERROR Cannot mutate immutable variable
}
