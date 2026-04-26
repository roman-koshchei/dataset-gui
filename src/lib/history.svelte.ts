import { load } from "@tauri-apps/plugin-store";
import type { SingleDataset } from "./dataset";

export const history = $state<{ items: SingleDataset[] }>({ items: [] });
export type VideoHistoryEntry = { dataPath: string; videosDir: string };

export const videoHistory = $state<{ items: VideoHistoryEntry[] }>({ items: [] });

let store: Awaited<ReturnType<typeof load>> | null = null;
const HISTORY_KEY = "history";
const VIDEO_HISTORY_KEY = "videoHistory";

async function getStore() {
  if (!store) {
    store = await load("store.json");
  }
  return store;
}

export async function loadHistory(): Promise<SingleDataset[]> {
  const s = await getStore();
  return (await s.get<SingleDataset[]>(HISTORY_KEY)) ?? [];
}

export async function pushToHistory(imagesDir: string, labelsDir: string) {
  const s = await getStore();
  const current = (await s.get<SingleDataset[]>(HISTORY_KEY)) ?? [];
  const dataset: SingleDataset = { imagesDir, labelsDir };

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
  const current = (await s.get<SingleDataset[]>(HISTORY_KEY)) ?? [];
  const newHistory = current.filter(
    (x) => !(x.imagesDir === imagesDir && x.labelsDir === labelsDir)
  );

  await s.set(HISTORY_KEY, newHistory);
  await s.save();

  history.items = newHistory;
}

export async function loadVideoHistory(): Promise<VideoHistoryEntry[]> {
  const s = await getStore();
  return (await s.get<VideoHistoryEntry[]>(VIDEO_HISTORY_KEY)) ?? [];
}

export async function pushToVideoHistory(dataPath: string, videosDir: string) {
  const s = await getStore();
  const current = (await s.get<VideoHistoryEntry[]>(VIDEO_HISTORY_KEY)) ?? [];

  const filteredCurrent = current.filter((x) => x.dataPath !== dataPath);
  const newHistory = [{ dataPath, videosDir }, ...filteredCurrent].slice(0, 10);

  await s.set(VIDEO_HISTORY_KEY, newHistory);
  await s.save();

  videoHistory.items = newHistory;
}

export async function removeFromVideoHistory(dataPath: string) {
  const s = await getStore();
  const current = (await s.get<VideoHistoryEntry[]>(VIDEO_HISTORY_KEY)) ?? [];
  const newHistory = current.filter((x) => x.dataPath !== dataPath);

  await s.set(VIDEO_HISTORY_KEY, newHistory);
  await s.save();

  videoHistory.items = newHistory;
}

export async function initHistory() {
  history.items = await loadHistory();
  videoHistory.items = await loadVideoHistory();
}
