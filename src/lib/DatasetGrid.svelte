<script lang="ts">
  import {
    deleteItem,
    itemImagePath,
    itemLabelPath,
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
      <button class="relative" onclick={() => openEditDialog(item)}>
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
            handleDelete(index);
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

<EditDialog item={selectedItem} onClose={closeEditDialog} />
