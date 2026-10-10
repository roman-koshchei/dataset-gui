use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CliArgs {
    images_dir: Option<String>,
    labels_dir: Option<String>,
    dataset_kit_dir: Option<String>,
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
            "--dataset-kit-dir" => {
                if i + 1 < args.len() {
                    result.dataset_kit_dir = Some(args[i + 1].clone());
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
        dataset_kit_dir: cli_args.dataset_kit_dir.clone(),
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

struct ProcessJob {
    child: Arc<Mutex<Child>>,
}

struct JobRegistry {
    jobs: HashMap<String, ProcessJob>,
}

struct AnnotateServer {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
    kit_dir: String,
    running_job: Option<String>,
}

struct AnnotateServerState {
    server: Option<AnnotateServer>,
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
async fn prepare_dataset_load(
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
async fn load_prepared_dataset_batch(
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
async fn load_and_store_batch(
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
async fn get_filtered_window(
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
async fn resave_all_labels(
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
async fn get_dataset_count(images_dir: String) -> Result<usize, String> {
    let dir = Path::new(&images_dir);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", images_dir));
    }
    let entries = get_sorted_image_files(dir)?;
    Ok(entries.len())
}

#[tauri::command]
async fn load_dataset_batch(
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
async fn load_single_item(
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
async fn get_sorted_image_names(images_dir: String) -> Result<Vec<String>, String> {
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
async fn get_last_frame_path(images_dir: String) -> Result<Option<String>, String> {
    let dir = Path::new(&images_dir);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", images_dir));
    }
    let entries = get_sorted_image_files(dir)?;
    Ok(entries
        .last()
        .map(|entry| entry.path().to_string_lossy().to_string()))
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
async fn list_video_files(dir: String) -> Result<Vec<String>, String> {
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
async fn list_subdirs(dir: String) -> Result<Vec<String>, String> {
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

fn is_dataset_kit_dir(path: &Path) -> bool {
    path.join("main.py").is_file() && path.join("auto_annotate.py").is_file()
}

fn resolve_dataset_kit_dir(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(dir) = explicit.map(str::trim).filter(|dir| !dir.is_empty()) {
        let candidate = PathBuf::from(dir);
        if is_dataset_kit_dir(&candidate) {
            return Ok(candidate);
        }
        return Err(format!(
            "dataset-kit directory is invalid (expected main.py): {}",
            candidate.display()
        ));
    }

    if let Ok(env_dir) = std::env::var("DATASET_KIT_DIR") {
        let candidate = PathBuf::from(env_dir.trim());
        if is_dataset_kit_dir(&candidate) {
            return Ok(candidate);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("dataset-kit"));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join("dataset-kit"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            candidates.push(ancestor.join("dataset-kit"));
            if let Some(parent) = ancestor.parent() {
                candidates.push(parent.join("dataset-kit"));
            }
        }
    }

    for candidate in candidates {
        if is_dataset_kit_dir(&candidate) {
            return Ok(candidate);
        }
    }

    Err(
        "Could not locate dataset-kit. Pass --dataset-kit-dir or set DATASET_KIT_DIR."
            .to_string(),
    )
}

fn python_invocation(kit_dir: &Path) -> Command {
    #[cfg(windows)]
    let venv_python = kit_dir.join(".venv").join("Scripts").join("python.exe");
    #[cfg(not(windows))]
    let venv_python = kit_dir.join(".venv").join("bin").join("python");

    if venv_python.is_file() {
        return Command::new(venv_python);
    }

    let mut command = Command::new("uv");
    command.arg("run").arg("python");
    command
}

fn spawn_job_reader<R: std::io::Read + Send + 'static>(
    app: tauri::AppHandle,
    video_id: String,
    event_name: String,
    stream: &'static str,
    reader: R,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines() {
            let Ok(line) = line else { break };
            let _ = app.emit(
                event_name.as_str(),
                serde_json::json!({
                    "videoId": video_id,
                    "stream": stream,
                    "line": line,
                }),
            );
        }
    })
}

fn run_background_process(
    app: tauri::AppHandle,
    registry: &Mutex<JobRegistry>,
    key: String,
    event_base: &'static str,
    mut command: Command,
) -> Result<(), String> {
    {
        let state = registry.lock().map_err(|e| e.to_string())?;
        if state.jobs.contains_key(&key) {
            return Err(format!(
                "A dataset-kit job is already running for {}",
                key
            ));
        }
    }

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start dataset-kit via uv: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));

    {
        let mut state = registry.lock().map_err(|e| e.to_string())?;
        state.jobs.insert(
            key.clone(),
            ProcessJob {
                child: Arc::clone(&child),
            },
        );
    }

    let app_handle = app.clone();
    let job_key = key.clone();
    let progress_event = format!("{}-progress", event_base);
    let done_event = format!("{}-done", event_base);
    let error_event = format!("{}-error", event_base);

    std::thread::spawn(move || {
        let stdout_handle = stdout.map(|out| {
            spawn_job_reader(
                app_handle.clone(),
                job_key.clone(),
                progress_event.clone(),
                "stdout",
                out,
            )
        });
        let stderr_handle = stderr.map(|err| {
            spawn_job_reader(
                app_handle.clone(),
                job_key.clone(),
                progress_event.clone(),
                "stderr",
                err,
            )
        });
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        let wait_result = {
            let mut guard = child.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.wait()
        };

        if let Some(state) = app_handle.try_state::<Mutex<JobRegistry>>() {
            if let Ok(mut state) = state.lock() {
                state.jobs.remove(&job_key);
            }
        }

        match wait_result {
            Ok(status) if status.success() => {
                let _ = app_handle.emit(
                    done_event.as_str(),
                    serde_json::json!({
                        "videoId": job_key,
                        "success": true,
                        "code": status.code(),
                    }),
                );
            }
            Ok(status) => {
                let _ = app_handle.emit(
                    done_event.as_str(),
                    serde_json::json!({
                        "videoId": job_key,
                        "success": false,
                        "code": status.code(),
                    }),
                );
            }
            Err(err) => {
                let _ = app_handle.emit(
                    error_event.as_str(),
                    serde_json::json!({
                        "videoId": job_key,
                        "error": err.to_string(),
                    }),
                );
            }
        }
    });

    Ok(())
}

fn cancel_job(registry: &Mutex<JobRegistry>, key: &str) -> Result<(), String> {
    let child = {
        let state = registry.lock().map_err(|e| e.to_string())?;
        state.jobs.get(key).map(|job| Arc::clone(&job.child))
    };
    if let Some(child) = child {
        let mut guard = child.lock().map_err(|e| e.to_string())?;
        guard
            .kill()
            .map_err(|e| format!("Failed to cancel job: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn resolve_dataset_kit(dataset_kit_dir: Option<String>) -> Result<String, String> {
    resolve_dataset_kit_dir(dataset_kit_dir.as_deref())
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn start_video_split(
    app: tauri::AppHandle,
    state: State<'_, Mutex<JobRegistry>>,
    data_json: String,
    project_dir: String,
    videos_dir: Option<String>,
    video_id: String,
    dataset_kit_dir: Option<String>,
) -> Result<(), String> {
    let video_id = video_id.trim().to_string();
    if video_id.is_empty() {
        return Err("videoId is required".to_string());
    }

    let kit_dir = resolve_dataset_kit_dir(dataset_kit_dir.as_deref())?;
    if !Path::new(&data_json).is_file() {
        return Err(format!("data.json not found: {}", data_json));
    }

    let mut command = python_invocation(&kit_dir);
    command
        .arg("-u")
        .arg("main.py")
        .arg("split-video-segments")
        .arg("--data-json")
        .arg(&data_json)
        .arg("--project-dir")
        .arg(&project_dir)
        .arg("--video-id")
        .arg(&video_id)
        .current_dir(&kit_dir);

    if let Some(videos_dir) = videos_dir
        .as_deref()
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
    {
        command.arg("--videos-dir").arg(videos_dir);
    }

    run_background_process(
        app,
        state.inner(),
        video_id,
        "video-split",
        command,
    )
}

#[tauri::command]
fn cancel_video_split(state: State<'_, Mutex<JobRegistry>>, video_id: String) -> Result<(), String> {
    cancel_job(state.inner(), &video_id)
}

fn current_annotate_job(app: &tauri::AppHandle) -> Option<String> {
    let state = app.try_state::<Mutex<AnnotateServerState>>()?;
    let guard = state.lock().ok()?;
    guard
        .server
        .as_ref()
        .and_then(|server| server.running_job.clone())
}

fn finish_annotate_job(app: &tauri::AppHandle, error: Option<String>) {
    let video_id = {
        let Some(state) = app.try_state::<Mutex<AnnotateServerState>>() else {
            return;
        };
        let Ok(mut guard) = state.lock() else {
            return;
        };
        guard
            .server
            .as_mut()
            .and_then(|server| server.running_job.take())
    };
    let Some(video_id) = video_id else {
        return;
    };
    match error {
        Some(message) => {
            let _ = app.emit(
                "video-annotate-error",
                serde_json::json!({ "videoId": video_id, "error": message }),
            );
        }
        None => {
            let _ = app.emit(
                "video-annotate-done",
                serde_json::json!({ "videoId": video_id, "success": true, "code": 0 }),
            );
        }
    }
}

fn spawn_annotate_stdout_reader<R: std::io::Read + Send + 'static>(
    app: tauri::AppHandle,
    reader: R,
) {
    std::thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines() {
            let Ok(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };
            if value.get("type").and_then(|kind| kind.as_str()) != Some("result") {
                continue;
            }
            let ok = value.get("ok").and_then(|flag| flag.as_bool()).unwrap_or(false);
            if ok {
                finish_annotate_job(&app, None);
            } else {
                let message = value
                    .get("error")
                    .and_then(|err| err.as_str())
                    .unwrap_or("annotation failed")
                    .to_string();
                finish_annotate_job(&app, Some(message));
            }
        }
        finish_annotate_job(&app, Some("annotation server stopped".to_string()));
        if let Some(state) = app.try_state::<Mutex<AnnotateServerState>>() {
            if let Ok(mut guard) = state.lock() {
                guard.server = None;
            }
        }
    });
}

fn spawn_annotate_stderr_reader<R: std::io::Read + Send + 'static>(
    app: tauri::AppHandle,
    reader: R,
) {
    std::thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines() {
            let Ok(line) = line else { break };
            if let Some(video_id) = current_annotate_job(&app) {
                let _ = app.emit(
                    "video-annotate-progress",
                    serde_json::json!({
                        "videoId": video_id,
                        "stream": "stderr",
                        "line": line,
                    }),
                );
            }
        }
    });
}

fn annotate_server_alive(server: &AnnotateServer) -> bool {
    server
        .child
        .lock()
        .map(|mut child| matches!(child.try_wait(), Ok(None)))
        .unwrap_or(false)
}

fn ensure_annotate_server(
    app: &tauri::AppHandle,
    state: &mut AnnotateServerState,
    kit_dir: &Path,
) -> Result<(), String> {
    let kit_dir_str = kit_dir.to_string_lossy().to_string();
    let reusable = state
        .server
        .as_ref()
        .map(|server| server.kit_dir == kit_dir_str && annotate_server_alive(server))
        .unwrap_or(false);
    if reusable {
        return Ok(());
    }

    if let Some(previous) = state.server.take() {
        if let Ok(mut child) = previous.child.lock() {
            let _ = child.kill();
        }
    }

    let mut command = python_invocation(kit_dir);
    command
        .arg("-u")
        .arg("annotate_server.py")
        .current_dir(kit_dir);

    #[cfg(windows)]
    {
        // DETACHED_PROCESS keeps the worker console-less without CREATE_NO_WINDOW,
        // which deadlocks the scipy/transformers import inside the worker.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(DETACHED_PROCESS);
    }

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start annotation server: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to capture annotation server stdin".to_string())?;

    if let Some(out) = stdout {
        spawn_annotate_stdout_reader(app.clone(), out);
    }
    if let Some(err) = stderr {
        spawn_annotate_stderr_reader(app.clone(), err);
    }

    state.server = Some(AnnotateServer {
        child: Arc::new(Mutex::new(child)),
        stdin: Arc::new(Mutex::new(stdin)),
        kit_dir: kit_dir_str,
        running_job: None,
    });
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn start_video_annotate(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AnnotateServerState>>,
    data_json: String,
    project_dir: String,
    videos_dir: Option<String>,
    video_id: String,
    classes: Vec<String>,
    class_id: Option<i64>,
    every_n: Option<i64>,
    write_all: Option<bool>,
    moondream_model: Option<String>,
    dataset_kit_dir: Option<String>,
    use_tracking: Option<bool>,
    score_threshold: Option<f64>,
    track_device: Option<String>,
    lightfc_onnx_dir: Option<String>,
    preview: Option<bool>,
    segment: Option<String>,
    labels_subdir: Option<String>,
    init_bboxes: Option<Vec<Vec<f64>>>,
) -> Result<(), String> {
    let video_id = video_id.trim().to_string();
    if video_id.is_empty() {
        return Err("videoId is required".to_string());
    }

    let classes: Vec<String> = classes
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    let manual_init = init_bboxes
        .as_ref()
        .map(|boxes| !boxes.is_empty())
        .unwrap_or(false);
    if classes.is_empty() && !manual_init {
        return Err("At least one class name is required".to_string());
    }

    let kit_dir = resolve_dataset_kit_dir(dataset_kit_dir.as_deref())?;
    if !Path::new(&data_json).is_file() {
        return Err(format!("data.json not found: {}", data_json));
    }

    let trimmed = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
    };

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard
        .server
        .as_ref()
        .map(|server| server.running_job.is_some())
        .unwrap_or(false)
    {
        return Err("Another auto-annotation is already running".to_string());
    }

    ensure_annotate_server(&app, &mut guard, &kit_dir)?;

    let request = serde_json::json!({
        "id": video_id,
        "action": "annotate",
        "data_json": data_json,
        "project_dir": project_dir,
        "videos_dir": trimmed(&videos_dir),
        "video_ids": [video_id.clone()],
        "classes": classes,
        "class_id": class_id.unwrap_or(0),
        "every_n": every_n.unwrap_or(30).max(1),
        "write_all": write_all.unwrap_or(false),
        "moondream_model": trimmed(&moondream_model).unwrap_or_else(|| "moondream2".to_string()),
        "use_tracking": use_tracking.unwrap_or(false),
        "score_threshold": score_threshold.unwrap_or(0.0).max(0.0),
        "track_device": trimmed(&track_device),
        "lightfc_onnx_dir": trimmed(&lightfc_onnx_dir),
        "preview": preview.unwrap_or(false),
        "segment": trimmed(&segment),
        "labels_subdir": trimmed(&labels_subdir).unwrap_or_else(|| "labels".to_string()),
        "init_bboxes": init_bboxes,
        "overwrite": false,
    });

    let server = guard
        .server
        .as_mut()
        .ok_or_else(|| "Annotation server unavailable".to_string())?;
    server.running_job = Some(video_id.clone());

    let serialized = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    let write_result = {
        let mut stdin = server.stdin.lock().map_err(|e| e.to_string())?;
        writeln!(stdin, "{}", serialized).and_then(|_| stdin.flush())
    };
    if let Err(err) = write_result {
        server.running_job = None;
        return Err(format!("Failed to send annotation request: {}", err));
    }
    Ok(())
}

#[tauri::command]
fn cancel_video_annotate(
    state: State<'_, Mutex<AnnotateServerState>>,
    video_id: String,
) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let Some(server) = guard.server.as_ref() else {
        return Ok(());
    };
    if server.running_job.as_deref() != Some(video_id.as_str()) {
        return Ok(());
    }

    let payload = serde_json::json!({ "action": "cancel", "id": video_id });
    let serialized = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let mut stdin = server.stdin.lock().map_err(|e| e.to_string())?;
    writeln!(stdin, "{}", serialized)
        .and_then(|_| stdin.flush())
        .map_err(|e| format!("Failed to send cancel request: {}", e))?;
    Ok(())
}

const DEFAULT_PREVIEW_SUBDIR: &str = ".preview_labels";

fn read_processed_files(segment_dir: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(segment_dir.join("processed.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };
    value
        .get("files")
        .and_then(|files| files.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
fn commit_segment_preview(
    segment_dir: String,
    preview_subdir: Option<String>,
    labels_subdir: Option<String>,
) -> Result<usize, String> {
    let segment_path = Path::new(&segment_dir);
    let preview_subdir = preview_subdir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PREVIEW_SUBDIR);
    let labels_subdir = labels_subdir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("labels");

    let preview_dir = segment_path.join(preview_subdir);
    if !preview_dir.is_dir() {
        return Err(format!(
            "Preview directory not found: {}",
            preview_dir.display()
        ));
    }

    let labels_dir = segment_path.join(labels_subdir);
    fs::create_dir_all(&labels_dir)
        .map_err(|e| format!("Failed to create labels directory: {}", e))?;

    let frames_dir = segment_path.join("frames");
    let mut frame_by_stem: HashMap<String, String> = HashMap::new();
    if let Ok(entries) = fs::read_dir(&frames_dir) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            if !entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some((stem, _ext)) = name.rsplit_once('.') {
                frame_by_stem.insert(
                    stem.to_string(),
                    entry.path().to_string_lossy().to_string(),
                );
            }
        }
    }

    let mut copied = 0usize;
    let mut committed_frames: Vec<String> = Vec::new();
    let entries =
        fs::read_dir(&preview_dir).map_err(|e| format!("Failed to read preview dir: {}", e))?;
    for entry in entries.filter_map(|entry| entry.ok()) {
        if !entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".txt") {
            continue;
        }
        let stem = name.trim_end_matches(".txt");
        let destination = labels_dir.join(&name);
        fs::copy(entry.path(), &destination)
            .map_err(|e| format!("Failed to copy {}: {}", name, e))?;
        if let Some(frame_path) = frame_by_stem.get(stem) {
            committed_frames.push(frame_path.clone());
        }
        copied += 1;
    }

    let mut files = read_processed_files(segment_path);
    for frame_path in committed_frames {
        if !files.contains(&frame_path) {
            files.push(frame_path);
        }
    }
    let payload = serde_json::json!({ "files": files });
    fs::write(
        segment_path.join("processed.json"),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{\"files\":[]}".to_string()),
    )
    .map_err(|e| format!("Failed to write processed.json: {}", e))?;

    Ok(copied)
}

#[tauri::command]
fn discard_segment_preview(
    segment_dir: String,
    preview_subdir: Option<String>,
) -> Result<(), String> {
    let preview_subdir = preview_subdir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PREVIEW_SUBDIR);
    let preview_dir = Path::new(&segment_dir).join(preview_subdir);
    if preview_dir.is_dir() {
        fs::remove_dir_all(&preview_dir)
            .map_err(|e| format!("Failed to remove preview directory: {}", e))?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AutoAnnotateStatus {
    status: String,
    total_frames: usize,
    processed_frames: usize,
}

fn image_file_count(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_file()).unwrap_or(false))
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            matches!(
                name.rsplit('.').next(),
                Some("png" | "jpg" | "jpeg" | "bmp" | "webp" | "tif" | "tiff" | "gif")
            )
        })
        .count()
}

fn processed_frame_count(segment_dir: &Path) -> usize {
    let Ok(content) = fs::read_to_string(segment_dir.join("processed.json")) else {
        return 0;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return 0;
    };
    value
        .get("files")
        .and_then(|files| files.as_array())
        .map(|files| files.len())
        .unwrap_or(0)
}

fn expected_frame_count(total: usize, every_n: usize) -> usize {
    if total == 0 {
        return 0;
    }
    if every_n <= 1 {
        return total;
    }
    if total < every_n {
        return 1;
    }
    (total - every_n) / every_n + 1
}

fn scan_annotate_status(video_dir: &Path, every_n: usize) -> Option<AutoAnnotateStatus> {
    let entries = fs::read_dir(video_dir).ok()?;
    let mut total_frames = 0usize;
    let mut processed_frames = 0usize;
    let mut expected_frames = 0usize;

    for entry in entries.filter_map(|entry| entry.ok()) {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let segment_dir = entry.path();
        let frames_dir = segment_dir.join("frames");
        if !frames_dir.is_dir() {
            continue;
        }
        let total = image_file_count(&frames_dir);
        if total == 0 {
            continue;
        }
        total_frames += total;
        expected_frames += expected_frame_count(total, every_n);
        processed_frames += processed_frame_count(&segment_dir).min(total);
    }

    if expected_frames == 0 {
        return None;
    }

    let status = if processed_frames >= expected_frames {
        "completed"
    } else if processed_frames == 0 {
        "notStarted"
    } else {
        "partial"
    };

    Some(AutoAnnotateStatus {
        status: status.to_string(),
        total_frames,
        processed_frames,
    })
}

#[tauri::command]
async fn get_auto_annotate_statuses(
    videos_dir: String,
    every_n: Option<usize>,
) -> Result<HashMap<String, AutoAnnotateStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let every_n = every_n.unwrap_or(1).max(1);
        let root = Path::new(&videos_dir);
        if !root.is_dir() {
            return Ok(HashMap::new());
        }

        let mut statuses = HashMap::new();
        let entries =
            fs::read_dir(root).map_err(|e| format!("Failed to read videos dir: {}", e))?;
        for entry in entries.filter_map(|entry| entry.ok()) {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let video_id = entry.file_name().to_string_lossy().to_string();
            if let Some(status) = scan_annotate_status(&entry.path(), every_n) {
                statuses.insert(video_id, status);
            }
        }
        Ok(statuses)
    })
    .await
    .map_err(|e| format!("Status scan task failed: {}", e))?
}

