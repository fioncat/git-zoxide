use std::process;

mod config;
mod db;
mod util;

fn main() {
    let mut db = match db::Database::open() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    };

    if let Some(repo) = db.get("test") {
        println!("repo = {:?}", repo);
    }

    if let Err(err) = db.save() {
        eprintln!("error: {err}");
        process::exit(1);
    }

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
