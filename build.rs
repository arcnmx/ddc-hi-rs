use std::env::var;

fn feature_enabled(name: &str) -> bool {
    var(&format!("CARGO_FEATURE_{}", name.to_uppercase().replace("-", "_"))).is_ok()
}

fn emit_feature(name: &str) {
    println!("cargo:rustc-cfg=feature=\"{}\"", name);
}

fn main() {
    if var("CARGO_CFG_TARGET_OS") == Ok("macos".into()) {
        if feature_enabled("ddc-macos") {
            emit_feature("has-ddc-macos");
        }
    } else if var("CARGO_CFG_UNIX").is_ok() {
        if feature_enabled("ddc-i2c") {
            emit_feature("has-ddc-i2c");
        }
    }

    if var("CARGO_CFG_WINDOWS").is_ok() {
        if feature_enabled("ddc-winapi") {
            emit_feature("has-ddc-winapi");
        }
        if feature_enabled("nvapi") {
            emit_feature("has-nvapi");
        }
    }
}
