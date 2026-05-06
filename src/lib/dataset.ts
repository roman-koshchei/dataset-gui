import { writeTextFile, exists, remove } from "@tauri-apps/plugin-fs";
import { path } from "@tauri-apps/api";
import { invoke } from "@tauri-apps/api/core";

export type DatasetDir = { imagesDir: string; labelsDir: string };
export type Dataset = { dirs: DatasetDir[] };

export type DatasetLoadProgress = {
  loadId: string;
  phase: string;
  loaded: number;
  total: number;
  current?: string | null;
};

type LoadProgressCallback = (progress: DatasetLoadProgress) => void;

const LOAD_BATCH_SIZE = 500;

export function logPerformance(message: string) {
  const formatted = `[dataset-gui perf] ${new Date().toISOString()} ${message}`;
  console.log(formatted);
  void invoke("frontend_log", { message: formatted }).catch(() => {});
}

export function datasetDirs(dataset: Dataset): DatasetDir[] {
  return dataset.dirs;
}

export class DatasetLoadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DatasetLoadError";
  }
}

export class DirectoryNotFoundError extends DatasetLoadError {
  constructor(directory: string) {
    super(`Directory does not exist: ${directory}`);
    this.name = "DirectoryNotFoundError";
  }
}

export class PermissionError extends DatasetLoadError {
  constructor(operation: string, path: string) {
    super(`Permission denied: cannot ${operation} ${path}`);
    this.name = "PermissionError";
  }
}

export class MalformedLabelError extends DatasetLoadError {
  constructor(labelFile: string, reason: string) {
    super(`Malformed label in file ${labelFile}: ${reason}`);
    this.name = "MalformedLabelError";
  }
}

export type DatasetLabel = {
  classId: number;
  left: number;
  top: number;
  width: number;
  height: number;
};

export function normArea(label: DatasetLabel): number {
  return label.width * label.height;
}

export type DatasetItem = {
  name: string;
  imageSrc: string;
  labels: DatasetLabel[];
  hasLabelFile: boolean;
  imagesDir: string;
  labelsDir: string;
};

export function datasetItemKey(item: DatasetItem): string {
  return `${item.imagesDir}\0${item.name}`;
}

export function datasetLabelKey(label: DatasetLabel): string {
  return `${label.left},${label.top},${label.left + label.width},${label.top + label.height}`;
}

function getExtension(filename: string): string {
  const lastDotIndex = filename.lastIndexOf(".");
  if (lastDotIndex === -1) return "";
  return filename.slice(lastDotIndex + 1);
}

export async function loadWholeDataset(
  dataset: Dataset,
  loadId: string,
  onProgress?: LoadProgressCallback
): Promise<DatasetItem[]> {
  let prepared = false;
  try {
    const startMs = performance.now();

    logPerformance(`Dataset load started: dirs=${dataset.dirs.length}`);
    onProgress?.({ loadId, phase: "Scanning files", loaded: 0, total: dataset.dirs.length });

    const total = await invoke<number>("prepare_dataset_load", { loadId, dirs: dataset.dirs });
    prepared = true;
    logPerformance(`Prepared dataset load: total=${total} elapsedMs=${Math.round(performance.now() - startMs)}`);

    const items: DatasetItem[] = [];
    let loaded = 0;
    let batchIndex = 0;
    onProgress?.({ loadId, phase: "Loading labels", loaded, total });

    for (let offset = 0; offset < total; offset += LOAD_BATCH_SIZE) {
      const batch = await invoke<DatasetItem[]>("load_prepared_dataset_batch", {
        loadId,
        offset,
        limit: Math.min(LOAD_BATCH_SIZE, total - offset),
      });

      items.push(...batch);
      loaded += batch.length;
      batchIndex += 1;
      logPerformance(`Loaded metadata batch: batch=${batchIndex} loaded=${loaded}/${total} batchItems=${batch.length}`);
      onProgress?.({
        loadId,
        phase: "Loading labels",
        loaded,
        total,
      });

      if (batch.length === 0) break;
    }

    onProgress?.({ loadId, phase: "Complete", loaded: items.length, total: items.length });
    logPerformance(`Dataset metadata loaded: items=${items.length} elapsedMs=${Math.round(performance.now() - startMs)}`);
    return items;
  } catch (err) {
    throw new DatasetLoadError(err instanceof Error ? err.message : String(err));
  } finally {
    if (prepared) {
      await invoke("clear_prepared_dataset_load", { loadId }).catch(() => {});
    }
  }
}

export type FilterParams = {
  mode: string;
  classId?: number;
  nth?: number;
};

export type FilteredWindow = {
  totalFiltered: number;
  items: DatasetItem[];
};

export async function loadAndStoreBatch(
  loadId: string,
  offset: number,
  limit: number
): Promise<number> {
  return invoke<number>("load_and_store_batch", { loadId, offset, limit });
}

export async function getFilteredWindow(
  loadId: string,
  filter: FilterParams,
  offset: number,
  limit: number
): Promise<FilteredWindow> {
  return invoke<FilteredWindow>("get_filtered_window", { loadId, filter, offset, limit });
}

export async function updateStoredItem(
  loadId: string,
  item: DatasetItem
): Promise<void> {
  await invoke("update_stored_item", { loadId, item });
}

export async function removeStoredItem(
  loadId: string,
  name: string,
  imagesDir: string
): Promise<void> {
  await invoke("remove_stored_item", { loadId, name, imagesDir });
}

export async function resaveAllLabels(loadId: string): Promise<number> {
  return invoke<number>("resave_all_labels", { loadId });
}

export async function clearStoredDataset(loadId: string): Promise<void> {
  await invoke("clear_stored_dataset", { loadId });
}

export function getItemBaseName(name: string): string {
  return name.includes("/") ? name.split("/").pop()! : name;
}

export function getItemImageExt(item: DatasetItem): string {
  const filename = item.imageSrc.split("/").pop() ?? "";
  const lastDot = filename.lastIndexOf(".");
  return lastDot === -1 ? "" : filename.slice(lastDot + 1);
}

export async function resaveLabelsToFile(_dataset: Dataset, item: DatasetItem) {
  let contents = "";
  for (const label of item.labels) {
    const xCenter = label.left + label.width / 2;
    const yCenter = label.top + label.height / 2;
    contents += `${label.classId} ${xCenter} ${yCenter} ${label.width} ${label.height}\n`;
  }

  const name = item.name.includes("/") ? item.name.split("/").pop()! : item.name;
  await writeTextFile(
    await path.join(item.labelsDir, `${name}.txt`),
    contents,
    { append: false }
  );
  item.hasLabelFile = true;
}

export async function itemImagePath(
  _dataset: Dataset,
  item: DatasetItem
): Promise<string> {
  const name = item.name.includes("/") ? item.name.split("/").pop()! : item.name;
  return path.join(
    item.imagesDir,
    `${name}.${getExtension(item.imageSrc)}`
  );
}

export async function itemLabelPath(
  _dataset: Dataset,
  item: DatasetItem
): Promise<string> {
  const name = item.name.includes("/") ? item.name.split("/").pop()! : item.name;
  return path.join(item.labelsDir, `${name}.txt`);
}

export async function deleteItem(dataset: Dataset, item: DatasetItem) {
  const imagePath = await itemImagePath(dataset, item);
  const labelPath = await itemLabelPath(dataset, item);

  await remove(imagePath);
  if (await exists(labelPath)) {
    await remove(labelPath);
  }
}
