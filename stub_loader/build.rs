use std::path::Path;

fn main() {
    println!("cargo:warning=build.rs is running!");

    if Path::new("gui.txt").exists() {
        println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    } else {
        println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
    }

    println!("cargo:rerun-if-changed=configs/gui.txt");
    println!("cargo:rerun-if-changed=configs/icon.ico");

    let mut res = winres::WindowsResource::new();

    if Path::new("icon.ico").exists() {
        res.set_icon("icon.ico");
    }

    res.compile().expect("Failed to compile resources");
}
