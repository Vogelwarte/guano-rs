use clap::Parser;
use guano_rs::{GuanoFile, GuanoValue};
use std::fs::File;
use std::process;

#[derive(Parser)]
#[command(name = "guano-rs")]
#[command(about = "Display GUANO metadata from WAV files", long_about = None)]
struct Args {
    /// Path to the WAV file
    file: String,

    /// Output format: kv (key-value) or json
    #[arg(short, long, default_value = "kv")]
    format: String,

    #[arg(short, long)]
    compact: bool,
}

fn main() {
    let args = Args::parse();

    let file = match File::open(&args.file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening file: {}", e);
            process::exit(1);
        }
    };

    let guano = match GuanoFile::new(file) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error reading GUANO metadata: {}", e);
            process::exit(1);
        }
    };

    match args.format.as_str() {
        "json" => print_json(&guano, !args.compact),
        "kv" => print_kv(guano.metadata()),
        _ => {
            eprintln!("Invalid format. Use 'kv' or 'json'");
            process::exit(1);
        }
    }
}

/// Sorts entries alphabetically. Iterating the `HashMap` directly would print the fields in a
/// different order on every run.
fn sorted(metadata: &std::collections::HashMap<String, GuanoValue>) -> Vec<(&String, &GuanoValue)> {
    let mut entries: Vec<_> = metadata.iter().collect();
    entries.sort_by_key(|(key, _)| *key);
    entries
}

/// Sorts root entries the way the library serializes them: the `GUANO` namespace first, because
/// the specification requires `GUANO|Version` to lead, then the rest alphabetically.
fn ordered(
    metadata: &std::collections::HashMap<String, GuanoValue>,
) -> Vec<(&String, &GuanoValue)> {
    let mut entries = sorted(metadata);
    entries.sort_by_key(|(k, _)| k.as_str() != "GUANO");
    entries
}

fn print_kv(metadata: &std::collections::HashMap<String, GuanoValue>) {
    for (key, value) in ordered(metadata) {
        match value {
            GuanoValue::String(s) => println!("{}: {}", key, s),
            GuanoValue::Object(obj) => {
                for (sub_key, sub_value) in sorted(obj) {
                    if let GuanoValue::String(s) = sub_value {
                        println!("{}|{}: {}", key, sub_key, s);
                    }
                }
            }
        }
    }
}

fn print_json(guano: &GuanoFile, pretty: bool) {
    let json = if pretty {
        serde_json::to_string_pretty(guano)
    } else {
        serde_json::to_string(guano)
    };
    match json {
        Ok(s) => println!("{}", s),
        Err(e) => {
            eprintln!("Error serializing GUANO metadata: {}", e);
            process::exit(1);
        }
    }
}
