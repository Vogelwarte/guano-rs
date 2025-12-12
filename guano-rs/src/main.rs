use clap::Parser;
use guano_rs::{GuanoFile, GuanoValue};
use serde_json::{Map, Value};
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
        "json" => print_json(guano.metadata(), !args.compact),
        "kv" => print_kv(guano.metadata()),
        _ => {
            eprintln!("Invalid format. Use 'kv' or 'json'");
            process::exit(1);
        }
    }
}

fn print_kv(metadata: &std::collections::HashMap<String, GuanoValue>) {
    for (key, value) in metadata {
        match value {
            GuanoValue::String(s) => println!("{}: {}", key, s),
            GuanoValue::Object(obj) => {
                for (sub_key, sub_value) in obj {
                    if let GuanoValue::String(s) = sub_value {
                        println!("{}|{}: {}", key, sub_key, s);
                    }
                }
            }
        }
    }
}

fn print_json(metadata: &std::collections::HashMap<String, GuanoValue>, pretty: bool) {
    let json_map = convert_to_json(metadata);
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json_map).unwrap());
    } else {
        println!("{}", serde_json::to_string(&json_map).unwrap());
    }
}

fn convert_to_json(metadata: &std::collections::HashMap<String, GuanoValue>) -> Value {
    let mut map = Map::new();
    for (key, value) in metadata {
        match value {
            GuanoValue::String(s) => {
                map.insert(key.clone(), Value::String(s.clone()));
            }
            GuanoValue::Object(obj) => {
                let mut sub_map = Map::new();
                for (sub_key, sub_value) in obj {
                    if let GuanoValue::String(s) = sub_value {
                        sub_map.insert(sub_key.clone(), Value::String(s.clone()));
                    }
                }
                map.insert(key.clone(), Value::Object(sub_map));
            }
        }
    }
    Value::Object(map)
}
