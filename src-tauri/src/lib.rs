use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tauri::{Emitter, State};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CliArgs {
    images_dir: Option<String>,
    labels_dir: Option<String>,
}

fn parse_cli_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut result = CliArgs::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--images-dir" | "-i" => {
                if i + 1 < args.len() {
                    result.images_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--labels-dir" | "-l" => {
                if i + 1 < args.len() {
                    result.labels_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    result
}

#[tauri::command]
fn get_cli_args(cli_args: State<'_, CliArgs>) -> CliArgs {
    CliArgs {
        images_dir: cli_args.images_dir.clone(),
        labels_dir: cli_args.labels_dir.clone(),
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DatasetLabel {
    class_id: u32,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DatasetItem {
    name: String,
    image_src: String,
    labels: Vec<DatasetLabel>,
}

struct WatcherEntry {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

struct WatchState {
    watchers: HashMap<String, WatcherEntry>,
}

fn parse_yolo_line(line: &str) -> Option<DatasetLabel> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }
    let class_id = parts[0].parse::<u32>().ok()?;
    let x_center = parts[1].parse::<f64>().ok()?;
    let y_center = parts[2].parse::<f64>().ok()?;
    let width = parts[3].parse::<f64>().ok()?;
    let height = parts[4].parse::<f64>().ok()?;
    Some(DatasetLabel {
        class_id,
        left: x_center - width / 2.0,
        top: y_center - height / 2.0,
        width,
        height,
    })
}

fn load_labels_for_name(labels_dir: &Path, name: &str) -> Vec<DatasetLabel> {
    let label_path = labels_dir.join(format!("{}.txt", name));
    if !label_path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&label_path) {
        Ok(content) => content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    parse_yolo_line(trimmed)
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn convert_to_asset_src(file_path: &str) -> String {
    let normalized = file_path.replace('\\', "/");
    format!(
        "http://asset.localhost/{}",
        normalized.trim_start_matches('/')
    )
}

fn get_sorted_image_files(images_dir: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    let mut entries: Vec<std::fs::DirEntry> = fs::read_dir(images_dir)
        .map_err(|e| format!("Failed to read images directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

#[tauri::command]
fn get_dataset_count(images_dir: String) -> Result<usize, String> {
    let dir = Path::new(&images_dir);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", images_dir));
    }
    let entries = get_sorted_image_files(dir)?;
    Ok(entries.len())
}

#[tauri::command]
fn load_dataset_batch(
    images_dir: String,
    labels_dir: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<DatasetItem>, String> {
    let img_dir = Path::new(&images_dir);
    let lbl_dir = Path::new(&labels_dir);

    if !img_dir.exists() {
        return Err(format!("Directory does not exist: {}", images_dir));
    }
    if !lbl_dir.exists() {
        return Err(format!("Directory does not exist: {}", labels_dir));
    }

    let entries = get_sorted_image_files(img_dir)?;

    if offset >= entries.len() {
        return Ok(Vec::new());
    }

    let end = std::cmp::min(offset + limit, entries.len());
    let batch = &entries[offset..end];

    let items: Vec<DatasetItem> = batch
        .iter()
        .map(|entry| {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy().to_string();
            let name = file_name_str
                .rsplit_once('.')
                .map(|(n, _)| n.to_string())
                .unwrap_or(file_name_str.clone());

            let labels = load_labels_for_name(lbl_dir, &name);

            let full_path = entry.path().to_string_lossy().to_string();
            let image_src = convert_to_asset_src(&full_path);

            DatasetItem {
                name,
                image_src,
                labels,
            }
        })
        .collect();

    Ok(items)
}

#[tauri::command]
fn load_single_item(
    images_dir: String,
    labels_dir: String,
    name: String,
    cache_bust: bool,
) -> Result<Option<DatasetItem>, String> {
    let img_dir = Path::new(&images_dir);
    let lbl_dir = Path::new(&labels_dir);

    let entries = get_sorted_image_files(img_dir)?;
    let entry = entries.iter().find(|e| {
        let fname = e.file_name().to_string_lossy().to_string();
        let base = fname
            .rsplit_once('.')
            .map(|(n, _)| n.to_string())
            .unwrap_or(fname);
        base == name
    });

    let entry = match entry {
        Some(e) => e,
        None => return Ok(None),
    };

    let labels = load_labels_for_name(lbl_dir, &name);
    let full_path = entry.path().to_string_lossy().to_string();
    let mut image_src = convert_to_asset_src(&full_path);

    if cache_bust {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        image_src = format!("{}?t={}", image_src, ts);
    }

    Ok(Some(DatasetItem {
        name,
        image_src,
        labels,
    }))
}

#[tauri::command]
fn get_sorted_image_names(images_dir: String) -> Result<Vec<String>, String> {
    let dir = Path::new(&images_dir);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", images_dir));
    }
    let entries = get_sorted_image_files(dir)?;
    let names: Vec<String> = entries
        .iter()
        .map(|e| {
            let fname = e.file_name().to_string_lossy().to_string();
            fname
                .rsplit_once('.')
                .map(|(n, _)| n.to_string())
                .unwrap_or(fname)
        })
        .collect();
    Ok(names)
}

#[tauri::command]
fn resave_labels(
    labels_dir: String,
    name: String,
    labels: Vec<DatasetLabel>,
) -> Result<(), String> {
    let mut content = String::new();
    for label in &labels {
        let x_center = label.left + label.width / 2.0;
        let y_center = label.top + label.height / 2.0;
        content.push_str(&format!(
            "{} {:.6} {:.6} {:.6} {:.6}\n",
            label.class_id, x_center, y_center, label.width, label.height
        ));
    }
    let label_path = Path::new(&labels_dir).join(format!("{}.txt", name));
    fs::write(label_path, content).map_err(|e| format!("Failed to write label file: {}", e))
}

#[tauri::command]
fn delete_dataset_item(
    images_dir: String,
    labels_dir: String,
    name: String,
    image_ext: String,
) -> Result<(), String> {
    let img_path = Path::new(&images_dir).join(format!("{}.{}", name, image_ext));
    let lbl_path = Path::new(&labels_dir).join(format!("{}.txt", name));

    if img_path.exists() {
        fs::remove_file(img_path).map_err(|e| format!("Failed to delete image: {}", e))?;
    }
    if lbl_path.exists() {
        fs::remove_file(lbl_path).map_err(|e| format!("Failed to delete label: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_item_paths(
    images_dir: String,
    labels_dir: String,
    name: String,
    image_ext: String,
) -> Result<(String, String), String> {
    let img_path = Path::new(&images_dir)
        .join(format!("{}.{}", name, image_ext))
        .to_string_lossy()
        .to_string();
    let lbl_path = Path::new(&labels_dir)
        .join(format!("{}.txt", name))
        .to_string_lossy()
        .to_string();
    Ok((img_path, lbl_path))
}

#[tauri::command]
fn watch_directories(
    app: tauri::AppHandle,
    state: State<'_, Mutex<WatchState>>,
    watch_id: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let app_clone = app.clone();
    let wid = watch_id.clone();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.paths.is_empty() {
                    return;
                }
                let kind = match &event.kind {
                    notify::EventKind::Create(_) => "create",
                    notify::EventKind::Modify(_) => "modify",
                    notify::EventKind::Remove(_) => "remove",
                    notify::EventKind::Access(_) => return,
                    _ => "modify",
                };
                let paths: Vec<String> = event
                    .paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                let _ = app_clone.emit(
                    "dataset-fs-change",
                    serde_json::json!({
                        "watch_id": wid,
                        "paths": paths,
                        "kind": kind,
                    }),
                );
            }
        })
        .map_err(|e| e.to_string())?;

    for path in &paths {
        let p = Path::new(path);
        if p.exists() {
            watcher
                .watch(p, RecursiveMode::NonRecursive)
                .map_err(|e| e.to_string())?;
        }
    }

    let mut state = state.lock().map_err(|e| e.to_string())?;
    state.watchers.insert(watch_id, WatcherEntry { watcher });

    Ok(())
}

#[tauri::command]
fn unwatch_directories(
    state: State<'_, Mutex<WatchState>>,
    watch_id: String,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    state.watchers.remove(&watch_id);
    Ok(())
}

#[tauri::command]
fn list_video_files(dir: String) -> Result<Vec<String>, String> {
    let path = Path::new(&dir);
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", dir));
    }
    let video_exts: Vec<&str> = vec![
        "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "ts", "mpg", "mpeg",
    ];
    let entries: Vec<String> = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
            video_exts.contains(&ext.as_str())
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    Ok(entries)
}

#[tauri::command]
fn list_subdirs(dir: String) -> Result<Vec<String>, String> {
    let path = Path::new(&dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let entries: Vec<String> = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    Ok(entries)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli_args = parse_cli_args();
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(WatchState {
            watchers: HashMap::new(),
        }))
        .manage(cli_args)
        .invoke_handler(tauri::generate_handler![
            get_dataset_count,
            load_dataset_batch,
            load_single_item,
            get_sorted_image_names,
            resave_labels,
            delete_dataset_item,
            get_item_paths,
            watch_directories,
            unwatch_directories,
            list_video_files,
            list_subdirs,
            get_cli_args,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
