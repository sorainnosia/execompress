#![cfg_attr(feature = "gui", windows_subsystem = "windows")]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::fs::File;
use std::fs::{remove_dir_all};
use std::io::{BufRead, BufReader, Write, Read};
use std::path::{PathBuf, Path};
use std::process::Command;
use std::env;
use tempfile::NamedTempFile;
use xz2::write::XzEncoder;
use xz2::read::XzDecoder;
use close_file::Closable;
use rand::{distributions::Alphanumeric, Rng};
use fs_more::directory::{move_directory, DirectoryMoveOptions, DestinationDirectoryRule};
use fs_more::file::remove_file;
use std::io;
use std::fs;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

pub fn delete_all_files_in_folder(dir_path: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir_path.clone())? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            remove_file(&path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else if path.is_dir() {
            delete_all_files_in_folder(&path)?;
            remove_dir_all(&path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
	remove_dir_all(&dir_path);

    Ok(())
}

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
	let marker_extra = b"--EXTRA-FILE--\n";
	
    let mut payload_start = None;
    let mut filename_start = None;
	let mut extra_starts = vec![];

    for i in 0..buffer.len() {
        if i + marker_payload.len() <= buffer.len() && &buffer[i..i + marker_payload.len()] == marker_payload {
            payload_start = Some(i + marker_payload.len());
        }
        if i + marker_filename.len() <= buffer.len() && &buffer[i..i + marker_filename.len()] == marker_filename {
            filename_start = Some(i + marker_filename.len());
        }
		if i + marker_extra.len() <= buffer.len() && &buffer[i..i + marker_extra.len()] == marker_extra {
            extra_starts.push(i + marker_extra.len());
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

	let decompressed = decompress(&compressed_data);
    let px = Path::new(&xfilenamex)
        .file_name()       // Gets just the filename (OsStr)
        .unwrap()          // or handle Option
        .to_string_lossy() // Converts OsStr to String
        .to_string();
    let mut path = env::temp_dir();
    path.push(generate_random_string(10));
	let mut path_dir = path.clone();
    std::fs::create_dir_all(&path);
    path.push(px.to_string());

    let mut file = File::create(&path).expect("Failed to create temp exe");
    file.write_all(&decompressed).expect("Failed to write payload");

    let path2 = path.display().to_string();
    let original_exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    file.close();
	
	//for start in extra_starts {
	let pool = ThreadPoolBuilder::new()
				.num_threads(4)
				.build()
				.unwrap();

	pool.install(|| {
		extra_starts.into_iter().par_bridge().for_each(|start| {
			if let Some(path_len_pos) = buffer[start..].iter().position(|&b| b == b'\n') {
				let path_str = String::from_utf8_lossy(&buffer[start..start + path_len_pos]).to_string();
				let mut full_path = path_dir.clone();
				if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(path_str) {
					full_path = path_dir.join(String::from_utf8_lossy(&decoded).to_string());
				}

				let content_start = start + path_len_pos + 1;
				if let Some(len_end) = buffer[content_start..].iter().position(|&b| b == b'\n') {
					let len_str = String::from_utf8_lossy(&buffer[content_start..content_start + len_end]);
					if let Ok(payload_len) = len_str.trim().parse::<usize>() {
						let bin_start = content_start + len_end + 1;
						let compressed = &buffer[bin_start..bin_start + payload_len];
						let content = decompress(compressed);
						if let Some(parent) = full_path.parent() {
							fs::create_dir_all(parent).ok();
						}
						File::create(&full_path).unwrap().write_all(&content).unwrap();
					}
				}
			}
		});
    });
	
    let mut child = Command::new(&path2)
        .current_dir(original_exe_dir)
        .spawn()
        .expect("Failed to launch extracted EXE");
		
	let _ = child.wait(); 
	delete_all_files_in_folder(&path_dir);
	remove_dir_all(&path_dir);
}

fn decompress(data: &[u8]) -> Vec<u8> {
    let decompressed = if let Ok(decompressed) = decompress_lzma(data) {
        decompressed
    } else {
        decompress_zstd(data).expect("Failed to decompress zstd")
    };
	return decompressed;
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
