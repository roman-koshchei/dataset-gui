<script lang="ts">
  import Tab from "../lib/Tab.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let tabs = $state<{ id: string }[]>([{ id: crypto.randomUUID() }]);
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

  $effect(() => {
    const tab = activeTabId;
    getCurrentWindow().setTitle(tab);
  });
</script>

<main class="grid grid-rows-[auto_1fr] h-screen w-screen overflow-hidden">
  <div
    class="border-y border-zinc-700 overflow-x-auto scrollbar flex items-stretch"
  >
    <button
      aria-label="Add new tab"
      class="px-3 py-2 cursor-pointer hover:bg-zinc-800 transition-colors border-r border-zinc-700"
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
          "flex-none flex items-baseline border-r border-zinc-700 pr-3 text-sm",
          activeTabId === tab.id
            ? "border-b-2 border-b-blue-500 text-white"
            : "hover:bg-zinc-800 text-zinc-400",
        ]}
      >
        <button
          class="h-full px-3 cursor-pointer whitespace-nowrap"
          onclick={() => {
            activeTabId = tab.id;
          }}
        >
          {tab.id}
        </button>
        <button
          class=" border border-transparent hover:border-red-500 cursor-pointer transition-colors"
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
      <Tab active={activeTabId === tab.id} />
    {/each}
  </div>
</main>

<style>
  .scrollbar::-webkit-scrollbar {
    display: none;
  }
</style>
