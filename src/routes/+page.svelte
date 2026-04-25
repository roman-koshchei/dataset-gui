<script lang="ts">
  import { initHistory } from "$lib/history.svelte";
  import Tab from "$lib/Tab.svelte";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { Dataset } from "$lib/dataset";

  type TabData = {
    id: string;
    label?: string;
    initialState?: Dataset;
  };

  let tabs = $state<TabData[]>([{ id: crypto.randomUUID() }]);
  let activeTabId = $state<string>(tabs[0].id);

  function closeTab(id: string) {
    const index = tabs.findIndex((x) => x.id === id);
    tabs.splice(index, 1);
    activeTabId = tabs[tabs.length - 1]?.id;
  }

  function addNewTab() {
    const id = crypto.randomUUID();
    tabs.push({ id });
    activeTabId = id;
  }

  function openDatasetInNewTab(dataset: Dataset, label: string) {
    const id = crypto.randomUUID();
    tabs.push({ id, label, initialState: dataset });
    activeTabId = id;
  }

  $effect(() => {
    document.title = activeTabId;
  });

  onMount(async () => {
    initHistory();

    try {
      const cliArgs = await invoke<{ imagesDir?: string; labelsDir?: string }>("get_cli_args");
      if (cliArgs.imagesDir && cliArgs.labelsDir) {
        tabs[0] = {
          id: tabs[0].id,
          label: cliArgs.imagesDir.split(/[/\\]/).pop(),
          initialState: { imagesDir: cliArgs.imagesDir, labelsDir: cliArgs.labelsDir },
        };
      }
    } catch {}
  });
</script>

<main class="grid grid-rows-[auto_1fr] h-screen">
  <div
    class="border-y border-zinc-700 overflow-x-auto scrollbar flex items-stretch"
  >
    <button
      aria-label="Add new tab"
      class="px-3 hover:bg-zinc-800 py-[0.42rem] transition-colors border-r border-zinc-700"
      onclick={() => addNewTab()}
    >
      <svg
        class="w-4 h-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M12 4v16m8-8H4"
        ></path>
      </svg>
    </button>
    {#each tabs.toReversed() as tab}
      <div
        class={[
          "flex-none flex items-baseline border-r py-1 border-zinc-700 pr-3 text-sm",
          activeTabId === tab.id
            ? "border-b-2 border-b-blue-500 text-white"
            : "hover:bg-zinc-800 text-zinc-400",
        ]}
      >
        <button
          class="h-full px-3 whitespace-nowrap"
          onclick={() => {
            activeTabId = tab.id;
          }}
        >
          {tab.label ?? tab.id.slice(0, 8)}
        </button>
        <button
          class=" border border-transparent hover:border-red-500 transition-colors"
          onclick={() => closeTab(tab.id)}
          aria-label="Close tab"
        >
          <svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
            <path
              fill-rule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clip-rule="evenodd"
            ></path>
          </svg>
        </button>
      </div>
    {/each}
  </div>

  <div class="overflow-hidden flex">
    {#each tabs as tab (tab.id)}
      <Tab active={activeTabId === tab.id} {openDatasetInNewTab} initialState={tab.initialState} />
    {/each}
  </div>
</main>

<style>
  .scrollbar::-webkit-scrollbar {
    display: none;
  }
</style>
