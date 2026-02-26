use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
//use std::sync::Arc;
use walkdir::WalkDir;

use serde_json::Value;

use std::collections::HashMap;

static ENTITY_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap());
static SYSTEM_METADATA_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""SystemMetadataId":\s*"([0-9a-f-]*)""#).unwrap());

struct ReplaceConfig {
    pub combined_re: Regex,
    pub patterns_reps: Vec<String>,
}

fn init_replace_config() -> ReplaceConfig {
    let patterns_reg = [
        r#""FolderId":\s*"[0-9a-f-]*""#,
        r#""ClusterId":\s*"[0-9a-f-]*""#,
        r#""EntityId":\s*"[0-9a-f-]*""#,
        r#""Version":\s*[0-9]+,"#,
        r#""X":\s*[-0-9]+"#,
        r#""Y":\s*[-0-9]+"#,
        r#""Key":\s*"[0-9a-f-]*""#,
        r#""Id":\s*"[0-9a-f-]*""#,
    ];
    let patterns_reps = vec![
        r#""FolderId": "00000000-0000-0000-0000-000000000000""#.to_string(),
        r#""ClusterId": "00000000-0000-0000-0000-000000000000""#.to_string(),
        r#""EntityId": "00000000-0000-0000-0000-000000000000""#.to_string(),
        r#""Version": 0,"#.to_string(),
        r#""X": 0"#.to_string(),
        r#""Y": 0"#.to_string(),
        r#""Key": "00000000-0000-0000-0000-000000000000""#.to_string(),
        r#""Id": "00000000-0000-0000-0000-000000000000""#.to_string(),
    ];

    let combined_pattern: String = patterns_reg
        .iter()
        .enumerate()
        .map(|(i, re_str)| format!("(?P<p{}>{})", i, re_str))
        .collect::<Vec<_>>()
        .join("|");
    let combined_re = Regex::new(&combined_pattern).unwrap();

    ReplaceConfig {
        combined_re,
        patterns_reps,
    }
}

pub fn run(target_dir: String) -> io::Result<()> {
    println!("Start processing Datareon files");

    let init_cfg = init_replace_config();

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

    // Step 3: Обработка файлов
    files.par_iter().for_each(|path| {
        if let Err(e) = change_file_content(path, &init_cfg, &id_map) {
            eprintln!("Error processing {:?}: {}", path, e);
        };
    });

    Ok(())
}

fn change_file_content(
    path: &Path,
    init_cfg: &ReplaceConfig,
    id_map: &HashMap<String, String>,
) -> io::Result<()> {
    let content = fs::read_to_string(path)?;

    let mut changed = false;
    let mut modified_content = content;

    // Применяем все замены patterns
    let new_text = init_cfg
        .combined_re
        .replace_all(&modified_content, |caps: &regex::Captures| {
            for (i, rep_str) in init_cfg.patterns_reps.iter().enumerate() {
                let name = format!("p{}", i);
                if caps.name(&name).is_some() {
                    return rep_str.clone();
                }
            }
            String::new()
        })
        .to_string();
    if !changed && new_text != modified_content {
        changed = true;
    };
    modified_content = new_text;

    // SystemMetadataId
    let new_text2 = SYSTEM_METADATA_ID_PATTERN.replace_all(&modified_content, |caps: &regex::Captures| {
        let id_str: &str = &caps.get(1).map_or("", |m| m.as_str());
        if let Some(name) = id_map.get(id_str) {
            return format!(r#""SystemMetadataId": "{}""#, name);
        }else{
            return format!(r#""SystemMetadataId": "{}""#, id_str);
        };
    })
    .to_string();
    if !changed && new_text2 != modified_content {
        changed = true;
    };
    modified_content = new_text2;

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
        let mut el_modified = false;
        for elem in arr.iter_mut() {
            if let Some(old_id) = elem.as_str()
                && let Some(new_id) = id_map.get(old_id)
            {
                *elem = serde_json::Value::String(new_id.clone());
                el_modified = true;
            };
        }

        if el_modified {
            arr.sort_by(|a, b| {
                let a_id = a.as_str().unwrap_or("");
                let b_id = b.as_str().unwrap_or("");
                a_id.cmp(b_id)
            });
        };
        modified = modified || el_modified;
    };

    // Step 2. Config.HandlersList
    if let Some(arr) = json
        .get_mut("Config")
        .and_then(|v| v.get_mut("HandlersList"))
        .and_then(|v| v.as_array_mut())
    {
        let mut el_modified = false;
        for handler in arr.iter_mut() {
            if let Some(handler_obj) = handler.as_object_mut()
                && let Some(id_value) = handler_obj.get_mut("HandlerId")
                && let Some(id_str) = id_value.as_str()
                && let Some(name) = id_map.get(id_str)
            {
                *id_value = serde_json::Value::String(name.clone());
                el_modified = true;
            };
        }

        if el_modified {
            arr.sort_by(|a, b| {
                let a_id = a.get("HandlerId").and_then(|v| v.as_str()).unwrap_or("");

                let b_id = b.get("HandlerId").and_then(|v| v.as_str()).unwrap_or("");

                a_id.cmp(b_id)
            });
        };
        modified = modified || el_modified;
    };

    Ok(modified)
}
