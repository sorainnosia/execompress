use std::path::Path;
use std::fs;

fn read_file_if_exists(path: &str) -> Option<String> {
    if Path::new(path).exists() {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

fn parse_version(version: &str) -> Option<(u16, u16, u16, u16)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 4 {
        let major = parts[0].parse::<u16>().ok()?;
        let minor = parts[1].parse::<u16>().ok()?;
        let patch = parts[2].parse::<u16>().ok()?;
        let build = parts[3].parse::<u16>().ok()?;
        Some((major, minor, patch, build))
    } else {
        None
    }
}

fn generate_manifest(
    product_name: &str,
    file_description: &str,
    product_version: &str,
    require_admin: bool
) -> String {
    let execution_level = if require_admin {
        "requireAdministrator"
    } else {
        "asInvoker"
    };

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    name="{}"
    version="{}"
    type="win32"
  />
  <description>{}</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="{}" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and Windows 11 -->
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
      <!-- Windows 8.1 -->
      <supportedOS Id="{{1f676c76-80e1-4239-95bb-83d0f6d0da78}}"/>
      <!-- Windows 8 -->
      <supportedOS Id="{{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}}"/>
      <!-- Windows 7 -->
      <supportedOS Id="{{35138b9a-5d96-4fbd-8e2d-a2440225f93a}}"/>
    </application>
  </compatibility>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>"#,
        product_name,
        product_version,
        file_description,
        execution_level
    )
}

fn main() {
    println!("cargo:warning=build.rs is running!");

    if Path::new("gui.txt").exists() {
        println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    } else {
        println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
    }

    println!("cargo:rerun-if-changed=configs/gui.txt");
    println!("cargo:rerun-if-changed=configs/icon.ico");
    println!("cargo:rerun-if-changed=product_name.txt");
    println!("cargo:rerun-if-changed=company_name.txt");
    println!("cargo:rerun-if-changed=file_description.txt");
    println!("cargo:rerun-if-changed=product_version.txt");
    println!("cargo:rerun-if-changed=file_version.txt");
    println!("cargo:rerun-if-changed=copyright.txt");
    println!("cargo:rerun-if-changed=original_filename.txt");
    println!("cargo:rerun-if-changed=require_admin.txt");
    println!("cargo:rerun-if-changed=manifest.txt");

    let mut res = winres::WindowsResource::new();

    if Path::new("icon.ico").exists() {
        res.set_icon("icon.ico");
    }

    // Note: Manifest will be generated later in this script and set before compile

    // Read version information or use defaults
    let product_name = read_file_if_exists("product_name.txt")
        .unwrap_or_else(|| "Application".to_string());
    let company_name = read_file_if_exists("company_name.txt")
        .unwrap_or_else(|| "".to_string());
    let file_description = read_file_if_exists("file_description.txt")
        .unwrap_or_else(|| "Application".to_string());
    let product_version = read_file_if_exists("product_version.txt")
        .unwrap_or_else(|| "1.0.0.0".to_string());
    let file_version = read_file_if_exists("file_version.txt")
        .unwrap_or_else(|| "1.0.0.0".to_string());
    let copyright = read_file_if_exists("copyright.txt")
        .unwrap_or_else(|| "".to_string());
    let original_filename = read_file_if_exists("original_filename.txt")
        .unwrap_or_else(|| "application.exe".to_string());
    let require_admin = Path::new("require_admin.txt").exists();
    let create_manifest = Path::new("manifest.txt").exists();

    // Only generate manifest if --manifest flag is specified
    if create_manifest {
        let manifest_content = generate_manifest(
            &product_name,
            &file_description,
            &product_version,
            require_admin
        );
        fs::write("app.manifest", manifest_content).expect("Failed to write manifest");
    } else {
        // Remove any existing manifest file
        let _ = fs::remove_file("app.manifest");
    }

    // Set all version information fields - MUST set all to override Cargo.toml defaults
    let trimmed_product_name = product_name.trim();
    let trimmed_company_name = company_name.trim();
    let trimmed_file_description = file_description.trim();
    let trimmed_product_version = product_version.trim();
    let trimmed_file_version = file_version.trim();
    let trimmed_copyright = copyright.trim();
    let trimmed_filename = original_filename.trim();
    let internal_name = trimmed_filename.trim_end_matches(".exe");

    // Set string info - these will appear in file properties
    res.set("ProductName", trimmed_product_name);
    res.set("CompanyName", trimmed_company_name);
    res.set("FileDescription", trimmed_file_description);
    res.set("ProductVersion", trimmed_product_version);
    res.set("FileVersion", trimmed_file_version);
    res.set("LegalCopyright", trimmed_copyright);
    res.set("InternalName", internal_name);
    res.set("OriginalFilename", trimmed_filename);

    // Also set the language-independent version numbers
    // Parse version string like "1.0.0.0" into individual components
    if let Some((major, minor, patch, build)) = parse_version(trimmed_file_version) {
        res.set_version_info(winres::VersionInfo::FILEVERSION,
            (major as u64) << 48 | (minor as u64) << 32 | (patch as u64) << 16 | (build as u64));
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION,
            (major as u64) << 48 | (minor as u64) << 32 | (patch as u64) << 16 | (build as u64));
    }

    // Only set manifest file if --manifest flag was specified
    if create_manifest && Path::new("app.manifest").exists() {
        res.set_manifest_file("app.manifest");
    }

    res.compile().expect("Failed to compile resources");
}
