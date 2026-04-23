<script lang="ts">
  import { history, pushToHistory, removeFromHistory } from "./history.svelte";
  import DatasetGrid from "./DatasetGrid.svelte";
  import VideoManagement from "./VideoManagement.svelte";
  import { emptyVideoCollection } from "./video-collection";
  import { writeTextFile } from "@tauri-apps/plugin-fs";
  import { load } from "@tauri-apps/plugin-store";

  let {
    active,
    openDatasetInNewTab,
    initialState,
  }: {
    active: boolean;
    openDatasetInNewTab?: (imagesDir: string, labelsDir: string, label: string) => void;
    initialState?: { imagesDir: string; labelsDir: string };
  } = $props();

  type ViewMode = "start" | "dataset" | "videos";
  let viewMode = $state<ViewMode>(initialState ? "dataset" : "start");

  let imagesDir = $state(initialState?.imagesDir ?? "");
  let labelsDir = $state(initialState?.labelsDir ?? "");

  if (initialState) {
    pushToHistory(initialState.imagesDir, initialState.labelsDir);
  }
  let datasetError = $state("");
  let videoError = $state("");
  let videoDataPath = $state("");
  let videosDir = $state("");

  let store: Awaited<ReturnType<typeof load>> | null = null;
  async function getStore() {
    if (!store) store = await load("store.json");
    return store;
  }

  async function loadVideoPath() {
    try {
      const s = await getStore();
      const saved = await s.get<string>("videoDataPath");
      if (saved) videoDataPath = saved;
      const savedDir = await s.get<string>("videosDir");
      if (savedDir) videosDir = savedDir;
    } catch { }
  }

  async function saveVideoPath() {
    try {
      const s = await getStore();
      await s.set("videoDataPath", videoDataPath);
      await s.set("videosDir", videosDir);
      await s.save();
    } catch { }
  }

  loadVideoPath();

  async function selectDataset() {
    if (imagesDir == "" || labelsDir == "") {
      datasetError = "Images and labels directories must be specified";
      return;
    }

    datasetError = "";
    try {
      viewMode = "dataset";
      await pushToHistory(imagesDir, labelsDir);
    } catch (err) {
      viewMode = "start";
      datasetError = err instanceof Error ? err.message : String(err);
    }
  }

  async function createNewCollection() {
    if (!videoDataPath.trim()) {
      videoError = "Specify a data.json path for the new collection";
      return;
    }
    if (!videoDataPath.endsWith("data.json")) {
      videoError = "Path must end with data.json";
      return;
    }
    videoError = "";
    try {
      const collection = emptyVideoCollection();
      collection.collection = videoDataPath.replace(/[/\\]data\.json$/, "").split(/[/\\]/).pop() || "";
      await writeTextFile(videoDataPath, JSON.stringify(collection, null, 2) + "\n");
      await saveVideoPath();
      viewMode = "videos";
    } catch (err) {
      videoError = err instanceof Error ? err.message : String(err);
    }
  }

  async function openVideoManagement() {
    if (!videoDataPath) {
      videoError = "Video data path must be specified";
      return;
    }
    videoError = "";
    await saveVideoPath();
    viewMode = "videos";
  }
</script>

<div class={["flex-[0_0_100%]", active ? "order-first" : ""]}>
  {#if viewMode === "start"}
    <div class="h-full overflow-y-auto grid grid-cols-2 gap-8 p-4">
      <div class="space-y-6">
        <h2 class="text-xl">Add new dataset</h2>

        <form onsubmit={selectDataset}>
           <label class="space-y-2 block">
             Images directory
             <input
               type="text"
               class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
               placeholder="Enter images folder..."
               bind:value={imagesDir}
              oninput={() => datasetError = ""}
            />
          </label>

          <label class="mt-2 space-y-2 block">
            Labels directory
            <input
              type="text"
              class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
              placeholder="Enter labels folder..."
              bind:value={labelsDir}
              oninput={() => datasetError = ""}
            />
          </label>

         {#if datasetError}
           <p class="mt-4 text-red-500">{datasetError}</p>
          {/if}

          <button
            class="mt-8 w-full bg-green-600 py-2 px-4 hover:bg-green-700 transition-colors"
            type="submit"
          >
            Select
          </button>
        </form>

        <div class="border-t border-zinc-700 pt-6 space-y-3">
          <h3 class="text-lg">Video Management</h3>
          <label class="space-y-2 block">
            data.json path
            <input
              type="text"
              class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
              placeholder="D:\Datasets\...\data.json"
              bind:value={videoDataPath}
              oninput={() => videoError = ""}
            />
          </label>
          <label class="space-y-2 block">
            Videos directory <span class="text-zinc-500">(defaults to ./videos alongside data.json)</span>
            <input
              type="text"
              class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
              placeholder="D:\Datasets\...\videos"
              bind:value={videosDir}
            />
          </label>
          {#if videoError}
            <p class="text-red-500 text-sm">{videoError}</p>
          {/if}
          <div class="flex gap-2">
            <button
              class="flex-1 bg-blue-600 py-2 px-4 hover:bg-blue-700 transition-colors"
              onclick={openVideoManagement}
            >
              Manage Videos
            </button>
            <button
              class="bg-zinc-700 py-2 px-4 hover:bg-zinc-600 transition-colors"
              onclick={createNewCollection}
            >
              New
            </button>
          </div>
        </div>
      </div>

      <div class="space-y-6">
        <h2 class="text-xl">Select from history</h2>

        <div class="space-y-3">
          {#each history.items as dataset}
            <div
              class="grid grid-cols-[1fr_auto] items-stretch border border-zinc-700"
            >
              <button
                onclick={async () => {
                  datasetError = "";
                  imagesDir = dataset.imagesDir;
                  labelsDir = dataset.labelsDir;
                  await selectDataset();
                }}
                class="px-3 py-2 hover:bg-zinc-800 text-left"
              >
                <p>{dataset.imagesDir}</p>
                <p>{dataset.labelsDir}</p>
              </button>
              <div
                class="border-l border-zinc-700 px-3 grid place-content-center"
              >
                <button
                  aria-label={`Delete from history: ${dataset.labelsDir}`}
                  class="py-1 px-3 bg-red-600 hover:bg-red-700"
                  onclick={async () => {
                    try {
                      await removeFromHistory(
                        dataset.imagesDir,
                        dataset.labelsDir
                      );
                    } catch (err) {
                      console.error("Failed to remove from history:", err);
                    }
                  }}
                >
                  Delete
                </button>
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {:else if viewMode === "dataset"}
    <DatasetGrid dataset={{ imagesDir, labelsDir }} />
  {:else if viewMode === "videos"}
    <VideoManagement dataPath={videoDataPath} {videosDir} onBack={() => viewMode = "start"} {openDatasetInNewTab} />
  {/if}
</div>
