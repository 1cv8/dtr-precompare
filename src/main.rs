use std::io;
use std::path::Path;
use std::process;
use std::time::Instant;

mod dtr_precompare;

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Целевой каталог
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    target_dir: String,

    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> io::Result<()> {
    let start_time = Instant::now();

    let args = Args::parse();

    // Проверяем аргументы командной строки
    if args.target_dir != "." {
        let path = Path::new(&args.target_dir);
        // Проверяем существование каталога
        if !path.exists() {
            eprintln!("Ошибка: каталог '{}' не существует", args.target_dir);
            process::exit(1);
        };

        // Проверяем, что это действительно каталог
        if !path.is_dir() {
            eprintln!("Ошибка: '{}' не является каталогом", args.target_dir);
            process::exit(1);
        }
        println!("processing in {}", args.target_dir);
    };

    dtr_precompare::run(&args.target_dir)?;

    println!(
        "All is Done за {:.3} секунд",
        start_time.elapsed().as_secs_f64()
    );
    Ok(())
}
