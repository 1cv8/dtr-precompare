
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use once_cell::sync::Lazy;
use regex::Regex;
use walkdir::WalkDir;
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::{Instant};

use std::env;
use std::process;

//static entity_id_pattern = Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap();
static ENTITY_ID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""EntityId":\s*"([0-9a-f-]+)""#).unwrap()
});

//let types_pattern = Regex::new(r#""RouteSystemDataTypes":\s*\[([0-9a-f-\",\r\n\t[:space:]]+)\]"#).unwrap();
static TYPES_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""RouteSystemDataTypes":\s*\[([0-9a-f-\",\r\n\t[:space:]]+)\]"#).unwrap()
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

    // Используем Rayon для параллельной обработки файлов
    let ids: HashMap<String, String> = 
    files.par_iter()
        .map(|path| {
            //let path = entry.path();
            let patterns = Arc::clone(&patterns);
            
            let res: (String, String) = match depersonalize_file(path, &*patterns) {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Ошибка обработки файла {}: {}", path.display(), e);
                    (String::from(""), String::from(""))
                },
            };
            return res
        })
        .collect();

    
    // Шаг 2 заменяем ID в теле на имена файлов
    files.par_iter()
        .for_each(|path| {
            if let Err(e) = change_ids_in_file(path, &ids) {
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

fn depersonalize_file(path: &Path, patterns: &[(Regex, &str)]) -> io::Result<(String, String)> {
    // Читаем файл
    let content = fs::read_to_string(path)?;
    
    // Ищем все FolderId до замены
    //
    let mut entity_id = String::new();
    let capt_rez = ENTITY_ID_PATTERN.captures(&content);
    match capt_rez {
        Some(capt_value) => {
            let capt_1_rez = capt_value.get(1); 
            match capt_1_rez {
                Some(capt_1_value) => {
                    entity_id = capt_1_value.as_str().to_string().clone();
                },
                None => {}
            };
        },
        None => {}
    };

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

    Ok((entity_id, dtr_obj_name(path).unwrap()))
}

fn dtr_obj_name(path: &Path) -> io::Result<String> {
    
    let f_name: String = path.file_name().unwrap().display().to_string();
    let f_name_wo_ext: String = path.file_stem().unwrap().display().to_string();
    assert_ne!(f_name, "", "empty filename!");
    
    let mut p_name: String = String::new();
    let parent = path.parent();
    if parent != None {
        p_name = parent.unwrap().file_name().unwrap().display().to_string();
        if p_name == f_name_wo_ext {
            let parent = parent.unwrap().parent();
            if parent != None {
                p_name = parent.unwrap().file_name().unwrap().display().to_string();
            };
        };
    };

    let rez;
    if p_name == "" {
        rez = f_name;
    }else {
        rez = format!("{}.{}", p_name, f_name_wo_ext);
    };
    
    Ok(rez)

}

fn change_ids_in_file(path: &Path, ids: &HashMap<String, String>) -> io::Result<()>{
    // Читаем файл
    let content = fs::read_to_string(path)?;

    //let types_pattern = Regex::new(r#""RouteSystemDataTypes":\s*\[([0-9a-f-\",\r\n\t[:space:]]+)\]"#).unwrap();

    let mut ch_names: Vec<String> = Vec::new();
    let orig_text: String;

    let capt_rez = TYPES_PATTERN.captures(&content);
    match capt_rez {
        Some(capt_value) => {
            let capt_1_rez = capt_value.get(1); 
            match capt_1_rez {
                Some(capt_1_value) => {
                    orig_text = capt_1_value.as_str().to_string();
                
                    for elem_id_text in orig_text.split(',') {
                        
                        let start = elem_id_text.find('"').unwrap();
                        let end = &elem_id_text[start + 1..].find('"').unwrap() + start + 1;
                        let elem_id = &elem_id_text[(start+1)..=(end-1)];

                        let el_name = ids.get(elem_id);
                        let rez_name;
                        if el_name == None {
                            rez_name = elem_id.to_string();
                        }else {
                            rez_name = el_name.unwrap().clone();
                        };
                        
                        ch_names.push(format!("{}{}{}",&elem_id_text[..=(start)], rez_name, "\""));
        
                    };
                    
                    // Datareon хранит в порядке добавления, для сравнения это вредно
                    ch_names.sort();

                    let change_text = ch_names.join(",") + "\r\n";
                    let new_content = content.replace(&orig_text, &change_text);
                    fs::write(path, new_content)?;

                    },
                None => {}
            };
        },
        None => {}
    };

    Ok(())
}
