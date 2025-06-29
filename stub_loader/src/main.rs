#![cfg_attr(feature = "gui", windows_subsystem = "windows")]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::fs::File;
use std::io::{BufRead, BufReader, Write, Read};
use std::path::{PathBuf, Path};
use std::process::Command;
use std::env;
use tempfile::NamedTempFile;
use xz2::write::XzEncoder;
use xz2::read::XzDecoder;
use close_file::Closable;
use rand::{distributions::Alphanumeric, Rng};

fn generate_random_string(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn main() {
    let exe_path = std::env::current_exe().unwrap();
    let mut file = File::open(&exe_path).unwrap();
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).unwrap();

    let marker_payload = b"--PAYLOAD--\n";
    let marker_filename = b"--XFILENAMEX--\n";

    let mut payload_start = None;
    let mut filename_start = None;

    for i in 0..buffer.len() {
        if i + marker_payload.len() <= buffer.len() && &buffer[i..i + marker_payload.len()] == marker_payload {
            payload_start = Some(i + marker_payload.len());
        }
        if i + marker_filename.len() <= buffer.len() && &buffer[i..i + marker_filename.len()] == marker_filename {
            filename_start = Some(i + marker_filename.len());
        }
    }

    let mut xfilenamex = "output.exe".to_string();

    if let Some(fstart) = filename_start {
        if let Some(ploc) = buffer[fstart..].iter().position(|&c| c == b'\n') {
            let filename_encoded = &buffer[fstart..fstart + ploc];
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(filename_encoded) {
                xfilenamex = String::from_utf8_lossy(&decoded).to_string();
            }
        }
    }

    let compressed_data = if let Some(pstart) = payload_start {
        if let Some(newline) = buffer[pstart..].iter().position(|&b| b == b'\n') {
            let len_str = String::from_utf8_lossy(&buffer[pstart..pstart + newline]);
            if let Ok(payload_len) = len_str.trim().parse::<usize>() {
                let bin_start = pstart + newline + 1;
                buffer[bin_start..bin_start + payload_len].to_vec()
            } else {
                panic!("Invalid payload length");
            }
        } else {
            panic!("Could not find payload length newline");
        }
    } else {
        panic!("Payload marker not found");
    };
    // let exe_path = std::env::current_exe().unwrap();
    // let file = File::open(&exe_path).unwrap();
    // let reader = BufReader::new(file);

    // let mut base64_data = String::new();
    // let mut base64_data2 = String::new();
    // let mut found = false;
    // let mut found2 = false;
    // let mut xfilenamex = String::from("output.exe");
    // for line in reader.lines().flatten() {
    //     if found {
    //         base64_data.push_str(&line);

    //     } else if line.trim() == "--PAYLOAD--" {
    //         found = true;
    //     }
    // }
    // //reader.close();

    // let file2 = File::open(&exe_path).unwrap();
    // let reader2 = BufReader::new(file2);
    // for line in reader2.lines().flatten() {
    //     if found2 {
    //         base64_data2.push_str(&line);
    //         if let Ok(x) = base64::engine::general_purpose::STANDARD.decode(base64_data2.trim()) {
    //             xfilenamex = String::from_utf8_lossy(&x).to_string();
    //         }
    //     } else if line.trim() == "--XFILENAMEX--" {
    //          found2 = true;
    //     }
    // }
    //reader2.close();

    //println!("App Name : {}", xfilenamex.to_string());

    // if !found {
    //     eprintln!("Payload not found");
    //     return;
    // }

    // let compressed_data = base64::engine::general_purpose::STANDARD
    //     .decode(base64_data.trim())
    //     .expect("Failed to decode base64");

    let decompressed = if let Ok(decompressed) = decompress_lzma(&compressed_data) {
        decompressed
    } else {
        decompress_zstd(&compressed_data).expect("Failed to decompress zstd")
    };

    // let mut temp_exe = NamedTempFile::new().unwrap();
    // temp_exe.write_all(&decompressed).unwrap();
    // let path = temp_exe.into_temp_path();

    // let original_exe_dir = env::current_exe()
    //     .ok()
    //     .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    //     .unwrap_or_else(|| ".".into());

    // Command::new(&path)
    //     .current_dir(original_exe_dir)
    //     .spawn()
    //     .expect("Failed to launch extracted EXE");

    let px = Path::new(&xfilenamex)
        .file_name()       // Gets just the filename (OsStr)
        .unwrap()          // or handle Option
        .to_string_lossy() // Converts OsStr to String
        .to_string();
    let mut path = env::temp_dir();
    path.push(generate_random_string(10));
    std::fs::create_dir_all(&path);
    path.push(px.to_string());

    //fs::write_all("a.txt", &path.display().to_string());
    //File::create("a.txt").unwrap().write_all(&path.display().to_string().as_bytes()).unwrap();

    let mut file = File::create(&path).expect("Failed to create temp exe");
    file.write_all(&decompressed).expect("Failed to write payload");

    let path2 = path.display().to_string();
    let original_exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    file.close();

    //File::create("a.txt").unwrap().write_all(&original_exe_dir.display().to_string().as_bytes()).unwrap();
    Command::new(&path2)
        .current_dir(original_exe_dir)
        .spawn()
        .expect("Failed to launch extracted EXE");
}

fn decompress_lzma(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = XzDecoder::new(data);
    let mut out = vec![];
    std::io::copy(&mut decoder, &mut out)?;
    Ok(out)
}

fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = zstd::stream::Decoder::new(data)?;
    let mut out = vec![];
    std::io::copy(&mut decoder, &mut out)?;
    Ok(out)
}
