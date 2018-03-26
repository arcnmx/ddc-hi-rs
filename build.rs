use std::env::var;

fn feature_enabled(name: &str) -> bool {
    var(&format!("CARGO_FEATURE_{}", name.to_uppercase().replace("-", "_"))).is_ok()
}

fn emit_feature(name: &str) {
    println!("cargo:rustc-cfg=feature=\"{}\"", name);
}

fn main() {
    if cfg!(unix) {
        if feature_enabled("ddc-i2c") {
            emit_feature("has-ddc-i2c");
        }
    }

    if cfg!(windows) {
        if feature_enabled("ddc-winapi") {
            emit_feature("has-ddc-winapi");
        }
        if feature_enabled("nvapi") {
            emit_feature("has-nvapi");
        }
    }
}
