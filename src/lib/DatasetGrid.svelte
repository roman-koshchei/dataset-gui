<script lang="ts">
  import {
    deleteItem,
    itemImagePath,
    itemLabelPath,
    loadWholeDataset,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import EditDialog from "./EditDialog.svelte";
  import { numberToTailwindBorder, numberToTailwindBg } from "./helper";
  let {
    dataset,
    items = $bindable(),
  }: { dataset: Dataset; items: DatasetItem[] } = $props();

  let selectedItem = $state<DatasetItem | null>(null);
  let reloadIsActive = $state(false);

  async function handleDelete(name: string) {
    const index = items.findIndex((x) => x.name === name);
    const item = items[index];
    await deleteItem(dataset, item);
    items.splice(index, 1);
  }

  function openEditDialog(item: DatasetItem) {
    selectedItem = item;
  }

  function closeEditDialog() {
    selectedItem = null;
  }
</script>

<section class="h-full grid grid-rows-[auto_1fr]">
  <div class="border-b border-zinc-700 flex items-stretch text-sm">
    <button
      class="px-3 border-r border-zinc-700 py-1 disabled:text-zinc-700"
      onclick={async () => {
        try {
          reloadIsActive = true;
          const newItems = await loadWholeDataset(dataset);
          items = newItems;
        } finally {
          reloadIsActive = false;
        }
      }}
      disabled={reloadIsActive}
    >
      Reload
    </button>

    <!-- 
    <label class="px-3 flex items-center gap-2 border-r border-zinc-700">
      <input type="checkbox" bind:checked={showOnlyItemsWith0Labels} />
      Show only items with 0 labels
    </label> 
    -->
  </div>

  <div
    class="overflow-y-auto grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 divide-y divide-x divide-zinc-700"
  >
    {#each items as item (item.name)}
      <div class="p-1 grid grid-rows-[auto_1fr] gap-1">
        <button
          class="relative overflow-hidden"
          onclick={() => openEditDialog(item)}
        >
          <img
            class="w-full h-auto"
            width={1280}
            height={720}
            src={item.imageSrc}
            alt=""
            loading="lazy"
          />

          {#each item.labels as label}
            <div
              class={[
                "absolute border",
                numberToTailwindBorder(label.classId),
                numberToTailwindBg(label.classId),
              ]}
              style:left={`${label.left * 100}%`}
              style:top={`${label.top * 100}%`}
              style:width={`${label.width * 100}%`}
              style:height={`${label.height * 100}%`}
            ></div>
          {/each}
        </button>

        <div class="flex flex-wrap items-end gap-2">
          <p>{item.name}</p>
          <button
            onclick={() => {
              handleDelete(item.name);
            }}
            class="bg-red-700 px-1"
          >
            Delete
          </button>
          <button
            class="bg-zinc-700 px-1"
            onclick={async () => {
              await revealItemInDir(
                await Promise.all([
                  itemLabelPath(dataset, item),
                  itemImagePath(dataset, item),
                ])
              );
            }}
          >
            Reveal files
          </button>
        </div>
      </div>
    {/each}
  </div>
</section>

<EditDialog {dataset} bind:item={selectedItem} onClose={closeEditDialog} />
