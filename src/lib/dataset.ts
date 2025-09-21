import {
  readDir,
  writeTextFile,
  exists,
  readTextFileLines,
} from "@tauri-apps/plugin-fs";
import { path } from "@tauri-apps/api";

export type Dataset = {
  imagesDir: string;
  labelsDir: string;
};

export type DatasetLabel = {
  classId: number;
  left: number;
  top: number;
  width: number;
  height: number;
};

function parseLabelFromYoloLine(line: string): DatasetLabel {
  const parts = line.split(" ");
  const classId = parseInt(parts[0], 10);
  const xCenter = parseFloat(parts[1]);
  const yCenter = parseFloat(parts[2]);
  const width = parseFloat(parts[3]);
  const height = parseFloat(parts[4]);

  return {
    classId: classId,
    left: xCenter - width / 2,
    top: yCenter - height / 2,
    width: width,
    height: height,
  };
}

export type DatasetItem = {
  imageName: string;
  labelName: string;
  labels: DatasetLabel[];
};

function removeExtension(filename: string) {
  const lastDotIndex = filename.lastIndexOf(".");
  if (lastDotIndex === -1) return filename;
  return filename.slice(0, lastDotIndex);
}

export async function loadWholeDataset(
  dataset: Dataset
): Promise<DatasetItem[]> {
  const imageFiles = await readDir(dataset.imagesDir).then((x) =>
    x.filter((entry) => entry.isFile)
  );

  const itemsPromises = imageFiles.map(
    async (imageFileEntry): Promise<DatasetItem> => {
      const labelName = `${removeExtension(imageFileEntry.name)}.txt`;
      const labelPath = await path.join(dataset.labelsDir, labelName);
      const labels = [];
      if (await exists(labelPath)) {
        const lines = await readTextFileLines(labelPath);
        for await (const line of lines) {
          labels.push(parseLabelFromYoloLine(line));
        }
      }
      return { imageName: imageFileEntry.name, labelName: labelName, labels };
    }
  );

  return await Promise.all(itemsPromises);
}

export async function saveLabelsToFile(
  labelsDir: string,
  labelName: string,
  labels: DatasetLabel[]
) {
  let contents = "";
  for (const label of labels) {
    const xCenter = label.left + label.width / 2;
    const yCenter = label.top + label.height / 2;
    contents += `0 ${xCenter} ${yCenter} ${label.width} ${label.height}\n`;
  }

  await writeTextFile(await path.join(labelsDir, labelName), contents, {
    append: false,
  });
}
