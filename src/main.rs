use std::env;
use std::path::Path;

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "rust_plc".to_string());

    let Some(path) = args.next() else {
        eprintln!("Usage: {program} <file.plc>");
        std::process::exit(1);
    };

    if args.next().is_some() {
        eprintln!("Usage: {program} <file.plc>");
        std::process::exit(1);
    }

    if Path::new(&path).extension().and_then(|ext| ext.to_str()) != Some("plc") {
        eprintln!("Expected a .plc file path, got: {path}");
        std::process::exit(1);
    }

    println!("RustPLC CLI initialized. Input file: {path}");
}
