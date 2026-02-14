
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use once_cell::sync::Lazy;
use regex::Regex;
use walkdir::WalkDir;
use rayon::prelude::*;
use std::time::{Instant};

use std::env;
use std::process;

use serde_json::Value;

use dashmap::DashMap;
//use crossbeam::queue::SegQueue;

type IdMap = DashMap<String, String>;


static ENTITY_ID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap()
});


fn main() -> io::Result<()> {
    let start_time = Instant::now();

    // Получаем аргументы командной строки
    let args: Vec<String> = env::args().collect();
    
    // Определяем целевой каталог
    let target_dir:String;
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
        target_dir = String::from(".");
        println!("processing in current dir");
    };

    println!("Start processing Datareon files");

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
        (Regex::new(r#""X":\s*[-0-9]+"#).unwrap(), 
         r#""X": 0"#),
        (Regex::new(r#""Y":\s*[-0-9]+"#).unwrap(), 
         r#""Y": 0"#),
    ]);

    let files = collect_json_files(&target_dir)?;

    let id_map = IdMap::new();

    // === Phase 1: Depersonalize + collect IDs ===
    files.par_iter()
        .for_each(|path| {
        let patterns = Arc::clone(&patterns);
        if let Err(e) = depersonalize_file(path, &*patterns, &id_map) {
            eprintln!("Error processing {:?}: {}", path, e);
        }
    });
    
    // Шаг 2 заменяем ID в теле на имена файлов
    files.par_iter()
        .for_each(|path| {
            if let Err(e) = replace_ids(path, &id_map) {
                eprintln!("Ошибка обработки файла {}: {}", path.display(), e);
            };
        });


    println!("All is Done за {:.3} секунд", start_time.elapsed().as_secs_f64());

    Ok(())
}

fn collect_json_files(dir: &str) -> io::Result<Vec<PathBuf>> {
    Ok(WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .map(|e| e.into_path())
        .collect())
}

fn depersonalize_file(path: &Path, patterns: &[(Regex, &str)], id_map: &IdMap) -> io::Result<()> {
    // Читаем файл
    let content = fs::read_to_string(path)?;
    
    // Ищем все EntityId до замены
    let capt_rez = ENTITY_ID_PATTERN.captures(&content);
    let entity_id:String = match capt_rez {
        Some(capt_value) => {
            let capt_1_rez = capt_value.get(1); 
            match capt_1_rez {
                Some(capt_1_value) => {
                    capt_1_value.as_str().to_string().clone()
                },
                None => {"".to_string()}
            }
        },
        None => {"".to_string()}
    };

    let new_name:String = build_object_name(path)?;

    id_map.insert(entity_id.clone(), new_name.clone());

    // Применяем все замены
    let mut is_mdf: bool = false;
    let  mut modified = content;

    for (regex, replacement) in patterns {
        let new_text = regex.replace_all(&modified, *replacement).to_string();
        if is_mdf == false && new_text != modified {
            is_mdf = true;
        };
        modified = new_text;
    };
    
    // Если были изменения, записываем файл
    if is_mdf == true {
        fs::write(path, modified)?;
    };

    Ok(())
}

fn build_object_name(path: &Path) -> io::Result<String> {
    let file_stem = path
        .file_stem()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|p| p.to_str())
        .unwrap_or("");

    let grandparent = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|p| p.to_str())
        .unwrap_or("");

    if file_stem == parent {
        return Ok(format!("{}.{}", grandparent, file_stem));
    };

    return Ok(format!("{}.{}", parent, file_stem))
}

fn replace_ids(path: &Path, id_map: &IdMap) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let mut json: Value = serde_json::from_str(&content)?;

    let mut modified = false;

    if let Some(arr) = json
        .get_mut("RouteSystemDataTypes")
        .and_then(|v| v.as_array_mut())
    {
        let mut ch_names = Vec::with_capacity(arr.len());

        for elem in arr.iter_mut() {
            if let Some(old_id) = elem.as_str() {
                if let Some(new_id) = id_map.get(old_id) {
                    ch_names.push(new_id.clone());
                    modified = true;
                } else {
                    ch_names.push(old_id.to_string());
                };
            };
        };

        if modified {
            ch_names.sort_unstable();

            *arr = ch_names
                .into_iter()
                .map(Value::String)
                .collect();
        };

    }

    if modified {

        let new_content = serde_json::to_string_pretty(&json)?;
        fs::write(path, &new_content)?;
    }

    Ok(())
}
