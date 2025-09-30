import { load } from "@tauri-apps/plugin-store";
import type { Dataset } from "./dataset";

const store = await load("store.json");

const HISTORY_KEY = "history";

export async function pushToHistory(imagesDir: string, labelsDir: string) {
  const current = (await store.get<Dataset[]>(HISTORY_KEY)) ?? [];
  const dataset: Dataset = { imagesDir, labelsDir };

  const filteredCurrent = current.filter(
    (x) => !(x.imagesDir === imagesDir && x.labelsDir === labelsDir)
  );
  const newHistory = [dataset, ...filteredCurrent].slice(0, 10);

  await store.set(HISTORY_KEY, newHistory);
  await store.save();
}

export async function getHistory(): Promise<Dataset[]> {
  return (await store.get<Dataset[]>(HISTORY_KEY)) ?? [];
}
