# ExeCompress
Compress Windows executable file becoming smaller size and able to run. During run the program self unpack and place the file into temporary folder and execute the file as if it is running in the current directory. When program terminates, the temporary file is deleted.

# Compile
1. Compile the main program using `cargo build -r`
2. Copy ExeCompress.exe to the same folder level with `stub_loader` folder

# Notes
1. During run the program will extract icon from input executable into stub_loader folder which also contains stub_loader source code.
2. The stub_loader source is compiled using Rust along with the extracted icon and output as final compressed file
3. The output file is then generated

# Example using zstd algorithm
`-l` means compression level, `zstd` supports from compression level of `0 to 22`
```
execompress --input "C:\folder\input.exe" --output "output.exe" -l 20 --gui --zstd
```
# Example using XzEncoder algorithm
`--gui` means the input.exe is a GUI app, and it suppress the command line console from being shown. Using `XzEncoder` (default), maximum compression level is `0 to 9`.

```
execompress --input "C:\folder\input.exe" --output "output.exe" -l 9 --gui
```

# Requirements
Rust is installed and in Environment Path during execution of execompress.

# AntiVirus
Exe compression program maybe falsely detected as virus.
