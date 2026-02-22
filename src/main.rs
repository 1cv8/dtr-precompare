use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

use std::env;
use std::process;

use serde_json::Value;

use std::collections::HashMap;

static ENTITY_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap());

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

    println!("Start processing Datareon files");

    // Step 1: Поиск файлов для обработки
    let files: Vec<PathBuf> = WalkDir::new(target_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.into_path())
        .collect();

    // Step 2: Сбор ID для замены
    let id_map: HashMap<String, String> = files
        .par_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(path).ok()?;

            let entity_id = ENTITY_ID_PATTERN
                .captures(&content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())?;

            let object_name = build_object_name(path).ok()?;

            Some((entity_id, object_name))
        })
        .collect();

    // Компилируем регулярные выражения один раз
    let patterns = Arc::new([
        (
            Regex::new(r#""FolderId":\s*"[0-9a-f-]*""#).unwrap(),
            r#""FolderId": "00000000-0000-0000-0000-000000000000""#,
        ),
        (
            Regex::new(r#""ClusterId":\s*"[0-9a-f-]*""#).unwrap(),
            r#""ClusterId": "00000000-0000-0000-0000-000000000000""#,
        ),
        (
            Regex::new(r#""EntityId":\s*"[0-9a-f-]*""#).unwrap(),
            r#""EntityId": "00000000-0000-0000-0000-000000000000""#,
        ),
        (
            Regex::new(r#""Version":\s*[0-9]+,"#).unwrap(),
            r#""Version": 0,"#,
        ),
        (Regex::new(r#""X":\s*[-0-9]+"#).unwrap(), r#""X": 0"#),
        (Regex::new(r#""Y":\s*[-0-9]+"#).unwrap(), r#""Y": 0"#),
        (
            Regex::new(r#""Key":\s*"[0-9a-f-]*""#).unwrap(),
            r#""Key": "00000000-0000-0000-0000-000000000000""#,
        ),
        (
            Regex::new(r#""Id":\s*"[0-9a-f-]*""#).unwrap(),
            r#""Id": "00000000-0000-0000-0000-000000000000""#,
        ),
    ]);

    // Step 2: Обработка файлов
    files.par_iter().for_each(|path| {
        let patterns = Arc::clone(&patterns);
        if let Err(e) = chage_file_content(path, &*patterns, &id_map) {
            eprintln!("Error processing {:?}: {}", path, e);
        };
    });

    println!(
        "All is Done за {:.3} секунд",
        start_time.elapsed().as_secs_f64()
    );
    Ok(())
}

fn chage_file_content(
    path: &Path,
    patterns: &[(Regex, &str)],
    id_map: &HashMap<String, String>,
) -> io::Result<()> {
    let content = fs::read_to_string(path)?;

    // Применяем все замены patterns
    let mut changed = false;
    let mut modified_content = content;

    for (regex, replacement) in patterns {
        let new_text = regex
            .replace_all(&modified_content, *replacement)
            .to_string();
        if !changed && new_text != modified_content {
            changed = true;
        };
        modified_content = new_text;
    }

    // JSON
    let mut json: Value = serde_json::from_str(&modified_content)?;
    if replace_ids(&mut json, id_map)? {
        changed = true;
        modified_content = serde_json::to_string_pretty(&json)?;
    };

    // Если были изменения, записываем файл
    if changed {
        fs::write(path, modified_content)?;
    };

    Ok(())
}

fn build_object_name(path: &Path) -> io::Result<String> {
    let file_stem = path.file_stem().and_then(|p| p.to_str()).unwrap_or("");

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

    Ok(format!("{}.{}", parent, file_stem))
}

fn replace_ids(json: &mut Value, id_map: &HashMap<String, String>) -> io::Result<bool> {
    let mut modified = false;
    
    // Step 1. RouteSystemDataTypes
    if let Some(arr) = json
        .get_mut("RouteSystemDataTypes")
        .and_then(|v| v.as_array_mut())
    {
        for elem in arr.iter_mut() {
            if let Some(old_id) = elem.as_str() {
                if let Some(new_id) = id_map.get(old_id) {
                    *elem = serde_json::Value::String(new_id.clone());
                    modified = true;
                };
            };
        };

        if modified {
            arr.sort_by(|a, b| {
                let a_id = a.as_str().unwrap_or("");
                let b_id = b.as_str().unwrap_or("");
                a_id.cmp(b_id)
            });
        };
    };

    // Step 2. Config.HandlersList
    if let Some(arr) = json
        .get_mut("Config")
        .and_then(|v| v.get_mut("HandlersList"))
        .and_then(|v| v.as_array_mut())
    {
        for handler in arr.iter_mut() {
            if let Some(handler_obj) = handler.as_object_mut() {
                if let Some(id_value) = handler_obj.get_mut("HandlerId") {
                    if let Some(id_str) = id_value.as_str() {
                        if let Some(name) = id_map.get(id_str) {
                            *id_value = serde_json::Value::String(name.clone());
                            modified = true;
                        };
                    };
                };
            };
        };

        if modified {
            arr.sort_by(|a, b| {
                let a_id = a.get("HandlerId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let b_id = b.get("HandlerId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                a_id.cmp(b_id)
            });
        };
    };

    // Step 3. DataToPlatform.SystemMetadataId
    if let Some(id_value) = json.get_mut("DataToPlatform").and_then(|v| v.get_mut("SystemMetadataId"))
    {
        if let Some(id_str) = id_value.as_str() {
            if let Some(name) = id_map.get(id_str) {
                *id_value = serde_json::Value::String(name.clone());
                modified = true;
            };
        };
    };

    // Step 4. DataFromPlatform.SystemMetadataId
    if let Some(id_value) = json.get_mut("DataFromPlatform").and_then(|v| v.get_mut("SystemMetadataId"))
    {
        if let Some(id_str) = id_value.as_str() {
            if let Some(name) = id_map.get(id_str) {
                *id_value = serde_json::Value::String(name.clone());
                modified = true;
            };
        };
    };

    Ok(modified)
}

