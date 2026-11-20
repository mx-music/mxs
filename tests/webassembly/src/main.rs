//! Prints the phrase's levels rendered natively, one to a line, or with `hash` the hash of its samples, for `tests/webassembly.rs` to compare.
//! Built without the `tables` feature, it first loads the tables the phrase plays from mxs's files.

fn main() {
    if !cfg!(feature = "tables") {
        let figures = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/synths/figures");
        for file in mxs_webassembly_check::TABLES {
            let bytes = std::fs::read(format!("{figures}/{file}")).unwrap();
            mxs::synths::load_table(file, &bytes).unwrap();
        }
    }
    if std::env::args().nth(1).as_deref() == Some("hash") {
        println!("{:016x}", mxs_webassembly_check::hash());
        return;
    }
    for level in mxs_webassembly_check::levels() {
        println!("{level}");
    }
}
