import { load } from "@tauri-apps/plugin-store";
import type { Dataset } from "./dataset";

const store = await load("store.json");

const HISTORY_KEY = "history";

export async function pushToHistory(imagesDir: string, labelsDir: string) {
  const current = (await store.get<Dataset[]>(HISTORY_KEY)) ?? [];
  if (
    current.some((x) => x.imagesDir === imagesDir && x.labelsDir === labelsDir)
  ) {
    return;
  }

  const dataset: Dataset = { imagesDir, labelsDir };

  await store.set(HISTORY_KEY, [dataset, ...current.slice(0, 10)]);
  await store.save();
}

export async function getHistory(): Promise<Dataset[]> {
  return (await store.get<Dataset[]>("recent")) ?? [];
}
