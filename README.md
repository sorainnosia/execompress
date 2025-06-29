# ExeCompress
```
Usage: execompress [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>    Input executable
  -e, --extra <EXTRA>    Extra path to a directory containing files and directories to pack/unpack together
  -o, --output <OUTPUT>  Output compressed executable
  -l, --level <LEVEL>    Compression level: 1-9 (default) 1-22 (--zstd) [default: 3]
      --zstd             Use zstd instead of lzma
      --gui              When input file is GUI app, suppress command line window
  -h, --help             Print help
```
Compress Windows executable file becoming smaller size and able to run. During run the program self unpack and place the file into temporary folder and execute the file as if it is running in the current directory. When program terminates, the temporary file is deleted.

# Compile
1. Compile the main program using `cargo build -r`
2. Copy ExeCompress.exe to the same folder level with `stub_loader` folder

# Notes
1. During run the program will extract icon from input executable into stub_loader folder which also contains stub_loader source code.
2. The stub_loader source is compiled using Rust along with the extracted icon and output as final compressed file
3. The output file is then generated

# Example using zstd algorithm
`-l` means compression level, `zstd` supports from compression level of `1 to 22`
```
execompress --input "C:\folder\input.exe" --output "output.exe" -l 20 --gui --zstd
```
# Example using XzEncoder algorithm
`--gui` means the input.exe is a GUI app, and it suppress the command line console from being shown. Using `XzEncoder` (default), maximum compression level is `1 to 9`.

```
execompress --input "C:\folder\input.exe" --output "output.exe" -l 9 --gui
```

# Requirements
Rust is installed and in Environment Path during execution of execompress.

# AntiVirus
Exe compression program maybe falsely detected as virus.
