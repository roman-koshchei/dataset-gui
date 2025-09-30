<script lang="ts">
  import { type DatasetItem, loadWholeDataset } from "./dataset";
  import DatasetGrid from "./DatasetGrid.svelte";
  import { getHistory, pushToHistory } from "./store";

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

    isEdit = false;
    await Promise.all([
      loadWholeDataset({ imagesDir, labelsDir }).then((x) => {
        datasetItems = x;
      }),
      pushToHistory(imagesDir, labelsDir),
    ]);
  }
</script>

<div class={["overflow-y-auto", active ? "order-first flex-[0_0_100%]" : ""]}>
  {#if isEdit}
    <div class="grid grid-cols-2 gap-10 h-full p-4">
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
            />
          </label>

          <label class="mt-2 space-y-2 block">
            Labels directory
            <input
              type="text"
              class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
              placeholder="Enter labels folder..."
              bind:value={labelsDir}
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

      <div class="space-y-6 overflow-y-auto">
        <h2 class="text-xl">Select from history</h2>

        <div class="space-y-3">
          {#await getHistory() then datasets}
            {#each datasets as dataset}
              <button
                onclick={async () => {
                  imagesDir = dataset.imagesDir;
                  labelsDir = dataset.labelsDir;
                  await selectDataset();
                }}
                class="w-full px-3 py-2 border border-zinc-700 hover:bg-zinc-800 transition-colors text-left"
              >
                <p>{dataset.imagesDir}</p>
                <p>{dataset.labelsDir}</p>
              </button>
            {/each}
          {/await}
        </div>
      </div>
    </div>
  {:else}
    <DatasetGrid bind:items={datasetItems} dataset={{ imagesDir, labelsDir }} />
  {/if}
</div>
