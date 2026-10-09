#![deny(unsafe_op_in_unsafe_fn)]

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version") | Some("-V") => println!("local-stt {}", env!("CARGO_PKG_VERSION")),
        _ => println!("local-stt {}: app wiring lands in Task 8", env!("CARGO_PKG_VERSION")),
    }
}
