//! Fuzz the Dioxus VirtualDom by driving template, dynamic-node, and dynamic-attr
//! mutations from an iterator-fuzz coverage-guided RNG.
//!
//! Each iteration of `curious()` yields an `RngCore` that samples a fresh op list; the
//! iterator-fuzz search mutates the consumed byte prefix to maximize coverage of the diff
//! machinery. On a divergence, the forked RNG path is handed to `cautious()` for min-path
//! coverage-reducing minimization.
//!
//! Build the harness with LLVM coverage instrumentation:
//!
//! ```sh
//! RUSTFLAGS="-Cinstrument-coverage" \
//!   FUZZ_DISCOVERY=8192 FUZZ_STEPS=256 FUZZ_MINIMIZE=2048 \
//!   cargo run -p dioxus-vdom-fuzz --release
//! ```
#![allow(dead_code, non_snake_case)]

mod harness;
mod model;
mod ops;
mod vdom;

fn main() {
    eprintln!("Run the minimized regression corpus with `cargo test -p dioxus-vdom-fuzz`.");
}
