use std::env::*;
use std::fs::*;
use std::path::*;
use std::process::*;

/// Recursively invokes `cargo` to build the given mod.
fn build_mod(name: &str, out_path: &Path, output: &mut String) {
    println!("cargo:rerun-if-changed={name}");

    let result = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(name)
        .arg("--release")
        .arg("--target")
        .arg("wasm32-wasip1")
        .arg("--target-dir")
        .arg(out_path)
        .spawn()
        .expect("Failed to start mod build.")
        .wait()
        .expect("Failed to build mod.");

    if result.success() {
        let mut path_buf = PathBuf::from(out_path);

        path_buf.push(format!("wasm32-wasip1/release/{name}.wasm"));
        assert!(path_buf.exists(), "Mod not found at path: {path_buf:?}");

        let wasm = read(&path_buf).expect("Could not read WASM output.");
        let _ = std::fmt::Write::write_fmt(output, format_args!(
            "/// The WASM binary for the `{name}` mod.\n\
            pub const {}: &[u8] = b\"{};\";",
            name.to_uppercase(),
            to_byte_string_literal(&wasm)
        ));
    }
    else {
        panic!("Failed to generate {name} mod");
    }
}

/// Converts a sequence of bytes to a string literal.
fn to_byte_string_literal(bytes: &[u8]) -> String {
    let mut lit = String::new();
    for &byte in bytes {
        if 40 <= byte && byte <= 126 && ![39, 92, 10, 13].contains(&byte) {
            lit.push(std::char::from_u32(byte as u32).unwrap());
        } else {
            let _ = std::fmt::Write::write_fmt(&mut lit, format_args!("\\x{byte:02X}"));
        }
    }
    lit
}

/// Builds all WASM plugins and embeds them for consumption as byte arrays.
fn main() {
    let mut result = String::default();
    
    let out_dir = var("OUT_DIR").expect("Could not get output directory.");
    let out_path = Path::new(&out_dir);

    for mod_name in ["player_controller"] {
        build_mod(mod_name, out_path, &mut result);
    }

    write(
        out_path.join("standard_plugins.rs"),
        result,
    )
    .expect("Could not write WASM mod Rust file.");
}