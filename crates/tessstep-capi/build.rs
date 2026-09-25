//! Give the shared ABI library a relocatable loader identity, independent of the
//! Cargo build path. Otherwise native consumers can silently load the build copy.
#![forbid(unsafe_code)]

fn main() {
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            println!("cargo:rustc-link-arg-cdylib=-Wl,-install_name,@rpath/libtessstep_capi.dylib")
        }
        Ok("linux") => {
            println!("cargo:rustc-link-arg-cdylib=-Wl,-soname,libtessstep_capi.so");
        }
        _ => {}
    }
}
