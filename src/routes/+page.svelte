<script lang="ts">
  import Tab from "../lib/Tab.svelte";

  let tabs = $state<{ id: string; title: string | null }[]>([]);
  let activeTab = $state(0);

  function closeTab(index: number) {
    tabs.splice(index, 1);
    activeTab = activeTab <= 1 ? 0 : activeTab - 1;
  }

  function addNewTab() {
    tabs.push({
      id: crypto.randomUUID(),
      title: null,
    });
    activeTab = tabs.length - 1;
  }
</script>

<main class="grid grid-rows-[auto_1fr] h-screen w-screen overflow-hidden">
  <div class="border-y border-zinc-700 overflow-x-auto flex items-stretch h-10">
    {#each tabs as tab, index}
      <div
        class={[
          "flex-none flex items-baseline gap-3 border-r border-zinc-700 px-3",
          activeTab === index
            ? "border-b-2 border-b-blue-500 text-white"
            : "hover:bg-zinc-800 text-zinc-400",
        ]}
      >
        <button
          class="h-full cursor-pointer whitespace-nowrap"
          onclick={() => {
            activeTab = index;
          }}
        >
          {tab.title ?? `Tab ${index}`}
        </button>
        <button
          class=" border border-transparent hover:border-red-500 cursor-pointer transition-colors"
          onclick={() => closeTab(index)}
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
  </div>

  <div class="overflow-y-auto">
    {#each tabs as _, index}
      <Tab active={activeTab === index} />
    {/each}
  </div>
</main>

