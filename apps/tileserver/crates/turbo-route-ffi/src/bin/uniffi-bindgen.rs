//! Generates the Kotlin/Swift binding sources from the compiled cdylib.
//!
//! ```sh
//! cargo build -p turbo-route-ffi
//! cargo run -p turbo-route-ffi --bin uniffi-bindgen -- \
//!   generate --library target/debug/libturbo_route_ffi.so \
//!   --language kotlin --language swift --out-dir target/ffi-bindings
//! ```

fn main() {
    uniffi::uniffi_bindgen_main()
}
