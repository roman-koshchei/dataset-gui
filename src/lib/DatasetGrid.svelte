<script lang="ts">
  import {
    deleteItem,
    itemImagePath,
    itemLabelPath,
    loadWholeDataset,
    resaveLabelsToFile,
    getDatasetCount,
    loadDatasetBatch,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import EditDialog from "./EditDialog.svelte";
  import { numberToTailwindBorder, numberToTailwindBg } from "./helpers";
  import { onMount } from "svelte";

  let {
    dataset,
    items = $bindable(),
  }: { dataset: Dataset; items?: DatasetItem[] } = $props();

  let selectedItem = $state<DatasetItem | null>(null);
  let reloadIsActive = $state(false);
  let saveAllIsActive = $state(false);

  let loadedItems = $state<DatasetItem[]>([]);
  let isLoadingMore = $state(false);
  let hasMore = $state(true);
  let batchOffset = $state(0);
  let totalItemCount = $state<number | null>(null);

  let filterHasBoxes = $state(false);
  let filterNoBoxes = $state(false);
  let filterByClass = $state(false);
  let filterClassId = $state<string>("0");

  let filteredItems = $derived(() => {
    if (!filterHasBoxes && !filterNoBoxes && !filterByClass) {
      return loadedItems;
    }

    return loadedItems.filter(item => {
      const hasLabels = item.labels && item.labels.length > 0;

      if (filterHasBoxes) {
        return hasLabels;
      }

      if (filterNoBoxes) {
        return !hasLabels;
      }

      if (filterByClass) {
        const classId = parseInt(filterClassId, 10);
        if (isNaN(classId)) return false;
        return item.labels.some(label => label.classId === classId);
      }

      return true;
    });
  });

  let scrollContainer: HTMLDivElement | undefined = undefined;
  let sentinel: HTMLDivElement | undefined = undefined;
  let observer: IntersectionObserver | null = null;

  const BATCH_SIZE = 250;

  async function loadMoreItems() {
    if (isLoadingMore || !hasMore) {
      console.log('loadMoreItems called but skipping:', { isLoadingMore, hasMore, loadedItems: loadedItems.length, totalItemCount });
      return;
    }

    console.log('loadMoreItems starting:', { batchOffset, loadedItems: loadedItems.length, totalItemCount });
    isLoadingMore = true;
    try {
      const newItems = await loadDatasetBatch(dataset, batchOffset, BATCH_SIZE);
      console.log('loadMoreItems loaded:', newItems.length, 'items');
      loadedItems = [...loadedItems, ...newItems];
      batchOffset += newItems.length;

      if (totalItemCount !== null && loadedItems.length >= totalItemCount) {
        hasMore = false;
        console.log('loadMoreItems: all items loaded');
      } else if (newItems.length < BATCH_SIZE) {
        hasMore = false;
        console.log('loadMoreItems: no more items available');
      }
    } catch (err) {
      console.error(`Failed to load batch: ${err instanceof Error ? err.message : String(err)}`);
      alert(`Failed to load more items: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      isLoadingMore = false;
    }
  }

  async function handleDelete(name: string) {
    const index = loadedItems.findIndex((x) => x.name === name);
    if (index === -1) {
      console.error(`Item not found: ${name}`);
      return;
    }
    const item = loadedItems[index];
    try {
      await deleteItem(dataset, item);
      loadedItems.splice(index, 1);
      if (items) {
        const itemsIndex = items.findIndex((x) => x.name === name);
        if (itemsIndex !== -1) {
          items.splice(itemsIndex, 1);
        }
      }
    } catch (err) {
      alert(`Failed to delete ${name}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function openEditDialog(item: DatasetItem) {
    selectedItem = item;
  }

  function closeEditDialog() {
    selectedItem = null;
  }

  $effect(() => {
    if (items && items.length > 0 && loadedItems.length === 0) {
      loadedItems = items;
    }
  });

  onMount(async () => {
    try {
      const count = await getDatasetCount(dataset);
      totalItemCount = count;
      if (count === 0) {
        hasMore = false;
      }
    } catch (err) {
      console.error(`Failed to get dataset count: ${err}`);
    }

    loadMoreItems();
  });

  $effect(() => {
    if (scrollContainer && sentinel) {
      if (observer) {
        observer.disconnect();
      }

      observer = new IntersectionObserver(
        (entries) => {
          console.log('IntersectionObserver callback:', entries[0].isIntersecting, entries[0]);
          if (entries[0].isIntersecting) {
            loadMoreItems();
          }
        },
        {
          root: scrollContainer,
          rootMargin: '1600px',
          threshold: 0.01
        }
      );

      observer.observe(sentinel);
    }

    return () => {
      if (observer) {
        observer.disconnect();
      }
    };
  });
</script>

<section class="h-full grid grid-rows-[auto_1fr]">
  <div class="border-b border-zinc-700 flex items-stretch text-sm">
    <button
      class="px-3 border-r border-zinc-700 py-1 disabled:text-zinc-700"
      onclick={async () => {
        try {
          reloadIsActive = true;
          
          loadedItems = [];
          batchOffset = 0;
          hasMore = true;
          
          const count = await getDatasetCount(dataset);
          totalItemCount = count;
          
          filterHasBoxes = false;
          filterNoBoxes = false;
          filterByClass = false;
          filterClassId = "0";
          
          if (items) {
            items = [];
          }
          
          await loadMoreItems();
        } catch (err) {
          alert(`Failed to reload dataset: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
          reloadIsActive = false;
        }
      }}
      disabled={reloadIsActive}
    >
      Reload
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 disabled:text-zinc-700"
      onclick={async () => {
        try {
          saveAllIsActive = true;
          await Promise.all(loadedItems.map((x) => resaveLabelsToFile(dataset, x)));
        } catch (err) {
          alert(`Failed to save all changes: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
          saveAllIsActive = false;
        }
      }}
      disabled={saveAllIsActive}
    >
      Save all changes
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterHasBoxes ? 'bg-zinc-600' : ''}"
      onclick={() => {
        filterHasBoxes = !filterHasBoxes;
        filterNoBoxes = false;
        filterByClass = false;
      }}
    >
      Has boxes
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterNoBoxes ? 'bg-zinc-600' : ''}"
      onclick={() => {
        filterNoBoxes = !filterNoBoxes;
        filterHasBoxes = false;
        filterByClass = false;
      }}
    >
      No boxes
    </button>

    <div class="px-3 flex items-center gap-2 border-r border-zinc-700 {filterByClass ? 'bg-zinc-600' : ''}">
      <button
        class="flex items-center gap-2"
        onclick={() => {
          filterByClass = !filterByClass;
          filterHasBoxes = false;
          filterNoBoxes = false;
        }}
      >
        Class:
      </button>
      <input
        type="text"
        bind:value={filterClassId}
        placeholder="0"
        class="w-12 bg-zinc-800 border border-zinc-600 rounded px-1 text-center"
      />
    </div>
  </div>

  <div
    bind:this={scrollContainer}
    class="overflow-y-auto grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 divide-y divide-x divide-zinc-700"
  >
    {#each filteredItems() as item (item.name)}
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
    <div bind:this={sentinel} class="col-span-full h-10 flex items-center justify-center">
      {#if isLoadingMore}
        <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-white"></div>
      {/if}
    </div>
  </div>
</section>

<EditDialog {dataset} bind:item={selectedItem} onClose={closeEditDialog} />