#[tauri::command]
async fn get_segment_folders(
    videos_dir: String,
) -> Result<HashMap<String, Vec<String>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&videos_dir);
        if !root.is_dir() {
            return Ok(HashMap::new());
        }

        let mut folders: HashMap<String, Vec<String>> = HashMap::new();
        let entries =
            fs::read_dir(root).map_err(|e| format!("Failed to read videos dir: {}", e))?;
        for entry in entries.filter_map(|entry| entry.ok()) {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let video_id = entry.file_name().to_string_lossy().to_string();
            let mut subdirs: Vec<String> = Vec::new();
            if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                for sub_entry in sub_entries.filter_map(|sub| sub.ok()) {
                    if sub_entry
                        .file_type()
                        .map(|kind| kind.is_dir())
                        .unwrap_or(false)
                    {
                        subdirs.push(sub_entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            if !subdirs.is_empty() {
                subdirs.sort();
                folders.insert(video_id, subdirs);
            }
        }
        Ok(folders)
    })
    .await
    .map_err(|e| format!("Segment folder scan task failed: {}", e))?
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
        .manage(Mutex::new(JobRegistry {
            jobs: HashMap::new(),
        }))
        .manage(Mutex::new(AnnotateServerState { server: None }))
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
            get_last_frame_path,
            resave_labels,
            delete_dataset_item,
            get_item_paths,
            watch_directories,
            unwatch_directories,
            list_video_files,
            list_subdirs,
            reveal_in_file_manager,
            get_cli_args,
            resolve_dataset_kit,
            start_video_split,
            cancel_video_split,
            start_video_annotate,
            cancel_video_annotate,
            commit_segment_preview,
            discard_segment_preview,
            get_auto_annotate_statuses,
            get_segment_folders,
            frontend_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<Mutex<AnnotateServerState>>() {
                    if let Ok(mut guard) = state.lock() {
                        if let Some(server) = guard.server.take() {
                            if let Ok(mut child) = server.child.lock() {
                                let _ = child.kill();
                            }
                        }
                    }
                }
            }
        });
}
