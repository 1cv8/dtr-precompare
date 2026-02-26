use std::env;
use std::io;
use std::path::Path;
use std::process;
use std::time::Instant;

mod dtr_precompare;

fn main() -> io::Result<()> {
    let start_time = Instant::now();

    // Получаем аргументы командной строки
    let args: Vec<String> = env::args().collect();

    // Определяем целевой каталог
    let target_dir: String;
    if args.len() > 1 {
        target_dir = args[1].clone();

        let path = Path::new(&target_dir);
        // Проверяем существование каталога
        if !path.exists() {
            eprintln!("Ошибка: каталог '{}' не существует", target_dir);
            process::exit(1);
        };

        // Проверяем, что это действительно каталог
        if !path.is_dir() {
            eprintln!("Ошибка: '{}' не является каталогом", target_dir);
            process::exit(1);
        }
        println!("processing in {}", target_dir);
    } else {
        target_dir = ".".to_string();
        println!("processing in current dir");
    };

    dtr_precompare::run(target_dir)?;

    println!(
        "All is Done за {:.3} секунд",
        start_time.elapsed().as_secs_f64()
    );
    Ok(())
}
