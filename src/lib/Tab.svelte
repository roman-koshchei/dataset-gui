<script lang="ts">
  import { type DatasetItem, loadWholeDataset } from "./dataset";
  import DatasetGrid from "./DatasetGrid.svelte";
  import { history, pushToHistory, removeFromHistory } from "./history.svelte";

  let { active }: { active: boolean } = $props();

  let imagesDir = $state("");
  let labelsDir = $state("");
  let errorMessage = $state("");
  let isEdit = $state(true);
  let datasetItems = $state<DatasetItem[]>([]);

  async function selectDataset() {
    if (imagesDir == "" || labelsDir == "") {
      errorMessage = "Images and labels directories must be specified";
      return;
    }

    errorMessage = "";
    try {
      const items = await loadWholeDataset({ imagesDir, labelsDir });
      datasetItems = items;
      isEdit = false;
      await pushToHistory(imagesDir, labelsDir);
    } catch (err) {
      isEdit = true;
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<div class={["flex-[0_0_100%]", active ? "order-first" : ""]}>
  {#if isEdit}
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
               oninput={() => errorMessage = ""}
             />
           </label>

           <label class="mt-2 space-y-2 block">
             Labels directory
             <input
               type="text"
               class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
               placeholder="Enter labels folder..."
               bind:value={labelsDir}
               oninput={() => errorMessage = ""}
             />
           </label>

          {#if errorMessage}
            <p class="mt-4 text-red-500">{errorMessage}</p>
          {/if}

          <button
            class="mt-8 w-full bg-green-600 py-2 px-4 hover:bg-green-700 transition-colors"
            type="submit"
          >
            Select
          </button>
        </form>
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
                  errorMessage = "";
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
  {:else}
    <DatasetGrid bind:items={datasetItems} dataset={{ imagesDir, labelsDir }} />
  {/if}
</div>
