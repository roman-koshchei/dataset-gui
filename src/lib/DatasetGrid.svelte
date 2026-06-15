<script lang="ts">
  import {
    datasetItemKey,
    logPerformance,
    loadAndStoreBatch,
    getFilteredWindow,
    updateStoredItem,
    removeStoredItem,
    resaveAllLabels,
    clearStoredDataset,
    getItemBaseName,
    getItemImageExt,
    itemImagePath,
    itemLabelPath,
    type Dataset,
    type DatasetLoadProgress,
    type DatasetItem,
    type FilterParams,
  } from "./dataset";
  import EditDialog from "./EditDialog.svelte";
  import { numberToTailwindBorder } from "./helpers";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";

  type FilterMode = "all" | "hasBoxes" | "noBoxes" | "hasLabelFile" | "class" | "nth" | "nestedBoxes";
  const OVERSCAN_ROWS = 3;
  const IMAGE_ASPECT_RATIO = 9 / 16;
  const CARD_CHROME_HEIGHT = 44;
  const LOAD_BATCH_SIZE = 500;
  const CACHE_SIZE = 200;
  const CACHE_PREFETCH_MARGIN_ROWS = 5;

  let { dataset }: { dataset: Dataset } = $props();

  let selectedItem = $state<DatasetItem | null>(null);
  let selectedKey = $state<string | null>(null);
  let isLoadingDataset = $state(false);
  let loadError = $state("");
  let saveAllIsActive = $state(false);
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

  let loadId = $state("");
  let totalItems = $state(0);
  let filteredTotal = $state(0);
  let cacheItems = $state<DatasetItem[]>([]);
  let cacheOffset = 0;
  let pendingFetchRequest: Promise<void> | null = null;

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

  function currentFilter(): FilterParams {
    return {
      mode: filterMode,
      classId: filterMode === "class" ? parseInt(filterClassId, 10) || 0 : undefined,
      nth: filterMode === "nth" ? filterNthValue : undefined,
    };
  }

  function resetFilterState() {
    filterMode = "all";
    filterClassId = "0";
    filterNthValue = 1;
    resetScrollPosition();
  }

  function applyFilter() {
    resetScrollPosition();
    if (loadId && !isLoadingDataset) {
      void refreshCache(0);
    }
  }

  function toggleFilterMode(mode: Exclude<FilterMode, "all">) {
    filterMode = filterMode === mode ? "all" : mode;
    applyFilter();
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

  function commitToCache() {
    if (!selectedItem) return;
    const key = datasetItemKey(selectedItem);
    const cacheIdx = cacheItems.findIndex((x) => datasetItemKey(x) === key);
    if (cacheIdx !== -1) {
      cacheItems[cacheIdx] = cloneItem(selectedItem);
    }
    if (loadId) {
      void updateStoredItem(loadId, selectedItem);
    }
  }

  async function refreshCache(newOffset?: number) {
    if (!loadId) return;
    const offset = newOffset ?? Math.max(0, cacheOffset);
    const result = await getFilteredWindow(loadId, currentFilter(), offset, CACHE_SIZE);
    cacheItems = result.items;
    cacheOffset = offset;
    filteredTotal = result.totalFiltered;
  }

  let columnCount = $derived(() => {
    if (containerWidth >= 1024) return 3;
    if (containerWidth >= 768) return 2;
    return 1;
  });

  let rowHeight = $derived(() => {
    const cardWidth = Math.max(1, containerWidth / columnCount());
    return Math.ceil(cardWidth * IMAGE_ASPECT_RATIO + CARD_CHROME_HEIGHT);
  });

  let totalRows = $derived(() => Math.ceil(filteredTotal / columnCount()));
  let virtualStartRow = $derived(() => Math.max(0, Math.floor(scrollTop / rowHeight()) - OVERSCAN_ROWS));
  let virtualEndRow = $derived(() => Math.min(
    totalRows(),
    Math.ceil((scrollTop + viewportHeight) / rowHeight()) + OVERSCAN_ROWS,
  ));

  let virtualRows = $derived(() => {
    const rows: { rowIndex: number; items: DatasetItem[] }[] = [];
    const columns = columnCount();

    for (let rowIndex = virtualStartRow(); rowIndex < virtualEndRow(); rowIndex += 1) {
      const start = rowIndex * columns;
      const rowItems: DatasetItem[] = [];
      for (let col = 0; col < columns; col++) {
        const filteredIndex = start + col;
        const cacheIdx = filteredIndex - cacheOffset;
        if (cacheIdx >= 0 && cacheIdx < cacheItems.length) {
          rowItems.push(cacheItems[cacheIdx]);
        }
      }
      rows.push({ rowIndex, items: rowItems });
    }

    return rows;
  });

  let renderedVirtualItemCount = $derived(() => virtualRows().reduce((count, row) => count + row.items.length, 0));

  function checkCacheRefresh() {
    if (cacheItems.length === 0 || !loadId || pendingFetchRequest) return;

    const visibleStart = virtualStartRow() * columnCount();
    const visibleEnd = virtualEndRow() * columnCount();
    const cacheEnd = cacheOffset + cacheItems.length;
    const margin = columnCount() * CACHE_PREFETCH_MARGIN_ROWS;

    if (visibleStart < cacheOffset + margin || visibleEnd + margin > cacheEnd) {
      const center = Math.max(0, Math.floor((visibleStart + visibleEnd) / 2) - Math.floor(CACHE_SIZE / 2));
      if (center === cacheOffset) return;
      pendingFetchRequest = refreshCache(center).finally(() => { pendingFetchRequest = null; });
    }
  }

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
    checkCacheRefresh();
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
    if (!selectedKey) return undefined;
    const cacheIdx = cacheItems.findIndex((x) => datasetItemKey(x) === selectedKey);
    if (cacheIdx <= 0) return undefined;
    return () => {
      commitToCache();
      selectedKey = datasetItemKey(cacheItems[cacheIdx - 1]);
      selectedItem = cloneItem(cacheItems[cacheIdx - 1]);
    };
  });

  let onNext = $derived(() => {
    if (!selectedKey) return undefined;
    const cacheIdx = cacheItems.findIndex((x) => datasetItemKey(x) === selectedKey);
    if (cacheIdx === -1 || cacheIdx >= cacheItems.length - 1) return undefined;
    return () => {
      commitToCache();
      selectedKey = datasetItemKey(cacheItems[cacheIdx + 1]);
      selectedItem = cloneItem(cacheItems[cacheIdx + 1]);
    };
  });

  async function reloadDataset(resetFilters = false) {
    const newLoadId = crypto.randomUUID();
    const startMs = performance.now();
    activeLoadId = newLoadId;
    isLoadingDataset = true;
    loadError = "";
    totalItems = 0;
    filteredTotal = 0;
    cacheItems = [];
    cacheOffset = 0;
    loadId = "";
    loadProgress = { loadId: newLoadId, phase: "Starting", loaded: 0, total: 0 };
    resetScrollPosition();
    selectedItem = null;
    if (resetFilters) {
      resetFilterState();
    }

    try {
      logPerformance(`DatasetGrid reload started: loadId=${newLoadId} dirs=${dataset.dirs.length}`);
      loadProgress = { loadId: newLoadId, phase: "Scanning files", loaded: 0, total: dataset.dirs.length };

      const total = await invoke<number>("prepare_dataset_load", { loadId: newLoadId, dirs: dataset.dirs });
      totalItems = total;
      logPerformance(`Prepared dataset load: total=${total} elapsedMs=${Math.round(performance.now() - startMs)}`);

      let loaded = 0;
      loadProgress = { loadId: newLoadId, phase: "Loading labels", loaded, total };

      for (let offset = 0; offset < total; offset += LOAD_BATCH_SIZE) {
        const count = await loadAndStoreBatch(
          newLoadId,
          offset,
          Math.min(LOAD_BATCH_SIZE, total - offset),
        );
        loaded += count;
        loadProgress = { loadId: newLoadId, phase: "Loading labels", loaded, total };
        logPerformance(`Loaded and stored batch: loaded=${loaded}/${total}`);
        if (count === 0) break;
        if (newLoadId !== activeLoadId) return;
      }

      await invoke("clear_prepared_dataset_load", { loadId: newLoadId }).catch(() => {});

      if (newLoadId !== activeLoadId) return;

      loadId = newLoadId;
      await refreshCache(0);

      isLoadingDataset = false;
      loadProgress = null;

      await tick();
      if (newLoadId !== activeLoadId) return;
      logPerformance(`DatasetGrid data load complete: loadId=${newLoadId} items=${cacheItems.length} filteredItems=${filteredTotal} totalItems=${totalItems} columns=${columnCount()} rowHeight=${rowHeight()} elapsedMs=${Math.round(performance.now() - startMs)}`);
    } catch (err) {
      if (newLoadId !== activeLoadId) return;
      loadError = err instanceof Error ? err.message : String(err);
      isLoadingDataset = false;
      loadProgress = null;
      logPerformance(`DatasetGrid reload failed: loadId=${newLoadId} elapsedMs=${Math.round(performance.now() - startMs)} error=${loadError}`);
    }
  }

  async function handleDelete(itemToDelete: DatasetItem) {
    try {
      const baseName = getItemBaseName(itemToDelete.name);
      const ext = getItemImageExt(itemToDelete);
      await invoke("delete_dataset_item", {
        imagesDir: itemToDelete.imagesDir,
        labelsDir: itemToDelete.labelsDir,
        name: baseName,
        imageExt: ext,
      });
      if (loadId) {
        await removeStoredItem(loadId, itemToDelete.name, itemToDelete.imagesDir);
        totalItems -= 1;
        await refreshCache(cacheOffset);
      }
    } catch (err) {
      alert(`Failed to delete ${itemToDelete.name}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function openEditDialog(item: DatasetItem) {
    selectedKey = datasetItemKey(item);
    selectedItem = cloneItem(item);
  }

  function closeEditDialog() {
    commitToCache();
    selectedItem = null;
    selectedKey = null;
  }

  onMount(() => {
    void reloadDataset();

    return () => {
      activeLoadId = "";
      if (pendingScrollAnimation) {
        cancelAnimationFrame(pendingScrollAnimation);
        pendingScrollAnimation = 0;
      }
      if (loadId) {
        void clearStoredDataset(loadId);
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
          if (loadId) {
            await resaveAllLabels(loadId);
          }
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

    <button
      class="px-3 border-r border-zinc-700 py-1 {filterMode === 'nestedBoxes' ? 'bg-zinc-600' : ''}"
      onclick={() => toggleFilterMode("nestedBoxes")}
    >
      Nested
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
        oninput={applyFilter}
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
        oninput={applyFilter}
        placeholder="1"
        class="w-12 bg-zinc-800 border border-zinc-600 rounded px-1 text-center"
      />
    </div>

    <span class="px-3 py-1 text-zinc-500">
      {#if isLoadingDataset}
        {loadProgressText()}
      {:else}
        Rendering {renderedVirtualItemCount()} / {filteredTotal} matched / {totalItems} total
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
    {:else if filteredTotal === 0}
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
                      class="relative aspect-video w-full overflow-hidden"
                      onclick={() => openEditDialog(item)}
                    >
                      <img
                        class="block w-full h-full object-contain"
                        width={1280}
                        height={720}
                        src={item.imageSrc}
                        alt=""
                        loading="eager"
                        decoding="async"
                      />

                      {#each item.labels as label}
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
