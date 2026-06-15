use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rayon::prelude::*;
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
            "--images-dir" | "--dataset-images-dir" | "-i" => {
                if i + 1 < args.len() {
                    result.images_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--labels-dir" | "--dataset-labels-dir" | "-l" => {
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
struct DatasetDir {
    images_dir: String,
    labels_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DatasetItem {
    name: String,
    image_src: String,
    labels: Vec<DatasetLabel>,
    has_label_file: bool,
    images_dir: String,
    labels_dir: String,
}

#[derive(Clone)]
struct PreparedDatasetEntry {
    name: String,
    base_name: String,
    image_src: String,
    images_dir: String,
    labels_dir: String,
}

struct DatasetLoadState {
    loads: HashMap<String, Vec<PreparedDatasetEntry>>,
}

struct StoredDatasetState {
    datasets: HashMap<String, Vec<DatasetItem>>,
}

struct WatcherEntry {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

struct WatchState {
    watchers: HashMap<String, WatcherEntry>,
}

fn parse_yolo_line(line: &str) -> Result<DatasetLabel, String> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 5 {
        return Err(format!(
            "Invalid format: expected 5 values, got {}",
            parts.len()
        ));
    }
    let class_id = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("Invalid classId: \"{}\" is not a number", parts[0]))?;
    let x_center = parts[1]
        .parse::<f64>()
        .map_err(|_| format!("Invalid xCenter: \"{}\" is not a number", parts[1]))?;
    let y_center = parts[2]
        .parse::<f64>()
        .map_err(|_| format!("Invalid yCenter: \"{}\" is not a number", parts[2]))?;
    let width = parts[3]
        .parse::<f64>()
        .map_err(|_| format!("Invalid width: \"{}\" is not a number", parts[3]))?;
    let height = parts[4]
        .parse::<f64>()
        .map_err(|_| format!("Invalid height: \"{}\" is not a number", parts[4]))?;
    Ok(DatasetLabel {
        class_id,
        left: x_center - width / 2.0,
        top: y_center - height / 2.0,
        width,
        height,
    })
}

fn load_labels_for_name(
    labels_dir: &Path,
    name: &str,
) -> Result<(Vec<DatasetLabel>, bool), String> {
    let label_path = labels_dir.join(format!("{}.txt", name));
    if !label_path.exists() {
        return Ok((Vec::new(), false));
    }
    let content = fs::read_to_string(&label_path)
        .map_err(|e| format!("Failed to read label file {}: {}", label_path.display(), e))?;
    let mut labels = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        labels.push(parse_yolo_line(trimmed).map_err(|reason| {
            format!(
                "Malformed label in file {} line {}: {}",
                label_path.display(),
                line_index + 1,
                reason
            )
        })?);
    }
    Ok((labels, true))
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

fn video_name_prefix(images_dir: &str) -> Option<String> {
    let parts: Vec<&str> = images_dir
        .split(|ch| ch == '/' || ch == '\\')
        .filter(|part| !part.is_empty())
        .collect();
    let videos_index = parts.iter().rposition(|part| *part == "videos")?;
    if videos_index + 1 >= parts.len().saturating_sub(1) {
        return None;
    }
    Some(parts[videos_index + 1..parts.len() - 1].join("/"))
}

fn dataset_item_from_entry(
    entry: &std::fs::DirEntry,
    images_dir: &str,
    labels_dir: &str,
    name_prefix: Option<&str>,
) -> Result<DatasetItem, String> {
    let file_name = entry.file_name();
    let file_name_str = file_name.to_string_lossy().to_string();
    let name = file_name_str
        .rsplit_once('.')
        .map(|(n, _)| n.to_string())
        .unwrap_or(file_name_str.clone());
    let full_name = name_prefix
        .map(|prefix| format!("{}/{}", prefix, name))
        .unwrap_or_else(|| name.clone());

    let (labels, has_label_file) = load_labels_for_name(Path::new(labels_dir), &name)?;
    let full_path = entry.path().to_string_lossy().to_string();
    let image_src = convert_to_asset_src(&full_path);

    Ok(DatasetItem {
        name: full_name,
        image_src,
        labels,
        has_label_file,
        images_dir: images_dir.to_string(),
        labels_dir: labels_dir.to_string(),
    })
}

fn prepared_entry_from_entry(
    entry: &std::fs::DirEntry,
    images_dir: &str,
    labels_dir: &str,
    name_prefix: Option<&str>,
) -> PreparedDatasetEntry {
    let file_name = entry.file_name();
    let file_name_str = file_name.to_string_lossy().to_string();
    let base_name = file_name_str
        .rsplit_once('.')
        .map(|(n, _)| n.to_string())
        .unwrap_or(file_name_str);
    let name = name_prefix
        .map(|prefix| format!("{}/{}", prefix, base_name))
        .unwrap_or_else(|| base_name.clone());
    let full_path = entry.path().to_string_lossy().to_string();

    PreparedDatasetEntry {
        name,
        base_name,
        image_src: convert_to_asset_src(&full_path),
        images_dir: images_dir.to_string(),
        labels_dir: labels_dir.to_string(),
    }
}

fn dataset_item_from_prepared_entry(entry: &PreparedDatasetEntry) -> Result<DatasetItem, String> {
    let (labels, has_label_file) =
        load_labels_for_name(Path::new(&entry.labels_dir), &entry.base_name)?;

    Ok(DatasetItem {
        name: entry.name.clone(),
        image_src: entry.image_src.clone(),
        labels,
        has_label_file,
        images_dir: entry.images_dir.clone(),
        labels_dir: entry.labels_dir.clone(),
    })
}

fn validate_dataset_dir(dir: &DatasetDir) -> Result<(), String> {
    let img_dir = Path::new(&dir.images_dir);
    let lbl_dir = Path::new(&dir.labels_dir);

    if !img_dir.exists() {
        return Err(format!("Directory does not exist: {}", dir.images_dir));
    }
    if !lbl_dir.exists() {
        return Err(format!("Directory does not exist: {}", dir.labels_dir));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterParams {
    mode: String,
    class_id: Option<u32>,
    nth: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilteredWindow {
    total_filtered: usize,
    items: Vec<DatasetItem>,
}

fn dataset_item_key(name: &str, images_dir: &str) -> String {
    format!("{}\0{}", images_dir, name)
}

fn item_base_name(name: &str) -> &str {
    name.rsplit_once('/').map(|(_, n)| n).unwrap_or(name)
}

fn has_nested_boxes(labels: &[DatasetLabel]) -> bool {
    for i in 0..labels.len() {
        for j in 0..labels.len() {
            if i == j {
                continue;
            }
            let inner = &labels[i];
            let outer = &labels[j];
            if inner.left >= outer.left
                && inner.top >= outer.top
                && inner.left + inner.width <= outer.left + outer.width
                && inner.top + inner.height <= outer.top + outer.height
            {
                return true;
            }
        }
    }
    false
}

fn matches_filter(item: &DatasetItem, index: usize, filter: &FilterParams) -> bool {
    match filter.mode.as_str() {
        "all" => true,
        "hasBoxes" => !item.labels.is_empty(),
        "noBoxes" => item.labels.is_empty(),
        "hasLabelFile" => item.has_label_file,
        "class" => filter
            .class_id
            .map_or(false, |cid| item.labels.iter().any(|l| l.class_id == cid)),
        "nth" => filter.nth.map_or(false, |n| n >= 1 && index % (n as usize) == 0),
        "nestedBoxes" => has_nested_boxes(&item.labels),
        _ => true,
    }
}

#[tauri::command]
fn prepare_dataset_load(
    state: State<'_, Mutex<DatasetLoadState>>,
    load_id: String,
    dirs: Vec<DatasetDir>,
) -> Result<usize, String> {
    let mut prepared_entries = Vec::new();

    for dir in dirs {
        validate_dataset_dir(&dir)?;
        let entries = get_sorted_image_files(Path::new(&dir.images_dir))?;
        let prefix = video_name_prefix(&dir.images_dir);

        for entry in entries {
            prepared_entries.push(prepared_entry_from_entry(
                &entry,
                &dir.images_dir,
                &dir.labels_dir,
                prefix.as_deref(),
            ));
        }
    }

    let total = prepared_entries.len();
    let mut state = state.lock().map_err(|e| e.to_string())?;
    state.loads.insert(load_id, prepared_entries);
    Ok(total)
}

#[tauri::command]
fn load_prepared_dataset_batch(
    state: State<'_, Mutex<DatasetLoadState>>,
    load_id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<DatasetItem>, String> {
    let batch_entries = {
        let state = state.lock().map_err(|e| e.to_string())?;
        let entries = state
            .loads
            .get(&load_id)
            .ok_or_else(|| format!("Prepared dataset load not found: {}", load_id))?;

        if offset >= entries.len() {
            return Ok(Vec::new());
        }

        let end = std::cmp::min(offset + limit, entries.len());
        entries[offset..end].to_vec()
    };

    batch_entries
        .par_iter()
        .map(dataset_item_from_prepared_entry)
        .collect()
}

#[tauri::command]
fn clear_prepared_dataset_load(
    state: State<'_, Mutex<DatasetLoadState>>,
    load_id: String,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    state.loads.remove(&load_id);
    Ok(())
}

#[tauri::command]
fn load_and_store_batch(
    load_state: State<'_, Mutex<DatasetLoadState>>,
    store_state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
    offset: usize,
    limit: usize,
) -> Result<usize, String> {
    let batch_entries = {
        let state = load_state.lock().map_err(|e| e.to_string())?;
        let entries = state
            .loads
            .get(&load_id)
            .ok_or_else(|| format!("Prepared dataset load not found: {}", load_id))?;

        if offset >= entries.len() {
            return Ok(0);
        }

        let end = std::cmp::min(offset + limit, entries.len());
        entries[offset..end].to_vec()
    };

    let items: Vec<DatasetItem> = batch_entries
        .par_iter()
        .map(dataset_item_from_prepared_entry)
        .collect::<Result<Vec<_>, _>>()?;

    let count = items.len();

    let mut store = store_state.lock().map_err(|e| e.to_string())?;
    store.datasets.entry(load_id).or_default().extend(items);

    Ok(count)
}

#[tauri::command]
fn get_filtered_window(
    state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
    filter: FilterParams,
    offset: usize,
    limit: usize,
) -> Result<FilteredWindow, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let items = state_guard
        .datasets
        .get(&load_id)
        .ok_or_else(|| format!("Stored dataset not found: {}", load_id))?;

    let matching_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(index, item)| matches_filter(item, *index, &filter))
        .map(|(index, _)| index)
        .collect();

    let total_filtered = matching_indices.len();
    let start = offset.min(total_filtered);
    let end = (offset + limit).min(total_filtered);

    let window_items: Vec<DatasetItem> = matching_indices[start..end]
        .iter()
        .map(|&i| items[i].clone())
        .collect();

    Ok(FilteredWindow {
        total_filtered,
        items: window_items,
    })
}

#[tauri::command]
fn update_stored_item(
    state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
    item: DatasetItem,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    let items = state
        .datasets
        .get_mut(&load_id)
        .ok_or_else(|| format!("Stored dataset not found: {}", load_id))?;

    let key = dataset_item_key(&item.name, &item.images_dir);
    if let Some(stored) = items
        .iter_mut()
        .find(|i| dataset_item_key(&i.name, &i.images_dir) == key)
    {
        *stored = item;
    }

    Ok(())
}

#[tauri::command]
fn remove_stored_item(
    state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
    name: String,
    images_dir: String,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    let items = state
        .datasets
        .get_mut(&load_id)
        .ok_or_else(|| format!("Stored dataset not found: {}", load_id))?;

    let key = dataset_item_key(&name, &images_dir);
    let before = items.len();
    items.retain(|i| dataset_item_key(&i.name, &i.images_dir) != key);
    if items.len() == before {
        return Err(format!("Item not found: {}", name));
    }

    Ok(())
}

#[tauri::command]
fn resave_all_labels(
    state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
) -> Result<usize, String> {
    let items = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state
            .datasets
            .get(&load_id)
            .ok_or_else(|| format!("Stored dataset not found: {}", load_id))?
            .clone()
    };

    let count = items
        .par_iter()
        .filter(|item| item.has_label_file || !item.labels.is_empty())
        .map(|item| {
            let mut content = String::new();
            for label in &item.labels {
                let x_center = label.left + label.width / 2.0;
                let y_center = label.top + label.height / 2.0;
                content.push_str(&format!(
                    "{} {:.6} {:.6} {:.6} {:.6}\n",
                    label.class_id, x_center, y_center, label.width, label.height
                ));
            }
            let base = item_base_name(&item.name);
            let label_path = Path::new(&item.labels_dir).join(format!("{}.txt", base));
            fs::write(label_path, content)
        })
        .filter(|r| r.is_ok())
        .count();

    Ok(count)
}

#[tauri::command]
fn clear_stored_dataset(
    state: State<'_, Mutex<StoredDatasetState>>,
    load_id: String,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    state.datasets.remove(&load_id);
    Ok(())
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

    let prefix = video_name_prefix(&images_dir);
    batch
        .iter()
        .map(|entry| dataset_item_from_entry(entry, &images_dir, &labels_dir, prefix.as_deref()))
        .collect()
}

#[tauri::command]
fn load_single_item(
    images_dir: String,
    labels_dir: String,
    name: String,
    cache_bust: bool,
) -> Result<Option<DatasetItem>, String> {
    let img_dir = Path::new(&images_dir);

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

    let (labels, has_label_file) = load_labels_for_name(Path::new(&labels_dir), &name)?;
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
        has_label_file,
        images_dir,
        labels_dir,
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

#[tauri::command]
fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("dbus-send")
            .args([
                "--session",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                &format!("array:string:file://{}", path),
                "string:",
            ])
            .spawn()
            .or_else(|_| {
                std::process::Command::new("xdg-open")
                    .arg(p.parent().unwrap_or(p))
                    .spawn()
            })
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn frontend_log(message: String) {
    println!("{}", message);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli_args = parse_cli_args();
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(WatchState {
            watchers: HashMap::new(),
        }))
        .manage(Mutex::new(DatasetLoadState {
            loads: HashMap::new(),
        }))
        .manage(Mutex::new(StoredDatasetState {
            datasets: HashMap::new(),
        }))
        .manage(cli_args)
        .invoke_handler(tauri::generate_handler![
            prepare_dataset_load,
            load_prepared_dataset_batch,
            clear_prepared_dataset_load,
            load_and_store_batch,
            get_filtered_window,
            update_stored_item,
            remove_stored_item,
            resave_all_labels,
            clear_stored_dataset,
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
            reveal_in_file_manager,
            get_cli_args,
            frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
