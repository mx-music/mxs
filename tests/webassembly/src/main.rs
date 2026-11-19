//! Prints the phrase's levels rendered natively, one to a line, for `tests/webassembly.rs` to compare with WebAssembly's.

fn main() {
    for level in mxs_webassembly_check::levels() {
        println!("{level}");
    }
}
