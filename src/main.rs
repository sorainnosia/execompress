use clap::Parser;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{fs, io::Write, path::PathBuf};
use std::sync::{Arc, Mutex};
use std::process::Command;
use xz2::write::XzEncoder;
use zstd::stream::Encoder;
use brotli::CompressorWriter;
use std::fs::{write, File};
use walkdir::WalkDir;
mod icoextractor;
mod stub;
mod version_extractor;
use crate::icoextractor::IconExtractor;
use crate::version_extractor::extract_version_info;
use rayon::*;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

#[derive(Parser)]
struct Args {
    /// Input executable
    #[arg(short, long)]
    input: PathBuf,

	///	Extra directory containing files and directories to pack/unpack together
	#[arg(short, long)]
    extra_dir: Option<PathBuf>,

    /// Output compressed executable
    #[arg(short, long)]
    output: PathBuf,

    /// Compression level: 1-9 (lzma, default) 1-22 (--zstd), 0-11 (--brotli)
    #[arg(short, long, default_value = "3")]
    level: u32,

    /// Amount of thread used to pack binary and extra directory
    #[arg(short, long, default_value = "4")]
    parallel: usize,

    /// Use zstd instead of lzma
    #[arg(long)]
    zstd: bool,

    /// Use brotli instead of lzma
    #[arg(long)]
    brotli: bool,

    /// When input file is GUI app, suppress command line window
    #[arg(long)]
    gui: bool,

    /// Cleanup temporary files after execution (default: keep temp files)
    #[arg(long)]
    cleanup: bool,

    /// Product name for version info
    #[arg(long)]
    product_name: Option<String>,

    /// Company name for version info
    #[arg(long)]
    company_name: Option<String>,

    /// File description for version info
    #[arg(long)]
    file_description: Option<String>,

    /// Product version (e.g., "1.0.0.0")
    #[arg(long)]
    product_version: Option<String>,

    /// File version (e.g., "1.0.0.0")
    #[arg(long)]
    file_version: Option<String>,

    /// Copyright information
    #[arg(long)]
    copyright: Option<String>,

    /// Require administrator privileges (adds requireAdministrator to manifest)
    #[arg(long)]
    require_admin: bool,

    /// Generate and embed Windows manifest file
    #[arg(long)]
    manifest: bool,
}

fn validate_compression_level(level: u32, brotli: bool, zstd: bool) -> Result<(), String> {
    if brotli {
        if level > 11 {
            return Err(format!(
                "Invalid compression level {} for brotli. Brotli supports levels 0-11.\n\
                 Level 0 = fastest/lowest compression\n\
                 Level 11 = slowest/highest compression (best ratio)",
                level
            ));
        }
    } else if zstd {
        if level < 1 || level > 22 {
            return Err(format!(
                "Invalid compression level {} for zstd. Zstd supports levels 1-22.\n\
                 Level 1 = fastest/lowest compression\n\
                 Level 22 = slowest/highest compression (best ratio)\n\
                 Recommended: 3-19 for balanced performance",
                level
            ));
        }
    } else {
        // LZMA (default)
        if level > 9 {
            return Err(format!(
                "Invalid compression level {} for lzma. LZMA supports levels 0-9.\n\
                 Level 0 = fastest/lowest compression\n\
                 Level 9 = slowest/highest compression (best ratio)\n\
                 Recommended: 3-6 for balanced performance",
                level
            ));
        }
    }
    Ok(())
}

fn extract_icon(input_path: String, output_path: String) -> std::io::Result<()> {
    let mut extractor = IconExtractor::new(input_path)?;
    let ico_data = extractor.extract_largest_icon()?;

    let mut output_file = File::create(output_path)?;
    output_file.write_all(&ico_data)?;

    return Ok(());
}

