use clap::Parser;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{fs, io::Write, path::PathBuf};
use std::process::Command;
use xz2::write::XzEncoder;
use zstd::stream::Encoder;
use std::fs::{write, File};
use walkdir::WalkDir;
mod icoextractor;
mod stub;
use crate::icoextractor::IconExtractor;

#[derive(Parser)]
struct Args {
    /// Input executable
    #[arg(short, long)]
    input: PathBuf,

	///	Extra path to a directory containing files and directories to pack/unpack together
	#[arg(short, long)]
    extra: Option<PathBuf>,

    /// Output compressed executable
    #[arg(short, long)]
    output: PathBuf,

    /// Compression level: 1-9
    #[arg(short, long, default_value = "3")]
    level: u32,

    /// Use zstd instead of lzma
    #[arg(long)]
    zstd: bool,

	/// When input file is GUI app, suppress command line window
    #[arg(long)]
    gui: bool,
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
    let compressed_data = if args.zstd {
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
	
	let mut extra_files = vec![];
	
	
	if let Some(xtra) = &args.extra {
		if xtra.is_dir() {
			for entry in WalkDir::new(xtra.clone())
				.into_iter()
				.filter_map(|e| e.ok())
				.filter(|e| e.file_type().is_file())
			{
				let path = entry.path();
				let rel_path = path.strip_prefix(&xtra).unwrap().to_string_lossy().replace("\\", "/");
				let data = fs::read(path)?;

				let compressed_data = if args.zstd {
					let mut out = vec![];
					let mut encoder = Encoder::new(&mut out, args.level as i32)?;
					encoder.write_all(&data)?;
					encoder.finish()?;
					out
				} else {
					let mut out = vec![];
					let mut encoder = XzEncoder::new(&mut out, args.level);
					encoder.write_all(&data)?;
					encoder.finish()?;
					out
				};

				extra_files.push((rel_path, compressed_data));
			}
		}
	}

    println!("Original size: {}, Compressed: {}", input_data.len(), compressed_data.len());

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
    //stub.extend_from_slice(b"\n--PAYLOAD--\n");
    //stub.extend_from_slice(payload.as_bytes());
    stub.extend_from_slice(b"\n--PAYLOAD--\n");
    let payload_len_line = format!("{}\n", compressed_data.len());
    stub.extend_from_slice(payload_len_line.as_bytes()); // Write the length as a single line
    stub.extend_from_slice(&compressed_data);     

	for (filename, compressed_data) in extra_files {
		let encoded_name = STANDARD.encode(&filename);
		stub.extend_from_slice(b"\n--EXTRA-FILE--\n");
		stub.extend_from_slice(encoded_name.as_bytes());
		stub.extend_from_slice(b"\n");
		let len_line = format!("{}\n", compressed_data.len());
		stub.extend_from_slice(len_line.as_bytes());
		stub.extend_from_slice(&compressed_data);
	}

    fs::write(&args.output, stub)?;
    println!("Compressed executable written to {:?}", args.output);
    Ok(())
}
