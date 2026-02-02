
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
//use std::thread;
//use crossbeam_channel::{unbounded, Receiver};
use regex::Regex;
use walkdir::WalkDir;
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;

fn main() -> io::Result<()> {
    // Компилируем регулярные выражения один раз
    let patterns = Arc::new([
        (Regex::new(r#""FolderId":\s*"[0-9a-f-]*""#).unwrap(), 
         r#""FolderId": "00000000-0000-0000-0000-000000000000""#),
        (Regex::new(r#""ClusterId":\s*"[0-9a-f-]*""#).unwrap(), 
         r#""ClusterId": "00000000-0000-0000-0000-000000000000""#),
        (Regex::new(r#""EntityId":\s*"[0-9a-f-]*""#).unwrap(), 
         r#""EntityId": "00000000-0000-0000-0000-000000000000""#),
        (Regex::new(r#""Version":\s*[0-9]+,"#).unwrap(), 
         r#""Version": 0,"#),
        (Regex::new(r#""X":\s*[0-9]+"#).unwrap(), 
         r#""X": 0"#),
        (Regex::new(r#""Y":\s*[0-9]+"#).unwrap(), 
         r#""Y": 0"#),
    ]);

    // Используем Rayon для параллельной обработки файлов
    WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .par_bridge() // Превращаем в параллельный итератор
        .for_each(|entry| {
            let path = entry.path();
            let patterns = Arc::clone(&patterns);
            
            if let Err(e) = process_file(path, &*patterns) {
                eprintln!("Ошибка обработки файла {}: {}", path.display(), e);
            }
        });

    Ok(())
}

fn process_file(path: &Path, patterns: &[(Regex, &str)]) -> io::Result<()> {
    // Читаем файл
    let content = fs::read_to_string(path)?;
    
    // Применяем все замены
    let mut is_mdf = false;
    let  mut modified = content;
    for (regex, replacement) in patterns {
        let new_text = regex.replace_all(&modified, *replacement).to_string();
        if is_mdf == false && new_text != modified {
            is_mdf = true;
        };
        modified = new_text;
    }
    
    // Если были изменения, записываем файл
    if is_mdf == true {
        fs::write(path, modified)?;
    }
    
    Ok(())
}
