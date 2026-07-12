fn main() {
    // Expose the build target as an env var so binaries can embed it in --version output.
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=LX_TARGET={target}");
}