//fn extract_icon(input: &PathBuf, icon_path: &str) -> std::io::Result<()> {
//    let icons = ico_extract::extract_icons(input)?;
//    if let Some(icon) = icons.first() {
//        std::fs::write(icon_path, &icon.buffer)?;
//    } else {
//        eprintln!("⚠️ No icon found in input file.");
//    }
//    Ok(())
//}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    std::fs::create_dir_all("configs")?;

    // Validate compression level
    if let Err(e) = validate_compression_level(args.level, args.brotli, args.zstd) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Display compression algorithm information
    let algo_name = if args.brotli {
        "Brotli"
    } else if args.zstd {
        "Zstd"
    } else {
        "LZMA"
    };
    println!("Using {} compression (level {})", algo_name, args.level);

    // Extract version info from input executable (for use as defaults)
    let input_version_info = extract_version_info(&args.input);

    let input_data = fs::read(&args.input)?;
    let _ = std::fs::remove_file("stub_loader/icon.ico");
    let x = extract_icon(args.input.display().to_string(), "stub_loader/icon.ico".to_string());
    match x {
        Ok(x) => {},
        Err(x) => { println!("{:?}", x); }
    }

    if args.gui {
        let _ = std::fs::remove_file("stub_loader/gui.txt");
        write("stub_loader/gui.txt", "true")?;
    } else {
        let _ = std::fs::remove_file("stub_loader/gui.txt");
    }

    if args.cleanup {
        let _ = std::fs::remove_file("stub_loader/cleanup.txt");
        write("stub_loader/cleanup.txt", "true")?;
    } else {
        let _ = std::fs::remove_file("stub_loader/cleanup.txt");
    }

    // Write version info to files for build.rs to read
    // Use provided value, or fall back to extracted value from input exe
    if let Some(product_name) = &args.product_name {
        write("stub_loader/product_name.txt", product_name)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref product_name) = info.product_name {
            write("stub_loader/product_name.txt", product_name)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/product_name.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/product_name.txt");
    }

    if let Some(company_name) = &args.company_name {
        write("stub_loader/company_name.txt", company_name)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref company_name) = info.company_name {
            write("stub_loader/company_name.txt", company_name)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/company_name.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/company_name.txt");
    }

    if let Some(file_description) = &args.file_description {
        write("stub_loader/file_description.txt", file_description)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref file_description) = info.file_description {
            write("stub_loader/file_description.txt", file_description)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/file_description.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/file_description.txt");
    }

    if let Some(product_version) = &args.product_version {
        write("stub_loader/product_version.txt", product_version)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref product_version) = info.product_version {
            write("stub_loader/product_version.txt", product_version)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/product_version.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/product_version.txt");
    }

    if let Some(file_version) = &args.file_version {
        write("stub_loader/file_version.txt", file_version)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref file_version) = info.file_version {
            write("stub_loader/file_version.txt", file_version)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/file_version.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/file_version.txt");
    }

    if let Some(copyright) = &args.copyright {
        write("stub_loader/copyright.txt", copyright)?;
    } else if let Some(ref info) = input_version_info {
        if let Some(ref copyright) = info.copyright {
            write("stub_loader/copyright.txt", copyright)?;
        } else {
            let _ = std::fs::remove_file("stub_loader/copyright.txt");
        }
    } else {
        let _ = std::fs::remove_file("stub_loader/copyright.txt");
    }

    // Write the output filename for OriginalFilename field
    let output_filename = args.output.file_name().unwrap().to_string_lossy();
    write("stub_loader/original_filename.txt", output_filename.as_ref())?;

    // Write admin flag
    if args.require_admin {
        let _ = std::fs::remove_file("stub_loader/require_admin.txt");
        write("stub_loader/require_admin.txt", "true")?;
    } else {
        let _ = std::fs::remove_file("stub_loader/require_admin.txt");
    }

    // Write manifest flag
    if args.manifest {
        let _ = std::fs::remove_file("stub_loader/manifest.txt");
        write("stub_loader/manifest.txt", "true")?;
    } else {
        let _ = std::fs::remove_file("stub_loader/manifest.txt");
    }
    let compressed_data = if args.brotli {
        let mut out = vec![];
        let mut encoder = CompressorWriter::new(&mut out, 4096, args.level, 22);
        encoder.write_all(&input_data)?;
        drop(encoder);
        out
    } else if args.zstd {
        let mut out = vec![];
        let mut encoder = Encoder::new(&mut out, args.level as i32)?;
        encoder.write_all(&input_data)?;
        encoder.finish()?;
        out
    } else {
        let mut out = vec![];
        let mut encoder = XzEncoder::new(&mut out, args.level);
        encoder.write_all(&input_data)?;
        encoder.finish()?;
        out
    };
	
	let mut extra_files = Arc::new(Mutex::new(vec![]));
	
	let ef = extra_files.clone();
	if let Some(xtra) = &args.extra_dir {
		if xtra.is_dir() {
			//for entry in WalkDir::new(xtra.clone())
			//	.into_iter()
			
			let pool = ThreadPoolBuilder::new()
                    .num_threads(args.parallel)
                    .build()
                    .unwrap();

			pool.install(|| {
				WalkDir::new(xtra.clone())
					.into_iter()
					.filter_map(|e| e.ok())
					.filter(|e| e.file_type().is_file())
					.par_bridge()
					.for_each(|entry| 
				{
					let path = entry.path();
					let rel_path = path.strip_prefix(&xtra).unwrap().to_string_lossy().replace("\\", "/");
					let data = fs::read(path).unwrap();

					let compressed_data = if args.brotli {
						let mut out = vec![];
						let mut encoder = CompressorWriter::new(&mut out, 4096, args.level, 22);
						encoder.write_all(&data).unwrap();
						drop(encoder);
						out
					} else if args.zstd {
						let mut out = vec![];
						let mut encoder = Encoder::new(&mut out, args.level as i32).unwrap();
						encoder.write_all(&data).unwrap();
						encoder.finish().unwrap();
						out
					} else {
						let mut out = vec![];
						let mut encoder = XzEncoder::new(&mut out, args.level);
						encoder.write_all(&data).unwrap();
						encoder.finish().unwrap();
						out
					};

					{
						let ef2 = ef.clone();
						let list = &mut *ef2.lock().unwrap();
						list.push((rel_path, compressed_data));
					}
				});
			});
		}
	}

    let compression_ratio = (compressed_data.len() as f64 / input_data.len() as f64) * 100.0;
    println!("Original size: {} bytes, Compressed: {} bytes ({:.2}% of original)",
             input_data.len(), compressed_data.len(), compression_ratio);

    // Embed compressed payload as base64
    let payload = base64::engine::general_purpose::STANDARD.encode(&compressed_data);
    let original_filename = args.output.file_name().unwrap().to_string_lossy();
    let xfilenamex = base64::engine::general_purpose::STANDARD.encode(&original_filename.as_bytes());

    // Read embedded stub EXE
    let mut stub = stub::get_stub_exe(args.gui);
    //let mut stub = stub::get_stub_exe();

    // Append marker + base64 payload

    stub.extend_from_slice(b"\n--XFILENAMEX--\n");
    stub.extend_from_slice(xfilenamex.as_bytes());

    // Append cleanup flag
    if args.cleanup {
        stub.extend_from_slice(b"\n--CLEANUP--\n");
    }

    // Append compressed file content
    stub.extend_from_slice(b"\n--FILE-CONTENT--\n");
    let payload_len_line = format!("{}\n", compressed_data.len());
    stub.extend_from_slice(payload_len_line.as_bytes()); // Write the length as a single line
    stub.extend_from_slice(&compressed_data);     

	let ef3 = ef.clone();
	{
		let list = &*ef3.lock().unwrap();
		for (filename, compressed_data) in list {
			let encoded_name = STANDARD.encode(&filename);
			stub.extend_from_slice(b"\n--EXTRA-FILE--\n");
			stub.extend_from_slice(encoded_name.as_bytes());
			stub.extend_from_slice(b"\n");
			let len_line = format!("{}\n", compressed_data.len());
			stub.extend_from_slice(len_line.as_bytes());
			stub.extend_from_slice(&compressed_data);
		}
	}

    fs::write(&args.output, stub)?;
    println!("Compressed executable written to {:?}", args.output);
    Ok(())
}
