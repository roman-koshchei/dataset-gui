<script lang="ts">
  import Tab from "../lib/Tab.svelte";
  import { type Dataset, loadWholeDataset } from "../lib/dataset";
  import DatasetGrid from "../lib/DatasetGrid.svelte";

  // import { invoke } from "@tauri-apps/api/core";

  let tabs = $state<string[]>([]);
  let activeTab = $state(0);

  const dataset: Dataset = {
    imagesDir: "C:\\Work\\Project\\Datasets\\aerial-misses\\images",
    labelsDir: "C:\\Work\\Project\\Datasets\\aerial-misses\\labels",
  };

  const items = loadWholeDataset(dataset);
</script>

<!-- 
<div class="flex flex-wrap">
  {#each tabs as tab}
    <button class="px-2 py-1 border border-amber-500 cursor-pointer">
      {tab}
    </button>
  {/each}
  <button
    onclick={() => {
      tabs.push("Undefined");
    }}
    class="px-2 py-1 border border-amber-500 cursor-pointer">+</button
  >
</div> -->

{#await items then i}
  <DatasetGrid {dataset} items={i} />
{/await}
