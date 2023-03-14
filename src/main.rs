use std::process;

mod config;

fn main() {
    match config::parse() {
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
        Ok(config) => {
            println!("config: {:?}", config);
        }
    }
}
