import {
  readDir,
  writeTextFile,
  exists,
  readTextFileLines,
  remove,
} from "@tauri-apps/plugin-fs";
import { path } from "@tauri-apps/api";
import { convertFileSrc } from "@tauri-apps/api/core";

export type Dataset = {
  imagesDir: string;
  labelsDir: string;
};

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
};

async function validateDirectories(dataset: Dataset): Promise<void> {
  const [imagesExist, labelsExist] = await Promise.all([
    exists(dataset.imagesDir),
    exists(dataset.labelsDir),
  ]);

  if (!imagesExist) {
    throw new DirectoryNotFoundError(dataset.imagesDir);
  }
  if (!labelsExist) {
    throw new DirectoryNotFoundError(dataset.labelsDir);
  }

  try {
    await readDir(dataset.imagesDir);
  } catch (err) {
    throw new PermissionError("read", dataset.imagesDir);
  }

  try {
    await readDir(dataset.labelsDir);
  } catch (err) {
    throw new PermissionError("read", dataset.labelsDir);
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

export async function loadWholeDataset(
  dataset: Dataset
): Promise<DatasetItem[]> {
  await validateDirectories(dataset);

  let imageFiles;
  try {
    imageFiles = await readDir(dataset.imagesDir);
    imageFiles = imageFiles.filter((entry) => entry.isFile);
  } catch (err) {
    throw new DatasetLoadError(`Failed to read images directory: ${err}`);
  }

  const itemsPromises = imageFiles.map(
    async (imageFileEntry): Promise<DatasetItem> => {
      try {
        const name = removeExtension(imageFileEntry.name);
        const labelName = `${name}.txt`;
        const labelPath = await path.join(dataset.labelsDir, labelName);
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
          name,
          imageSrc: convertFileSrc(
            await path.join(dataset.imagesDir, imageFileEntry.name)
          ),
          labels,
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

export async function resaveLabelsToFile(dataset: Dataset, item: DatasetItem) {
  let contents = "";
  for (const label of item.labels) {
    const xCenter = label.left + label.width / 2;
    const yCenter = label.top + label.height / 2;
    contents += `${label.classId} ${xCenter} ${yCenter} ${label.width} ${label.height}\n`;
  }

  await writeTextFile(
    await path.join(dataset.labelsDir, `${item.name}.txt`),
    contents,
    { append: false }
  );
}

export async function itemImagePath(
  dataset: Dataset,
  item: DatasetItem
): Promise<string> {
  return path.join(
    dataset.imagesDir,
    `${item.name}.${getExtension(item.imageSrc)}`
  );
}

export async function itemLabelPath(
  dataset: Dataset,
  item: DatasetItem
): Promise<string> {
  return path.join(dataset.labelsDir, `${item.name}.txt`);
}

export async function deleteItem(dataset: Dataset, item: DatasetItem) {
  await Promise.all([
    itemImagePath(dataset, item).then(remove),
    itemLabelPath(dataset, item).then(remove),
  ]);
}
