use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

// PE file structure constants
const PE_SIGNATURE: u32 = 0x00004550; // "PE\0\0"
const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D; // "MZ"

// Resource types
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

#[derive(Debug)]
struct IconDirEntry {
    width: u8,
    height: u8,
    color_count: u8,
    reserved: u8,
    planes: u16,
    bit_count: u16,
    bytes_in_res: u32,
    image_offset: u32,
}

#[derive(Debug)]
struct IconDir {
    reserved: u16,
    type_: u16,
    count: u16,
    entries: Vec<IconDirEntry>,
}

#[derive(Debug)]
struct GroupIconDirEntry {
    width: u8,
    height: u8,
    color_count: u8,
    reserved: u8,
    planes: u16,
    bit_count: u16,
    bytes_in_res: u32,
    id: u16,
}

#[derive(Debug)]
struct ResourceDirectory {
    characteristics: u32,
    time_date_stamp: u32,
    major_version: u16,
    minor_version: u16,
    number_of_name_entries: u16,
    number_of_id_entries: u16,
}

#[derive(Debug)]
struct ResourceDirectoryEntry {
    name_or_id: u32,
    offset_to_data_or_subdirectory: u32,
}

#[derive(Debug)]
struct ResourceDataEntry {
    offset_to_data: u32,
    size: u32,
    code_page: u32,
    reserved: u32,
}

pub struct IconExtractor {
    file: File,
    pe_offset: u32,
    resource_section_offset: u32,
    resource_section_virtual_address: u32,
}

impl IconExtractor {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(&path)?;
        
        println!("Opening file: {}", path.as_ref().display());
        
        // Read DOS header
        let mut dos_signature = [0u8; 2];
        file.read_exact(&mut dos_signature)?;
        
