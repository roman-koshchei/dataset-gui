<script lang="ts">
  import {
    deleteItem,
    datasetItemKey,
    datasetLabelKey,
    itemImagePath,
    itemLabelPath,
    logPerformance,
    loadWholeDataset,
    resaveLabelsToFile,
    type Dataset,
    type DatasetLoadProgress,
    type DatasetItem,
  } from "./dataset";
  import EditDialog from "./EditDialog.svelte";
  import { numberToTailwindBorder } from "./helpers";
  import { onMount, tick } from "svelte";

  type FilterMode = "all" | "hasBoxes" | "noBoxes" | "hasLabelFile" | "class" | "nth";
  const OVERSCAN_ROWS = 3;
  const IMAGE_ASPECT_RATIO = 9 / 16;
  const CARD_CHROME_HEIGHT = 44;

  let { dataset }: { dataset: Dataset } = $props();

  let selectedItem = $state<DatasetItem | null>(null);
  let isLoadingDataset = $state(false);
  let loadError = $state("");
  let saveAllIsActive = $state(false);
  let loadedItems = $state<DatasetItem[]>([]);
  let activeLoadId = "";
  let loadProgress = $state<DatasetLoadProgress | null>(null);
  let scrollContainer = $state<HTMLDivElement | undefined>(undefined);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let containerWidth = $state(0);
  let pendingScrollTop = 0;
  let pendingScrollAnimation = 0;

  let filterMode = $state<FilterMode>("all");
  let filterClassId = $state<string>("0");
  let filterNthValue = $state(1);

  let loadProgressPercent = $derived(() => {
    if (!loadProgress || loadProgress.total <= 0) return null;
    return Math.round((loadProgress.loaded / loadProgress.total) * 100);
  });

  let loadProgressText = $derived(() => {
    if (!loadProgress) return "Starting...";
    if (loadProgress.total <= 0) return loadProgress.phase;
    return `${loadProgress.phase}: ${loadProgress.loaded} / ${loadProgress.total}`;
  });

  async function revealPaths(paths: string[]) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(paths);
  }

  function resetFilterState() {
    filterMode = "all";
    filterClassId = "0";
    filterNthValue = 1;
    resetScrollPosition();
  }

  function toggleFilterMode(mode: Exclude<FilterMode, "all">) {
    filterMode = filterMode === mode ? "all" : mode;
    resetScrollPosition();
  }

  function resetScrollPosition() {
    if (pendingScrollAnimation) {
      cancelAnimationFrame(pendingScrollAnimation);
      pendingScrollAnimation = 0;
    }

    pendingScrollTop = 0;
    scrollTop = 0;
    if (scrollContainer) {
      scrollContainer.scrollTop = 0;
    }
  }

  function cloneItem(item: DatasetItem): DatasetItem {
    return {
      ...item,
      labels: item.labels.map((label) => ({ ...label })),
    };
  }

  function commitItem(updatedItem: DatasetItem) {
    const nextItem = cloneItem(updatedItem);
    const nextItemKey = datasetItemKey(nextItem);
    const loadedIndex = loadedItems.findIndex((x) => datasetItemKey(x) === nextItemKey);
    if (loadedIndex !== -1) {
      loadedItems[loadedIndex] = nextItem;
    }
  }

  function commitSelectedItem() {
    if (!selectedItem) return;
    commitItem(selectedItem);
  }

  let filteredItems = $derived(() => {
    if (filterMode === "all") {
      return loadedItems;
    }

    return loadedItems.filter((item, index) => {
      const hasLabels = item.labels.length > 0;

      if (filterMode === "hasBoxes") {
        return hasLabels;
      }

      if (filterMode === "noBoxes") {
        return !hasLabels;
      }

      if (filterMode === "hasLabelFile") {
        return item.hasLabelFile;
      }

      if (filterMode === "class") {
        const classId = parseInt(filterClassId, 10);
        if (isNaN(classId)) return false;
        return item.labels.some((label) => label.classId === classId);
      }

      if (filterMode === "nth") {
        if (filterNthValue < 1) return false;
        return index % filterNthValue === 0;
      }

      return true;
    });
  });

  let columnCount = $derived(() => {
    if (containerWidth >= 1024) return 3;
    if (containerWidth >= 768) return 2;
    return 1;
  });

  let rowHeight = $derived(() => {
    const cardWidth = Math.max(1, containerWidth / columnCount());
    return Math.ceil(cardWidth * IMAGE_ASPECT_RATIO + CARD_CHROME_HEIGHT);
  });

  let totalRows = $derived(() => Math.ceil(filteredItems().length / columnCount()));
  let virtualStartRow = $derived(() => Math.max(0, Math.floor(scrollTop / rowHeight()) - OVERSCAN_ROWS));
  let virtualEndRow = $derived(() => Math.min(
    totalRows(),
    Math.ceil((scrollTop + viewportHeight) / rowHeight()) + OVERSCAN_ROWS,
  ));

  let virtualRows = $derived(() => {
    const rows: { rowIndex: number; items: DatasetItem[] }[] = [];
    const items = filteredItems();
    const columns = columnCount();

    for (let rowIndex = virtualStartRow(); rowIndex < virtualEndRow(); rowIndex += 1) {
      const start = rowIndex * columns;
      rows.push({
        rowIndex,
        items: items.slice(start, start + columns),
      });
    }

    return rows;
  });

  let renderedVirtualItemCount = $derived(() => virtualRows().reduce((count, row) => count + row.items.length, 0));

  function virtualRangeFor(nextScrollTop: number) {
    const height = rowHeight();
    if (height <= 0) {
      return { start: 0, end: 0 };
    }

    return {
      start: Math.max(0, Math.floor(nextScrollTop / height) - OVERSCAN_ROWS),
      end: Math.min(
        totalRows(),
        Math.ceil((nextScrollTop + viewportHeight) / height) + OVERSCAN_ROWS,
      ),
    };
  }

  function setVirtualScrollTop(nextScrollTop: number, force = false) {
    if (!force) {
      const currentRange = virtualRangeFor(scrollTop);
      const nextRange = virtualRangeFor(nextScrollTop);
      if (currentRange.start === nextRange.start && currentRange.end === nextRange.end) {
        return;
      }
    }

    scrollTop = nextScrollTop;
  }

  function scheduleVirtualScrollUpdate(nextScrollTop: number) {
    pendingScrollTop = nextScrollTop;
    if (pendingScrollAnimation) return;

    pendingScrollAnimation = requestAnimationFrame(() => {
      pendingScrollAnimation = 0;
      setVirtualScrollTop(pendingScrollTop);
    });
  }

  let onPrev = $derived(() => {
    if (!selectedItem) return undefined;
    const items = filteredItems();
    const selectedItemKey = datasetItemKey(selectedItem);
    const idx = items.findIndex((x) => datasetItemKey(x) === selectedItemKey);
    if (idx <= 0) return undefined;
    return () => {
      commitSelectedItem();
      selectedItem = cloneItem(items[idx - 1]);
    };
  });

  let onNext = $derived(() => {
    if (!selectedItem) return undefined;
    const items = filteredItems();
    const selectedItemKey = datasetItemKey(selectedItem);
    const idx = items.findIndex((x) => datasetItemKey(x) === selectedItemKey);
    if (idx === -1 || idx >= items.length - 1) return undefined;
    return () => {
      commitSelectedItem();
      selectedItem = cloneItem(items[idx + 1]);
    };
  });

  async function reloadDataset(resetFilters = false) {
    const loadId = crypto.randomUUID();
    const startMs = performance.now();
    activeLoadId = loadId;
    isLoadingDataset = true;
    loadError = "";
    loadProgress = { loadId, phase: "Starting", loaded: 0, total: 0 };
    loadedItems = [];
    resetScrollPosition();
    selectedItem = null;
    if (resetFilters) {
      resetFilterState();
    }

    try {
      logPerformance(`DatasetGrid reload started: loadId=${loadId} dirs=${dataset.dirs.length}`);
      const nextItems = await loadWholeDataset(dataset, loadId, (progress) => {
        if (progress.loadId !== activeLoadId) return;
        loadProgress = progress;
      });
      if (loadId !== activeLoadId) return;
      logPerformance(`DatasetGrid received metadata: loadId=${loadId} items=${nextItems.length} elapsedMs=${Math.round(performance.now() - startMs)}`);
      loadedItems = nextItems;
      isLoadingDataset = false;
      loadProgress = null;
      await tick();
      if (loadId !== activeLoadId) return;
      logPerformance(`DatasetGrid initial render complete: loadId=${loadId} renderedItems=${renderedVirtualItemCount()} matchedItems=${filteredItems().length} totalItems=${loadedItems.length} columns=${columnCount()} rowHeight=${rowHeight()} elapsedMs=${Math.round(performance.now() - startMs)}`);
    } catch (err) {
      if (loadId !== activeLoadId) return;
      loadError = err instanceof Error ? err.message : String(err);
      loadedItems = [];
      isLoadingDataset = false;
      loadProgress = null;
      logPerformance(`DatasetGrid reload failed: loadId=${loadId} elapsedMs=${Math.round(performance.now() - startMs)} error=${loadError}`);
    }
  }

  async function handleDelete(itemToDelete: DatasetItem) {
    const itemToDeleteKey = datasetItemKey(itemToDelete);
    const index = loadedItems.findIndex((x) => datasetItemKey(x) === itemToDeleteKey);
    if (index === -1) {
      console.error(`Item not found: ${itemToDelete.name}`);
      return;
    }

    try {
      await deleteItem(dataset, loadedItems[index]);
      loadedItems.splice(index, 1);
    } catch (err) {
      alert(`Failed to delete ${itemToDelete.name}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function openEditDialog(item: DatasetItem) {
    selectedItem = cloneItem(item);
  }

  function closeEditDialog() {
    commitSelectedItem();
    selectedItem = null;
  }

  onMount(() => {
    void reloadDataset();

    return () => {
      activeLoadId = "";
      if (pendingScrollAnimation) {
        cancelAnimationFrame(pendingScrollAnimation);
        pendingScrollAnimation = 0;
      }
    };
  });

  $effect(() => {
    if (!scrollContainer) return;

    function updateViewportMetrics() {
      if (!scrollContainer) return;
      containerWidth = scrollContainer.clientWidth;
      viewportHeight = scrollContainer.clientHeight;
      setVirtualScrollTop(scrollContainer.scrollTop, true);
    }

    updateViewportMetrics();
    const resizeObserver = new ResizeObserver(updateViewportMetrics);
    resizeObserver.observe(scrollContainer);

    return () => resizeObserver.disconnect();
  });
</script>

<section class="h-full grid grid-rows-[auto_1fr]">
  <div class="border-b border-zinc-700 flex items-stretch text-sm">
    <button
      class="px-3 border-r border-zinc-700 py-1 disabled:text-zinc-700"
      onclick={() => reloadDataset(true)}
      disabled={isLoadingDataset}
    >
      Reload
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 disabled:text-zinc-700"
      onclick={async () => {
        try {
          saveAllIsActive = true;
          await Promise.all(
            loadedItems
              .filter((item) => item.hasLabelFile || item.labels.length > 0)
              .map((item) => resaveLabelsToFile(dataset, item))
          );
        } catch (err) {
          alert(`Failed to save all changes: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
          saveAllIsActive = false;
        }
      }}
      disabled={saveAllIsActive || isLoadingDataset}
    >
      Save all changes
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterMode === 'hasBoxes' ? 'bg-zinc-600' : ''}"
      onclick={() => toggleFilterMode("hasBoxes")}
    >
      Has boxes
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterMode === 'noBoxes' ? 'bg-zinc-600' : ''}"
      onclick={() => toggleFilterMode("noBoxes")}
    >
      No boxes
    </button>

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterMode === 'hasLabelFile' ? 'bg-zinc-600' : ''}"
      onclick={() => toggleFilterMode("hasLabelFile")}
    >
      Has label file
    </button>

    <div class="px-3 flex items-center gap-2 border-r border-zinc-700 {filterMode === 'class' ? 'bg-zinc-600' : ''}">
      <button
        class="flex items-center gap-2"
        onclick={() => toggleFilterMode("class")}
      >
        Class:
      </button>
      <input
        type="text"
        bind:value={filterClassId}
        oninput={resetScrollPosition}
        placeholder="0"
        class="w-12 bg-zinc-800 border border-zinc-600 rounded px-1 text-center"
      />
    </div>

    <div class="px-3 flex items-center gap-2 border-r border-zinc-700 {filterMode === 'nth' ? 'bg-zinc-600' : ''}">
      <button
        class="flex items-center gap-2"
        onclick={() => toggleFilterMode("nth")}
      >
        Every n-th:
      </button>
      <input
        type="number"
        min="1"
        bind:value={filterNthValue}
        oninput={resetScrollPosition}
        placeholder="1"
        class="w-12 bg-zinc-800 border border-zinc-600 rounded px-1 text-center"
      />
    </div>

    <span class="px-3 py-1 text-zinc-500">
      {#if isLoadingDataset}
        {loadProgressText()}
      {:else}
        Rendering {renderedVirtualItemCount()} / {filteredItems().length} matched / {loadedItems.length} total
      {/if}
    </span>
  </div>

  <div
    bind:this={scrollContainer}
    class="min-h-0 overflow-y-auto"
    onscroll={(event) => scheduleVirtualScrollUpdate(event.currentTarget.scrollTop)}
  >
    {#if isLoadingDataset}
      <div class="h-full grid place-content-center text-zinc-400 p-4">
        <div class="w-[min(28rem,80vw)] space-y-3">
          <p class="text-center text-lg text-zinc-300">Loading dataset...</p>
          <p class="text-center text-sm">{loadProgressText()}</p>
          {#if loadProgressPercent() !== null}
            <div class="h-2 overflow-hidden rounded bg-zinc-800 border border-zinc-700">
              <div
                class="h-full bg-blue-500 transition-[width] duration-150"
                style:width={`${loadProgressPercent()}%`}
              ></div>
            </div>
            <p class="text-center text-xs text-zinc-500">{loadProgressPercent()}%</p>
          {/if}
          {#if loadProgress?.current}
            <p class="truncate text-center text-xs text-zinc-600" title={loadProgress.current}>
              {loadProgress.current}
            </p>
          {/if}
        </div>
      </div>
    {:else if loadError}
      <div class="p-4 text-red-400">
        Failed to load dataset: {loadError}
      </div>
    {:else if filteredItems().length === 0}
      <div class="h-full grid place-content-center text-zinc-500">
        No items match the current filter.
      </div>
    {:else}
      <div class="relative" style:height={`${totalRows() * rowHeight()}px`}>
        <div
          class="absolute inset-x-0 top-0 will-change-transform"
          style:transform={`translateY(${virtualStartRow() * rowHeight()}px)`}
        >
          {#each virtualRows() as row (row.rowIndex)}
            <div
              class="grid divide-x divide-zinc-700 border-b border-zinc-700"
              style:height={`${rowHeight()}px`}
              style:grid-template-columns={`repeat(${columnCount()}, minmax(0, 1fr))`}
            >
              {#each row.items as item (datasetItemKey(item))}
                <div class="p-1 h-full grid grid-rows-[auto_2rem] gap-1 overflow-hidden">
                  <button
                    class="relative overflow-hidden aspect-video w-full"
                    onclick={() => openEditDialog(item)}
                  >
                    <img
                      class="w-full h-full object-contain"
                      width={1280}
                      height={720}
                      src={item.imageSrc}
                      alt=""
                      loading="eager"
                      decoding="async"
                    />

                    {#each item.labels as label (datasetLabelKey(label))}
                      <div
                        class={[
                          "absolute pointer-events-none border-2",
                          numberToTailwindBorder(label.classId),
                        ]}
                        style:left={`${label.left * 100}%`}
                        style:top={`${label.top * 100}%`}
                        style:width={`${label.width * 100}%`}
                        style:height={`${label.height * 100}%`}
                      ></div>
                    {/each}
                  </button>

                  <div class="min-w-0 flex items-center gap-2 text-sm overflow-hidden">
                    <p class="min-w-0 flex-1 truncate" title={item.name}>{item.name}</p>
                    <button
                      onclick={() => {
                        handleDelete(item);
                      }}
                      class="bg-red-700 px-1 shrink-0"
                    >
                      Delete
                    </button>
                    <button
                      class="bg-zinc-700 px-1 shrink-0"
                      onclick={async () => {
                        await revealPaths(
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
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>

<EditDialog {dataset} bind:item={selectedItem} onClose={closeEditDialog} onPrev={onPrev()} onNext={onNext()} />
