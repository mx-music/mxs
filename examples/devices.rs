//! Lists the output and input devices with their channels, sample rates and buffer sizes.
//!
//! cargo run --release --example devices

fn main() {
    for device in mxs::devices().unwrap_or_else(|e| panic!("{e}")) {
        println!("{device:?}");
    }
}
