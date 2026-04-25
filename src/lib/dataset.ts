import {
  readDir,
  writeTextFile,
  exists,
  readTextFileLines,
  remove,
} from "@tauri-apps/plugin-fs";
import { path } from "@tauri-apps/api";
import { convertFileSrc } from "@tauri-apps/api/core";

export type SingleDataset = { imagesDir: string; labelsDir: string };
export type MultiDataset = { dirs: { imagesDir: string; labelsDir: string }[] };
export type Dataset = SingleDataset | MultiDataset;

export function datasetDirs(dataset: Dataset): { imagesDir: string; labelsDir: string }[] {
  return "dirs" in dataset ? dataset.dirs : [{ imagesDir: dataset.imagesDir, labelsDir: dataset.labelsDir }];
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

function parseLabelFromYoloLine(line: string): DatasetLabel {
  const parts = line.trim().split(/\s+/);
  
  if (parts.length < 5) {
    throw new Error(`Invalid format: expected 5 values, got ${parts.length}`);
  }

  const classId = parseInt(parts[0], 10);
  const xCenter = parseFloat(parts[1]);
  const yCenter = parseFloat(parts[2]);
  const width = parseFloat(parts[3]);
  const height = parseFloat(parts[4]);

  if (isNaN(classId)) {
    throw new Error(`Invalid classId: "${parts[0]}" is not a number`);
  }
  if (isNaN(xCenter)) {
    throw new Error(`Invalid xCenter: "${parts[1]}" is not a number`);
  }
  if (isNaN(yCenter)) {
    throw new Error(`Invalid yCenter: "${parts[2]}" is not a number`);
  }
  if (isNaN(width)) {
    throw new Error(`Invalid width: "${parts[3]}" is not a number`);
  }
  if (isNaN(height)) {
    throw new Error(`Invalid height: "${parts[4]}" is not a number`);
  }

  return {
    classId: classId,
    left: xCenter - width / 2,
    top: yCenter - height / 2,
    width: width,
    height: height,
  };
}

export type DatasetItem = {
  name: string;
  imageSrc: string;
  labels: DatasetLabel[];
  imagesDir: string;
  labelsDir: string;
};

async function validateDirectories(dataset: Dataset): Promise<void> {
  for (const { imagesDir, labelsDir } of datasetDirs(dataset)) {
    const [imagesExist, labelsExist] = await Promise.all([
      exists(imagesDir),
      exists(labelsDir),
    ]);

    if (!imagesExist) {
      throw new DirectoryNotFoundError(imagesDir);
    }
    if (!labelsExist) {
      throw new DirectoryNotFoundError(labelsDir);
    }

    try {
      await readDir(imagesDir);
    } catch (err) {
      throw new PermissionError("read", imagesDir);
    }

    try {
      await readDir(labelsDir);
    } catch (err) {
      throw new PermissionError("read", labelsDir);
    }
  }
}

function removeExtension(filename: string) {
  const lastDotIndex = filename.lastIndexOf(".");
  if (lastDotIndex === -1) return filename;
  return filename.slice(0, lastDotIndex);
}

function getExtension(filename: string): string {
  const lastDotIndex = filename.lastIndexOf(".");
  if (lastDotIndex === -1) return "";
  return filename.slice(lastDotIndex + 1);
}

async function loadItemsFromDir(
  imagesDir: string,
  labelsDir: string,
  namePrefix?: string
): Promise<DatasetItem[]> {
  let imageFiles;
  try {
    imageFiles = await readDir(imagesDir);
    imageFiles = imageFiles.filter((entry) => entry.isFile);
  } catch (err) {
    throw new DatasetLoadError(`Failed to read images directory: ${err}`);
  }

  const itemsPromises = imageFiles.map(
    async (imageFileEntry): Promise<DatasetItem> => {
      try {
        const name = removeExtension(imageFileEntry.name);
        const fullName = namePrefix ? `${namePrefix}/${name}` : name;
        const labelName = `${name}.txt`;
        const labelPath = await path.join(labelsDir, labelName);
        const labels: DatasetLabel[] = [];
        
        if (await exists(labelPath)) {
          try {
            const lines = await readTextFileLines(labelPath);
            for await (const line of lines) {
              if (line.trim()) {
                labels.push(parseLabelFromYoloLine(line));
              }
            }
          } catch (err) {
            throw new MalformedLabelError(labelName, err instanceof Error ? err.message : String(err));
          }
        }
        
        return {
          name: fullName,
          imageSrc: convertFileSrc(
            await path.join(imagesDir, imageFileEntry.name)
          ),
          labels,
          imagesDir,
          labelsDir,
        };
      } catch (err) {
        if (err instanceof MalformedLabelError) {
          throw err;
        }
        throw new DatasetLoadError(`Failed to load file ${imageFileEntry.name}: ${err}`);
      }
    }
  );

  return await Promise.all(itemsPromises);
}

export async function loadWholeDataset(
  dataset: Dataset
): Promise<DatasetItem[]> {
  await validateDirectories(dataset);

  const allItems: DatasetItem[] = [];
  for (const { imagesDir, labelsDir } of datasetDirs(dataset)) {
    const items = await loadItemsFromDir(imagesDir, labelsDir);
    allItems.push(...items);
  }
  return allItems;
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
  await Promise.all([
    itemImagePath(dataset, item).then(remove),
    itemLabelPath(dataset, item).then(remove),
  ]);
}

export async function getDatasetCount(dataset: Dataset): Promise<number> {
  await validateDirectories(dataset);

  let total = 0;
  for (const { imagesDir } of datasetDirs(dataset)) {
    try {
      const imageFiles = await readDir(imagesDir);
      total += imageFiles.filter((entry) => entry.isFile).length;
    } catch (err) {
      throw new DatasetLoadError(`Failed to read images directory: ${err}`);
    }
  }

  return total;
}

export async function loadDatasetBatch(
  dataset: Dataset,
  offset: number,
  limit: number
): Promise<DatasetItem[]> {
  await validateDirectories(dataset);

  const dirs = datasetDirs(dataset);
  const allItems: DatasetItem[] = [];

  for (const { imagesDir, labelsDir } of dirs) {
    if (allItems.length >= offset + limit) break;

    let imageFiles;
    try {
      imageFiles = await readDir(imagesDir);
      imageFiles = imageFiles.filter((entry) => entry.isFile);
    } catch (err) {
      throw new DatasetLoadError(`Failed to read images directory: ${err}`);
    }

    const endOfDir = allItems.length + imageFiles.length;
    if (endOfDir <= offset) {
      allItems.push(...Array(imageFiles.length).fill(null));
      continue;
    }

    const localStart = Math.max(0, offset - allItems.length);
    const localEnd = Math.min(imageFiles.length, offset + limit - allItems.length);
    const batchFiles = imageFiles.slice(localStart, localEnd);

    const dirPath = imagesDir.split(/[/\\]/);
    const segIdx = dirPath.lastIndexOf("videos");
    const prefix = segIdx >= 0 ? dirPath.slice(segIdx + 1, -1).join("/") : undefined;

    const itemsPromises = batchFiles.map(
      async (imageFileEntry): Promise<DatasetItem> => {
        try {
          const name = removeExtension(imageFileEntry.name);
          const fullName = prefix ? `${prefix}/${name}` : name;
          const labelName = `${name}.txt`;
          const labelPath = await path.join(labelsDir, labelName);
          const labels: DatasetLabel[] = [];
          
          if (await exists(labelPath)) {
            try {
              const lines = await readTextFileLines(labelPath);
              for await (const line of lines) {
                if (line.trim()) {
                  labels.push(parseLabelFromYoloLine(line));
                }
              }
            } catch (err) {
              throw new MalformedLabelError(labelName, err instanceof Error ? err.message : String(err));
            }
          }
          
          return {
            name: fullName,
            imageSrc: convertFileSrc(
              await path.join(imagesDir, imageFileEntry.name)
            ),
            labels,
            imagesDir,
            labelsDir,
          };
        } catch (err) {
          if (err instanceof MalformedLabelError) {
            throw err;
          }
          throw new DatasetLoadError(`Failed to load file ${imageFileEntry.name}: ${err}`);
        }
      }
    );

    const items = await Promise.all(itemsPromises);
    allItems.push(...items);
  }

  return allItems.filter(Boolean);
}
