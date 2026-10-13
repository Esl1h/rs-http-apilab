// Build script: bake the toolchain version and a reproducible build date
// into the binary (mirroring what v-http-apilab does with V_VERSION).
//
// BUILD_DATE_EPOCH honors SOURCE_DATE_EPOCH so builds are reproducible;
// without it, the current time is used. The epoch is formatted at runtime
// by src/timefmt.rs, so the formatting logic exists only once.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs();
    let epoch = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(now);
    println!("cargo:rustc-env=RSAPILAB_BUILD_DATE_EPOCH={epoch}");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=RSAPILAB_RUSTC_VERSION={version}");
}
