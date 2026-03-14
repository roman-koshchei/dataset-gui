import { load } from "@tauri-apps/plugin-store";
import type { Dataset } from "./dataset";

export const history = $state<{ items: Dataset[] }>({ items: [] });

let store: Awaited<ReturnType<typeof load>> | null = null;
const HISTORY_KEY = "history";

async function getStore() {
  if (!store) {
    store = await load("store.json");
  }
  return store;
}

export async function loadHistory(): Promise<Dataset[]> {
  const s = await getStore();
  return (await s.get<Dataset[]>(HISTORY_KEY)) ?? [];
}

export async function pushToHistory(imagesDir: string, labelsDir: string) {
  const s = await getStore();
  const current = (await s.get<Dataset[]>(HISTORY_KEY)) ?? [];
  const dataset: Dataset = { imagesDir, labelsDir };

  const filteredCurrent = current.filter(
    (x) => !(x.imagesDir === imagesDir && x.labelsDir === labelsDir)
  );
  const newHistory = [dataset, ...filteredCurrent].slice(0, 10);

  await s.set(HISTORY_KEY, newHistory);
  await s.save();

  history.items = newHistory;
}

export async function removeFromHistory(imagesDir: string, labelsDir: string) {
  const s = await getStore();
  const current = (await s.get<Dataset[]>(HISTORY_KEY)) ?? [];
  const newHistory = current.filter(
    (x) => !(x.imagesDir === imagesDir && x.labelsDir === labelsDir)
  );

  await s.set(HISTORY_KEY, newHistory);
  await s.save();

  history.items = newHistory;
}

export async function initHistory() {
  history.items = await loadHistory();
}
