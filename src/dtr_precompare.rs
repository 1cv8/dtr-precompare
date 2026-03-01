use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use serde_json::Value;

use std::collections::HashMap;

static ENTITY_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap());
static SYSTEM_METADATA_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""SystemMetadataId":\s*"([0-9a-f-]*)""#).unwrap());
//static SOURCE_ID_PATTERN: Lazy<Regex> =
//    Lazy::new(|| Regex::new(r#""SourceId":\s*"([0-9a-f-]*)""#).unwrap());

struct ReplaceConfig {
    pub combined_re: Regex,
    pub patterns_reps: Vec<&'static str>,
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
    let patterns_reps: Vec<&'static str> = vec![
        r#""FolderId": "00000000-0000-0000-0000-000000000000""#,
        r#""ClusterId": "00000000-0000-0000-0000-000000000000""#,
        r#""EntityId": "00000000-0000-0000-0000-000000000000""#,
        r#""Version": 0,"#,
        r#""X": 0"#,
        r#""Y": 0"#,
        r#""Key": "00000000-0000-0000-0000-000000000000""#,
        r#""Id": "00000000-0000-0000-0000-000000000000""#,
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

    let (modified_content, changed) = change_content(&content, init_cfg, id_map)?;

    if changed {
        fs::write(path, modified_content)?;
    }

    Ok(())
}

fn change_content(
    content: &str,
    init_cfg: &ReplaceConfig,
    id_map: &HashMap<String, String>,
) -> io::Result<(String, bool)> {
    // Применяем все замены patterns
    let repls = &init_cfg.patterns_reps;
    let cow = init_cfg
        .combined_re
        .replace_all(content, |caps: &regex::Captures| {
            // группы: 1..=repls.len() (потому что p0..pN)
            for gi in 1..=repls.len() {
                if caps.get(gi).is_some() {
                    return repls[gi - 1];
                }
            }
            ""
        });

    // cow: Cow<str>
    let mut modified_content = cow.into_owned();
    let mut changed = modified_content != content;

    // SystemMetadataId
    let new_text2 = SYSTEM_METADATA_ID_PATTERN
        .replace_all(&modified_content, |caps: &regex::Captures| {
            let id_str: &str = &caps.get(1).map_or("", |m| m.as_str());
            if let Some(name) = id_map.get(id_str) {
                return format!(r#""SystemMetadataId": "{}""#, name);
            } else {
                return format!(r#""SystemMetadataId": "{}""#, id_str);
            };
        })
        .to_string();
    changed |= new_text2 != modified_content;
    modified_content = new_text2;

    // JSON
    let mut json: Value = serde_json::from_str(&modified_content)?;
    if replace_ids(&mut json, id_map)? {
        changed = true;
        modified_content = serde_json::to_string_pretty(&json)?;
    };

    Ok((modified_content, changed))
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
            arr.sort_unstable_by(|a, b| {
                let a_id = a.as_str().unwrap_or("");
                let b_id = b.as_str().unwrap_or("");
                a_id.cmp(b_id)
            });
        };
        //modified = modified || el_modified;
        modified |= el_modified;
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
            arr.sort_unstable_by(|a, b| {
                let a_id = a.get("HandlerId").and_then(|v| v.as_str()).unwrap_or("");

                let b_id = b.get("HandlerId").and_then(|v| v.as_str()).unwrap_or("");

                a_id.cmp(b_id)
            });
        };
        //modified = modified || el_modified;
        modified |= el_modified;
    };

    Ok(modified)
}

// tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn run_case(
        input: &str,
        expected: &str,
        id_map: HashMap<String, String>,
        expect_changed: bool,
    ) {
        let cfg = init_replace_config();
        let (out, changed) = change_content(input, &cfg, &id_map).unwrap();
        assert_eq!(
            out, expected,
            "Output mismatch.\nINPUT:\n{}\n\nOUT:\n{}\n\nEXPECTED:\n{}",
            input, out, expected
        );
        assert_eq!(
            changed, expect_changed,
            "Changed flag mismatch.\nINPUT:\n{}\nOUT:\n{}",
            input, out
        );
    }

    #[test]
    fn no_changes_returns_same_string_and_changed_false() {
        // Ни один regex не подходит, SystemMetadataId нет, JSON-узлы для replace_ids отсутствуют
        let input = r#"{"A": 1, "B": 1}"#;
        let expected = r#"{"A": 1, "B": 1}"#;

        run_case(input, expected, HashMap::new(), false);
    }

    #[test]
    fn pattern_folder_id_replaced() {
        // Step: patterns (FolderId)
        let input = r#"{"FolderId": "11111111-1111-1111-1111-111111111111", "A": 1}"#;
        let expected = r#"{"FolderId": "00000000-0000-0000-0000-000000000000", "A": 1}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn pattern_version_replaced() {
        // Step: patterns (Version: число + запятая)
        let input = r#"{"Version": 123, "Z": 1}"#;
        let expected = r#"{"Version": 0, "Z": 1}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn pattern_x_and_y_replaced() {
        // Step: patterns (X/Y могут быть отрицательные)
        let input = r#"{"X": -12, "Y": 34, "A": 1}"#;
        let expected = r#"{"X": 0, "Y": 0, "A": 1}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn pattern_key_replaced() {
        // Step: patterns (Key)
        let input = r#"{"Key": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "A": 1}"#;
        let expected = r#"{"Key": "00000000-0000-0000-0000-000000000000", "A": 1}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn pattern_id_replaced() {
        // Step: patterns (Id)
        let input = r#"{"Id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "A": 1}"#;
        let expected = r#"{"Id": "00000000-0000-0000-0000-000000000000", "A": 1}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn system_metadata_id_replaced_by_name_when_mapping_exists() {
        // Step: SystemMetadataId (ветка: найдено имя по id)
        let mut id_map = HashMap::new();
        id_map.insert("1111".to_string(), "My.Object".to_string());

        let input = r#"{"A": {"SystemMetadataId": "1111"}}"#;
        let expected = r#"{"A": {"SystemMetadataId": "My.Object"}}"#;

        run_case(input, expected, id_map, true);

        let mut id_map = HashMap::new();
        id_map.insert("1111".to_string(), "My.Object".to_string());

        let input = r#"{"B": {"A": {"SystemMetadataId": "1111"}}}"#;
        let expected = r#"{"B": {"A": {"SystemMetadataId": "My.Object"}}}"#;

        run_case(input, expected, id_map, true);
    }

    #[test]
    fn system_metadata_id_kept_when_mapping_missing_but_format_changes() {
        // Step: SystemMetadataId (ветка: НЕ найдено имя, но формат всё равно меняется)
        let input = r#"{"SystemMetadataId":"1111"}"#;
        let expected = r#"{"SystemMetadataId": "1111"}"#;

        run_case(input, expected, HashMap::new(), true);
    }

    #[test]
    fn json_route_system_data_types_replaced_and_sorted_and_prettified() {
        // Step: JSON replace_ids -> RouteSystemDataTypes (замена + сортировка)
        // Важно: при модификации JSON функция вернет to_string_pretty(...)
        let input = r#"{"RouteSystemDataTypes":["b","a"]}"#;
        let expected = r#"{
  "RouteSystemDataTypes": [
    "AName",
    "BName"
  ]
}"#;

        let mut id_map = HashMap::new();
        id_map.insert("a".to_string(), "AName".to_string());
        id_map.insert("b".to_string(), "BName".to_string());

        run_case(input, expected, id_map, true);
    }

    #[test]
    fn json_handlers_list_replaced_and_sorted_and_prettified() {
        // Step: JSON replace_ids -> Config.HandlersList (замена + сортировка)
        let input = r#"{"Config":{"HandlersList":[{"HandlerId":"b"},{"HandlerId":"a"}]}}"#;
        let expected = r#"{
  "Config": {
    "HandlersList": [
      {
        "HandlerId": "AName"
      },
      {
        "HandlerId": "BName"
      }
    ]
  }
}"#;

        let mut id_map = HashMap::new();
        id_map.insert("a".to_string(), "AName".to_string());
        id_map.insert("b".to_string(), "BName".to_string());

        run_case(input, expected, id_map, true);
    }

    #[test]
    fn json_present_but_no_replacements_means_no_pretty_and_changed_depends_on_previous_steps() {
        // JSON парсится всегда, но pretty-печать происходит только если replace_ids вернул true.
        // Здесь replace_ids не меняет ничего -> строка должна остаться как была.
        let input = r#"{"RouteSystemDataTypes":["x","y"]}"#;
        let expected = r#"{"RouteSystemDataTypes":["x","y"]}"#;

        // id_map не содержит x/y -> замены не будет
        let mut id_map = HashMap::new();
        id_map.insert("a".to_string(), "AName".to_string());

        run_case(input, expected, id_map, false);
    }
}
