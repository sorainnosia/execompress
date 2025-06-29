use std::process::Command;
use std::path::PathBuf;
use std::fs;

pub fn get_stub_exe(gui: bool) -> Vec<u8> {
    let stub_dir = PathBuf::from("stub_loader");

    // Set build arguments
    let mut args = vec!["build", "--release"];
    if gui {
        args.push("--features");
        args.push("gui");
    }

    // Run cargo build
    let output = Command::new("cargo")
        .args(&args)
        .current_dir(&stub_dir)
        .output()
        .expect("Failed to build stub");

    if !output.status.success() {
        panic!(
            "Stub build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Read the result
    fs::read(stub_dir.join("target/release/stub_loader.exe"))
        .expect("Failed to read built stub_loader.exe")
}