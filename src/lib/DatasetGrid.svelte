<script lang="ts">
  import {
    deleteItem,
    itemImagePath,
    itemLabelPath,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import EditDialog from "$lib/EditDialog.svelte";
  import ItemWithLabels from "$lib/ItemWithLabels.svelte";
  let {
    dataset,
    items = $bindable(),
  }: { dataset: Dataset; items: DatasetItem[] } = $props();

  let selectedItem = $state<DatasetItem | null>(null);

  async function handleDelete(index: number) {
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

<div
  class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 divide-y divide-x divide-zinc-700"
>
  {#each items as item, index (item.name)}
    <div class="p-1 grid grid-rows-[auto_1fr] gap-1">
      <ItemWithLabels
              {item}
              class="cursor-pointer"
              labelClassName="border"
              role="button"
              tabindex="0"
              onclick={() => openEditDialog(item)}
              onkeydown={(e: KeyboardEvent) => {
                  if (e.key === "Enter" || e.key === " ") {
                      openEditDialog(item);
                  }
              }}
      />

      <div class="flex flex-wrap items-end gap-2">
        <p>{item.name}</p>
        <button
          onclick={() => {
            handleDelete(index);
          }}
          class="bg-red-700 px-1"
        >
          Delete
        </button>
        <button
          onclick={async () => {
            await revealItemInDir(
              await Promise.all([
                itemImagePath(dataset, item),
                itemLabelPath(dataset, item),
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

<EditDialog item={selectedItem} onClose={closeEditDialog} />