        if u16::from_le_bytes(dos_signature) != IMAGE_DOS_SIGNATURE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid PE file (invalid DOS signature)"));
        }
        
        println!("Valid DOS signature found");
        
        // Seek to PE header offset location
        file.seek(SeekFrom::Start(0x3C))?;
        let mut pe_offset_bytes = [0u8; 4];
        file.read_exact(&mut pe_offset_bytes)?;
        let pe_offset = u32::from_le_bytes(pe_offset_bytes);
        
        println!("PE header offset: 0x{:08X}", pe_offset);
        
        // Read PE signature
        file.seek(SeekFrom::Start(pe_offset as u64))?;
        let mut pe_signature = [0u8; 4];
        file.read_exact(&mut pe_signature)?;
        
        if u32::from_le_bytes(pe_signature) != PE_SIGNATURE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid PE signature"));
        }
        
        println!("Valid PE signature found");
        
        match Self::find_resource_section(&mut file, pe_offset) {
            Ok((resource_section_offset, resource_section_virtual_address)) => {
                println!("Resource section found at offset: 0x{:08X}, VA: 0x{:08X}", 
                    resource_section_offset, resource_section_virtual_address);
                    
                Ok(IconExtractor {
                    file,
                    pe_offset,
                    resource_section_offset,
                    resource_section_virtual_address,
                })
            }
            Err(e) => {
                println!("Failed to find resource section: {}", e);
                // Try alternative method - check if resources are in the data directory
                Self::try_alternative_resource_detection(&mut file, pe_offset, &path)
            }
        }
    }
    
    fn try_alternative_resource_detection<P: AsRef<Path>>(file: &mut File, pe_offset: u32, path: P) -> io::Result<Self> {
        println!("Attempting alternative resource detection...");
        
        // Read the optional header to get the resource data directory
        file.seek(SeekFrom::Start((pe_offset + 4) as u64))?;
        
        // Skip COFF header (20 bytes)
        file.seek(SeekFrom::Current(20))?;
        
        // Read optional header magic to determine if it's PE32 or PE32+
        let mut magic = [0u8; 2];
        file.read_exact(&mut magic)?;
        let magic = u16::from_le_bytes(magic);
        
        let data_directory_offset = match magic {
            0x10b => 96,  // PE32
            0x20b => 112, // PE32+
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown PE format")),
        };
        
        // Seek to the resource data directory entry (entry 2)
        file.seek(SeekFrom::Start((pe_offset + 4 + 20 + data_directory_offset + 2 * 8) as u64))?;
        
        let mut resource_rva = [0u8; 4];
        file.read_exact(&mut resource_rva)?;
        let resource_rva = u32::from_le_bytes(resource_rva);
        
        let mut resource_size = [0u8; 4];
        file.read_exact(&mut resource_size)?;
        let resource_size = u32::from_le_bytes(resource_size);
        
        if resource_rva == 0 || resource_size == 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, 
                "No resource directory found in data directory"));
        }
        
        println!("Found resource directory in data directory: RVA=0x{:08X}, Size={}", 
            resource_rva, resource_size);
        
        // Now we need to find which section contains this RVA
        let (resource_section_offset, resource_section_virtual_address) = 
            Self::find_section_for_rva(file, pe_offset, resource_rva)?;
            
        Ok(IconExtractor {
            file: File::open(path.as_ref())?,
            pe_offset,
            resource_section_offset,
            resource_section_virtual_address,
        })
    }
    
    fn find_section_for_rva(file: &mut File, pe_offset: u32, target_rva: u32) -> io::Result<(u32, u32)> {
        // Skip PE signature (4 bytes)
        file.seek(SeekFrom::Start((pe_offset + 4) as u64))?;
        
        // Read COFF header
        let mut machine = [0u8; 2];
        file.read_exact(&mut machine)?;
        
        let mut number_of_sections = [0u8; 2];
        file.read_exact(&mut number_of_sections)?;
        let number_of_sections = u16::from_le_bytes(number_of_sections);
        
        // Skip to optional header size
        file.seek(SeekFrom::Current(12))?;
        
        let mut optional_header_size = [0u8; 2];
        file.read_exact(&mut optional_header_size)?;
        let optional_header_size = u16::from_le_bytes(optional_header_size);
        
        // Skip characteristics and optional header
        file.seek(SeekFrom::Current(2 + optional_header_size as i64))?;
        
        // Check each section
        for i in 0..number_of_sections {
            let mut section_name = [0u8; 8];
            file.read_exact(&mut section_name)?;
            
            let mut virtual_size = [0u8; 4];
            file.read_exact(&mut virtual_size)?;
            let virtual_size = u32::from_le_bytes(virtual_size);
            
            let mut virtual_address = [0u8; 4];
            file.read_exact(&mut virtual_address)?;
            let virtual_address = u32::from_le_bytes(virtual_address);
            
            let mut size_of_raw_data = [0u8; 4];
            file.read_exact(&mut size_of_raw_data)?;
            
            let mut pointer_to_raw_data = [0u8; 4];
            file.read_exact(&mut pointer_to_raw_data)?;
            let pointer_to_raw_data = u32::from_le_bytes(pointer_to_raw_data);
            
            // Skip remaining section header fields
            file.seek(SeekFrom::Current(16))?;
            
            // Check if target RVA falls within this section
            if target_rva >= virtual_address && target_rva < virtual_address + virtual_size {
                let section_name_str = String::from_utf8_lossy(&section_name)
                    .trim_end_matches('\0')
                    .to_string();
                println!("Found target RVA in section: '{}'", section_name_str);
                return Ok((pointer_to_raw_data, virtual_address));
            }
        }
        
        Err(io::Error::new(io::ErrorKind::NotFound, 
            format!("No section contains RVA 0x{:08X}", target_rva)))
    }
    
    fn find_resource_section(file: &mut File, pe_offset: u32) -> io::Result<(u32, u32)> {
        // Skip PE signature (4 bytes)
        file.seek(SeekFrom::Start((pe_offset + 4) as u64))?;
        
        // Read COFF header
        let mut machine = [0u8; 2];
        file.read_exact(&mut machine)?;
        
        let mut number_of_sections = [0u8; 2];
        file.read_exact(&mut number_of_sections)?;
        let number_of_sections = u16::from_le_bytes(number_of_sections);
        
        // Skip time_date_stamp (4 bytes) and pointer_to_symbol_table (4 bytes) and number_of_symbols (4 bytes)
        file.seek(SeekFrom::Current(12))?;
        
        let mut optional_header_size = [0u8; 2];
        file.read_exact(&mut optional_header_size)?;
        let optional_header_size = u16::from_le_bytes(optional_header_size);
        
        // Skip characteristics (2 bytes)
        file.seek(SeekFrom::Current(2))?;
        
        // Skip optional header entirely
        file.seek(SeekFrom::Current(optional_header_size as i64))?;
        
        println!("Found {} sections", number_of_sections);
        
        // Read section headers
        for i in 0..number_of_sections {
            let mut section_name = [0u8; 8];
            file.read_exact(&mut section_name)?;
            
            let mut virtual_size = [0u8; 4];
            file.read_exact(&mut virtual_size)?;
            
            let mut virtual_address = [0u8; 4];
            file.read_exact(&mut virtual_address)?;
            let virtual_address = u32::from_le_bytes(virtual_address);
            
            let mut size_of_raw_data = [0u8; 4];
            file.read_exact(&mut size_of_raw_data)?;
            
            let mut pointer_to_raw_data = [0u8; 4];
            file.read_exact(&mut pointer_to_raw_data)?;
            let pointer_to_raw_data = u32::from_le_bytes(pointer_to_raw_data);
            
            // Skip remaining section header fields (20 bytes)
            file.seek(SeekFrom::Current(16))?;
            
            // Convert section name to string for debugging
            let section_name_str = String::from_utf8_lossy(&section_name)
                .trim_end_matches('\0')
                .to_string();
            
            println!("Section {}: '{}' (raw bytes: {:?})", i, section_name_str, &section_name[0..8]);
            println!("  Virtual Address: 0x{:08X}", virtual_address);
            println!("  Pointer to Raw Data: 0x{:08X}", pointer_to_raw_data);
            println!("  Size of Raw Data: {}", u32::from_le_bytes(size_of_raw_data));
            
            // Check if this is the resource section (try multiple variations)
            let name_lower = section_name_str.to_lowercase();
            if name_lower.starts_with(".rsrc") || 
               section_name_str.starts_with(".rsrc") ||
               &section_name[0..5] == b".rsrc" {
                println!("Found resource section: '{}'", section_name_str);
                return Ok((pointer_to_raw_data, virtual_address));
            }
        }
        
        Err(io::Error::new(io::ErrorKind::NotFound, 
            format!("Resource section not found. Found {} sections, but none were named '.rsrc'", number_of_sections)))
    }
    
    fn read_u16(&mut self, offset: u64) -> io::Result<u16> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut bytes = [0u8; 2];
        self.file.read_exact(&mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }
    
    fn read_u32(&mut self, offset: u64) -> io::Result<u32> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut bytes = [0u8; 4];
        self.file.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    
    fn read_resource_directory(&mut self, offset: u32) -> io::Result<ResourceDirectory> {
        let base_offset = (self.resource_section_offset + offset) as u64;
        
        Ok(ResourceDirectory {
            characteristics: self.read_u32(base_offset)?,
            time_date_stamp: self.read_u32(base_offset + 4)?,
            major_version: self.read_u16(base_offset + 8)?,
            minor_version: self.read_u16(base_offset + 10)?,
            number_of_name_entries: self.read_u16(base_offset + 12)?,
            number_of_id_entries: self.read_u16(base_offset + 14)?,
        })
    }
    
    fn read_resource_directory_entry(&mut self, offset: u32) -> io::Result<ResourceDirectoryEntry> {
        let base_offset = (self.resource_section_offset + offset) as u64;
        
        Ok(ResourceDirectoryEntry {
            name_or_id: self.read_u32(base_offset)?,
            offset_to_data_or_subdirectory: self.read_u32(base_offset + 4)?,
        })
    }
    
    fn read_resource_data_entry(&mut self, offset: u32) -> io::Result<ResourceDataEntry> {
        let base_offset = (self.resource_section_offset + offset) as u64;
        
        Ok(ResourceDataEntry {
            offset_to_data: self.read_u32(base_offset)?,
            size: self.read_u32(base_offset + 4)?,
            code_page: self.read_u32(base_offset + 8)?,
            reserved: self.read_u32(base_offset + 12)?,
        })
    }
    
    fn read_data(&mut self, rva: u32, size: u32) -> io::Result<Vec<u8>> {
        let file_offset = rva - self.resource_section_virtual_address + self.resource_section_offset;
        self.file.seek(SeekFrom::Start(file_offset as u64))?;
        let mut data = vec![0u8; size as usize];
        self.file.read_exact(&mut data)?;
        Ok(data)
    }
    
    pub fn extract_largest_icon(&mut self) -> io::Result<Vec<u8>> {
        println!("Starting icon extraction...");
        
        // Read root resource directory
        let root_dir = self.read_resource_directory(0)?;
        println!("Root resource directory: {} name entries, {} ID entries", 
            root_dir.number_of_name_entries, root_dir.number_of_id_entries);
        
        // List all resource types first for debugging
        self.list_resource_types()?;
        
        // Find icon group resource type
        let mut current_offset = 16u32; // Size of ResourceDirectory
        let mut group_icon_offset = None;
        
        // Look through all entries for RT_GROUP_ICON
        for i in 0..(root_dir.number_of_name_entries + root_dir.number_of_id_entries) {
            let entry = self.read_resource_directory_entry(current_offset)?;
            
            println!("Resource entry {}: ID/Name=0x{:08X}, Offset=0x{:08X}", 
                i, entry.name_or_id, entry.offset_to_data_or_subdirectory);
            
            if (entry.name_or_id & 0x80000000) == 0 && entry.name_or_id == RT_GROUP_ICON as u32 {
                group_icon_offset = Some(entry.offset_to_data_or_subdirectory & 0x7FFFFFFF);
                println!("Found RT_GROUP_ICON at offset: 0x{:08X}", group_icon_offset.unwrap());
                break;
            }
            current_offset += 8; // Size of ResourceDirectoryEntry
        }
        
        let group_icon_offset = group_icon_offset.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "No icon group found")
        })?;
        
        // Read icon group directory
        let group_dir = self.read_resource_directory(group_icon_offset)?;
        println!("Icon group directory: {} name entries, {} ID entries", 
            group_dir.number_of_name_entries, group_dir.number_of_id_entries);
        
        if group_dir.number_of_id_entries == 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No icon groups found"));
        }
        
        // Get first icon group
        let group_entry_offset = group_icon_offset + 16;
        let group_entry = self.read_resource_directory_entry(group_entry_offset)?;
        println!("First icon group entry: ID/Name=0x{:08X}, Offset=0x{:08X}", 
            group_entry.name_or_id, group_entry.offset_to_data_or_subdirectory);
        
        // Read the actual icon group data
        let group_data_dir = self.read_resource_directory(group_entry.offset_to_data_or_subdirectory & 0x7FFFFFFF)?;
        let group_data_entry_offset = (group_entry.offset_to_data_or_subdirectory & 0x7FFFFFFF) + 16;
        let group_data_entry = self.read_resource_directory_entry(group_data_entry_offset)?;
        let group_data_info = self.read_resource_data_entry(group_data_entry.offset_to_data_or_subdirectory)?;
        
        println!("Icon group data: RVA=0x{:08X}, Size={}", 
            group_data_info.offset_to_data, group_data_info.size);
        
        let group_data = self.read_data(group_data_info.offset_to_data, group_data_info.size)?;
        
        // Parse icon group data
        if group_data.len() < 6 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid icon group data"));
        }
        
        let count = u16::from_le_bytes([group_data[4], group_data[5]]) as usize;
        println!("Found {} icons in group", count);
        
        let mut group_entries = Vec::new();
        
        for i in 0..count {
            let base = 6 + i * 14;
            if base + 14 > group_data.len() {
                break;
            }
            
            let entry = GroupIconDirEntry {
                width: group_data[base],
                height: group_data[base + 1],
                color_count: group_data[base + 2],
                reserved: group_data[base + 3],
                planes: u16::from_le_bytes([group_data[base + 4], group_data[base + 5]]),
                bit_count: u16::from_le_bytes([group_data[base + 6], group_data[base + 7]]),
                bytes_in_res: u32::from_le_bytes([
                    group_data[base + 8], group_data[base + 9],
                    group_data[base + 10], group_data[base + 11]
                ]),
                id: u16::from_le_bytes([group_data[base + 12], group_data[base + 13]]),
            };
            
            println!("Icon {}: {}x{}, {} bytes, ID={}", 
                i, entry.width, entry.height, entry.bytes_in_res, entry.id);
            
            group_entries.push(entry);
        }
        
        if group_entries.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No icons found in group"));
        }
        
        // Find the largest icon (by size in bytes, then by dimensions)
        let largest_entry = group_entries.iter()
            .max_by(|a, b| {
                a.bytes_in_res.cmp(&b.bytes_in_res)
                    .then_with(|| (a.width as u32 * a.height as u32).cmp(&(b.width as u32 * b.height as u32)))
            })
            .unwrap();
        
        println!("Selected largest icon: {}x{}, {} bytes, ID={}", 
            largest_entry.width, largest_entry.height, largest_entry.bytes_in_res, largest_entry.id);
        
        // Now find and extract the actual icon data
        let icon_data = self.find_and_extract_icon(largest_entry.id)?;
        
        // Create a proper ICO file
        self.create_ico_file(&[largest_entry], &[icon_data])
    }
    
    fn list_resource_types(&mut self) -> io::Result<()> {
        println!("Listing all resource types...");
        let root_dir = self.read_resource_directory(0)?;
        let mut current_offset = 16u32;
        
        for i in 0..(root_dir.number_of_name_entries + root_dir.number_of_id_entries) {
            let entry = self.read_resource_directory_entry(current_offset)?;
            
            if (entry.name_or_id & 0x80000000) == 0 {
                let resource_type = match entry.name_or_id {
                    1 => "RT_CURSOR".to_string(),
                    2 => "RT_BITMAP".to_string(),
                    3 => "RT_ICON".to_string(),
                    4 => "RT_MENU".to_string(),
                    5 => "RT_DIALOG".to_string(),
                    6 => "RT_STRING".to_string(),
                    7 => "RT_FONTDIR".to_string(),
                    8 => "RT_FONT".to_string(),
                    9 => "RT_ACCELERATOR".to_string(),
                    10 => "RT_RCDATA".to_string(),
                    11 => "RT_MESSAGETABLE".to_string(),
                    12 => "RT_GROUP_CURSOR".to_string(),
                    14 => "RT_GROUP_ICON".to_string(),
                    16 => "RT_VERSION".to_string(),
                    17 => "RT_DLGINCLUDE".to_string(),
                    19 => "RT_PLUGPLAY".to_string(),
                    20 => "RT_VXD".to_string(),
                    21 => "RT_ANICURSOR".to_string(),
                    22 => "RT_ANIICON".to_string(),
                    23 => "RT_HTML".to_string(),
                    24 => "RT_MANIFEST".to_string(),
                    _ => format!("UNKNOWN({})", entry.name_or_id),
                };
                println!("  Resource type: {}", resource_type);
            } else {
                println!("  Named resource: 0x{:08X}", entry.name_or_id);
            }
            
            current_offset += 8;
        }
        
        Ok(())
    }
    
    fn find_and_extract_icon(&mut self, icon_id: u16) -> io::Result<Vec<u8>> {
        // Find RT_ICON in root directory
        let root_dir = self.read_resource_directory(0)?;
        let mut current_offset = 16u32;
        let mut icon_offset = None;
        
        for _ in 0..(root_dir.number_of_name_entries + root_dir.number_of_id_entries) {
            let entry = self.read_resource_directory_entry(current_offset)?;
            
            if (entry.name_or_id & 0x80000000) == 0 && entry.name_or_id == RT_ICON as u32 {
                icon_offset = Some(entry.offset_to_data_or_subdirectory & 0x7FFFFFFF);
                break;
            }
            current_offset += 8;
        }
        
        let icon_offset = icon_offset.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "No icon resources found")
        })?;
        
        // Find the specific icon by ID
        let icon_dir = self.read_resource_directory(icon_offset)?;
        let mut entry_offset = icon_offset + 16;
        
        for _ in 0..(icon_dir.number_of_name_entries + icon_dir.number_of_id_entries) {
            let entry = self.read_resource_directory_entry(entry_offset)?;
            
            if (entry.name_or_id & 0x80000000) == 0 && entry.name_or_id == icon_id as u32 {
                // Found the icon, get its data
                let lang_dir = self.read_resource_directory(entry.offset_to_data_or_subdirectory & 0x7FFFFFFF)?;
                let lang_entry_offset = (entry.offset_to_data_or_subdirectory & 0x7FFFFFFF) + 16;
                let lang_entry = self.read_resource_directory_entry(lang_entry_offset)?;
                let data_entry = self.read_resource_data_entry(lang_entry.offset_to_data_or_subdirectory)?;
                
                return self.read_data(data_entry.offset_to_data, data_entry.size);
            }
            entry_offset += 8;
        }
        
        Err(io::Error::new(io::ErrorKind::NotFound, "Icon not found"))
    }
    
    fn create_ico_file(&self, entries: &[&GroupIconDirEntry], icon_data: &[Vec<u8>]) -> io::Result<Vec<u8>> {
        let mut ico_file = Vec::new();
        
        // ICO header
        ico_file.extend_from_slice(&[0u8, 0u8]); // Reserved
        ico_file.extend_from_slice(&[1u8, 0u8]); // Type (1 = ICO)
        ico_file.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // Count
        
        let mut data_offset = 6 + entries.len() * 16; // Header + directory entries
        
        // Directory entries
        for (i, entry) in entries.iter().enumerate() {
            ico_file.push(if entry.width == 0 { 0 } else { entry.width }); // Width (0 = 256)
            ico_file.push(if entry.height == 0 { 0 } else { entry.height }); // Height (0 = 256)
            ico_file.push(entry.color_count); // Color count
            ico_file.push(0); // Reserved
            ico_file.extend_from_slice(&entry.planes.to_le_bytes()); // Planes
            ico_file.extend_from_slice(&entry.bit_count.to_le_bytes()); // Bit count
            ico_file.extend_from_slice(&(icon_data[i].len() as u32).to_le_bytes()); // Size
            ico_file.extend_from_slice(&(data_offset as u32).to_le_bytes()); // Offset
            
            data_offset += icon_data[i].len();
        }
        
        // Icon data
        for data in icon_data {
            ico_file.extend_from_slice(data);
        }
        
        Ok(ico_file)
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() != 3 {
        eprintln!("Usage: {} <input.exe> <output.ico>", args[0]);
        std::process::exit(1);
    }
    
    let input_path = &args[1];
    let output_path = &args[2];
    
    println!("Extracting icon from: {}", input_path);
    
    match IconExtractor::new(input_path) {
        Ok(mut extractor) => {
            match extractor.extract_largest_icon() {
                Ok(ico_data) => {
                    let mut output_file = File::create(output_path)?;
                    output_file.write_all(&ico_data)?;
                    
                    println!("Icon successfully extracted to: {}", output_path);
                    println!("Icon size: {} bytes", ico_data.len());
                }
                Err(e) => {
                    eprintln!("Failed to extract icon: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to open PE file: {}", e);
            println!("\nTroubleshooting tips:");
            println!("1. Make sure the file is a valid Windows executable (.exe, .dll, etc.)");
            println!("2. Check that the file actually contains icon resources");
            println!("3. Try running on a different executable file to test");
            std::process::exit(1);
        }
    }
    
    Ok(())
}