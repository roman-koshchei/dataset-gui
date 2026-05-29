<script lang="ts">
  import {
    normArea,
    resaveLabelsToFile,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { numberToTailwindBg, numberToTailwindBorder, numberToOutlineColor } from "./helpers";
  import ResizeHandles from "$lib/ResizeHandles.svelte";

  let {
    dataset,
    item = $bindable(),
    onClose,
    onPrev,
    onNext,
  }: {
    dataset: Dataset;
    item: DatasetItem | null;
    onClose: () => void;
    onPrev?: () => void;
    onNext?: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  let selectedLabelIndices = $state(new Set<number>());
  let hasUnsavedChanges = $state(false);
  let saveStatus = $state<"saving" | "saved" | "error" | null>(null);

  let imageContainer = $state<HTMLDivElement | undefined>(undefined);
  let imgEl = $state<HTMLImageElement | undefined>(undefined);
  let viewportEl = $state<HTMLDivElement | undefined>(undefined);
  let imageNaturalWidth = $state(0);
  let imageNaturalHeight = $state(0);
  let imageFitWidth = $state(0);
  let imageFitHeight = $state(0);
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let isPanning = $state(false);
  let spaceHeld = $state(false);
  let panStartMouseX = 0;
  let panStartMouseY = 0;
  let panStartPanX = 0;
  let panStartPanY = 0;

  // UI doesn't need reactive updates for those
  let mouseAction:
    | { type: "dragging" }
    | {
        type: "resizing";
        handle: "tl" | "tr" | "bl" | "br" | "t" | "b" | "l" | "r";
      }
    | null = null;
  let dragStartX = 0;
  let dragStartY = 0;

  let dragStartLabel = { left: 0, top: 0, width: 0, height: 0 };

  let clipboard: { left: number; top: number; width: number; height: number; classId: number } | null = null;
  let lastMouseNormX = -1;
  let lastMouseNormY = -1;

  $effect(() => {
    if (!dialog) return;

    if (item != null) {
      if (!dialog.open) {
        dialog.showModal();
      }
    } else {
      dialog.close();
    }
  });

  $effect(() => {
    const autosaveInterval = setInterval(() => {
      if (hasUnsavedChanges && saveStatus !== "saving") {
        performSave();
      }
    }, 300);

    return () => clearInterval(autosaveInterval);
  });

  $effect(() => {
    const v = viewportEl;
    if (!v) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (!imageContainer) return;
      const vr = v.getBoundingClientRect();
      const mouseX = e.clientX - vr.left;
      const mouseY = e.clientY - vr.top;
      const oldZoom = zoom;
      const factor = e.deltaY > 0 ? 0.9 : 1 / 0.9;
      const newZoom = Math.max(0.5, Math.min(20, zoom * factor));
      panX = mouseX - (mouseX - panX) * (newZoom / oldZoom);
      panY = mouseY - (mouseY - panY) * (newZoom / oldZoom);
      zoom = newZoom;
    };

    const onMouseDown = (e: MouseEvent) => {
      if (e.button === 1 || (e.button === 0 && (e.ctrlKey || spaceHeld))) {
        e.preventDefault();
        e.stopPropagation();
        isPanning = true;
        panStartMouseX = e.clientX;
        panStartMouseY = e.clientY;
        panStartPanX = panX;
        panStartPanY = panY;
      }
    };

    v.addEventListener("wheel", onWheel, { passive: false });
    v.addEventListener("mousedown", onMouseDown, true);
    return () => {
      v.removeEventListener("wheel", onWheel);
      v.removeEventListener("mousedown", onMouseDown, true);
    };
  });

  $effect(() => {
    if (item) {
      imageNaturalWidth = 0;
      imageNaturalHeight = 0;
      imageFitWidth = 0;
      imageFitHeight = 0;
      zoom = 1;
      panX = 0;
      panY = 0;
    }
  });

  $effect(() => {
    const v = viewportEl;
    if (!v) return;

    const resizeObserver = new ResizeObserver(() => updateImageFit());
    resizeObserver.observe(v);
    updateImageFit();

    return () => resizeObserver.disconnect();
  });

  async function handleClose() {
    onClose();
    clearSelection();
    if (hasUnsavedChanges && saveStatus !== "saving") {
      await performSave();
    }
  }

  async function navigate(callback: () => void) {
    if (hasUnsavedChanges && saveStatus !== "saving") {
      await performSave();
    }
    clearSelection();
    saveStatus = null;
    callback();
  }

  async function performSave() {
    if (!item) return;

    saveStatus = "saving";

    try {
      await resaveLabelsToFile(dataset, item);
      saveStatus = "saved";
      hasUnsavedChanges = false;
    } catch (error) {
      saveStatus = "error";
      console.error("Failed to save labels:", error);
    }
  }

  function centerImage() {
    requestAnimationFrame(() => {
      if (!viewportEl || !imageContainer) return;
      const vr = viewportEl.getBoundingClientRect();
      const iw = imageFitWidth || imageContainer.offsetWidth;
      const ih = imageFitHeight || imageContainer.offsetHeight;
      if (iw === 0 || ih === 0) return;
      panX = (vr.width - iw * zoom) / 2;
      panY = (vr.height - ih * zoom) / 2;
    });
  }

  function updateImageFit() {
    if (!viewportEl || imageNaturalWidth <= 0 || imageNaturalHeight <= 0) return;

    const vr = viewportEl.getBoundingClientRect();
    if (vr.width <= 0 || vr.height <= 0) return;

    const scale = Math.min(vr.width / imageNaturalWidth, vr.height / imageNaturalHeight);
    imageFitWidth = imageNaturalWidth * scale;
    imageFitHeight = imageNaturalHeight * scale;
    centerImage();
  }

  function handleImageLoad(event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    imageNaturalWidth = image.naturalWidth;
    imageNaturalHeight = image.naturalHeight;
    updateImageFit();
  }

  function resetView() {
    zoom = 1;
    centerImage();
  }

  function isSelectedSingle() {
    return selectedLabelIndices.size === 1;
  }

  function isSelectedAny() {
    return selectedLabelIndices.size > 0;
  }

  function getSingleSelectedIndex(): number {
    if (selectedLabelIndices.size !== 1) return -1;
    return selectedLabelIndices.values().next().value as number;
  }

  function isLabelSelected(labelIndex: number): boolean {
    return selectedLabelIndices.has(labelIndex);
  }

  function clearSelection() {
    selectedLabelIndices = new Set();
  }

  function selectLabel(labelIndex: number, ctrlKey: boolean) {
    if (ctrlKey) {
      const next = new Set(selectedLabelIndices);
      if (next.has(labelIndex)) {
        next.delete(labelIndex);
      } else {
        next.add(labelIndex);
      }
      selectedLabelIndices = next;
    } else {
      selectedLabelIndices = new Set([labelIndex]);
    }
  }

  function isSelectedSingleValid() {
    if (!item) return false;
    const idx = getSingleSelectedIndex();
    return idx >= 0 && idx < item.labels.length;
  }

  function handleMouseDown(e: MouseEvent, labelIndex: number, handle?: string) {
    selectLabel(labelIndex, e.ctrlKey || e.metaKey);

    if (selectedLabelIndices.size !== 1) return;
    if (!imageContainer || !item) return;

    e.stopPropagation();
    e.preventDefault();

    const rect = imageContainer.getBoundingClientRect();
    dragStartX = (e.clientX - rect.left) / rect.width;
    dragStartY = (e.clientY - rect.top) / rect.height;

    const idx = getSingleSelectedIndex();
    const label = item.labels[idx];
    dragStartLabel = { ...label };

    if (handle) {
      mouseAction = { type: "resizing", handle: handle as any };
    } else {
      mouseAction = { type: "dragging" };
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (imageContainer) {
      const r = imageContainer.getBoundingClientRect();
      lastMouseNormX = (e.clientX - r.left) / r.width;
      lastMouseNormY = (e.clientY - r.top) / r.height;
    }

    if (isPanning) {
      panX = panStartPanX + (e.clientX - panStartMouseX);
      panY = panStartPanY + (e.clientY - panStartMouseY);
      return;
    }

    if (
      !imageContainer ||
      !item ||
      !mouseAction ||
      selectedLabelIndices.size !== 1
    ) {
      return;
    }

    const rect = imageContainer.getBoundingClientRect();
    const currentX = (e.clientX - rect.left) / rect.width;
    const currentY = (e.clientY - rect.top) / rect.height;

    const deltaX = currentX - dragStartX;
    const deltaY = currentY - dragStartY;

    const idx = getSingleSelectedIndex();
    const label = item.labels[idx];

    if (mouseAction.type === "dragging") {
      label.left = Math.max(
        0,
        Math.min(1 - dragStartLabel.width, dragStartLabel.left + deltaX)
      );
      label.top = Math.max(
        0,
        Math.min(1 - dragStartLabel.height, dragStartLabel.top + deltaY)
      );
    } else if (mouseAction.type === "resizing") {
      if (mouseAction.handle === "tl") {
        const newLeft = Math.max(
          0,
          Math.min(
            dragStartLabel.left + dragStartLabel.width - 0.01,
            dragStartLabel.left + deltaX
          )
        );
        const newTop = Math.max(
          0,
          Math.min(
            dragStartLabel.top + dragStartLabel.height - 0.01,
            dragStartLabel.top + deltaY
          )
        );
        label.width = dragStartLabel.left + dragStartLabel.width - newLeft;
        label.height = dragStartLabel.top + dragStartLabel.height - newTop;
        label.left = newLeft;
        label.top = newTop;
      } else if (mouseAction.handle === "tr") {
        const newTop = Math.max(
          0,
          Math.min(
            dragStartLabel.top + dragStartLabel.height - 0.01,
            dragStartLabel.top + deltaY
          )
        );
        label.width = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.left, dragStartLabel.width + deltaX)
        );
        label.height = dragStartLabel.top + dragStartLabel.height - newTop;
        label.top = newTop;
      } else if (mouseAction.handle === "bl") {
        const newLeft = Math.max(
          0,
          Math.min(
            dragStartLabel.left + dragStartLabel.width - 0.01,
            dragStartLabel.left + deltaX
          )
        );
        label.width = dragStartLabel.left + dragStartLabel.width - newLeft;
        label.height = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.top, dragStartLabel.height + deltaY)
        );
        label.left = newLeft;
      } else if (mouseAction.handle === "br") {
        label.width = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.left, dragStartLabel.width + deltaX)
        );
        label.height = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.top, dragStartLabel.height + deltaY)
        );
      } else if (mouseAction.handle === "t") {
        const newTop = Math.max(
          0,
          Math.min(
            dragStartLabel.top + dragStartLabel.height - 0.01,
            dragStartLabel.top + deltaY
          )
        );
        label.height = dragStartLabel.top + dragStartLabel.height - newTop;
        label.top = newTop;
      } else if (mouseAction.handle === "b") {
        label.height = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.top, dragStartLabel.height + deltaY)
        );
      } else if (mouseAction.handle === "l") {
        const newLeft = Math.max(
          0,
          Math.min(
            dragStartLabel.left + dragStartLabel.width - 0.01,
            dragStartLabel.left + deltaX
          )
        );
        label.width = dragStartLabel.left + dragStartLabel.width - newLeft;
        label.left = newLeft;
      } else if (mouseAction.handle === "r") {
        label.width = Math.max(
          0.01,
          Math.min(1 - dragStartLabel.left, dragStartLabel.width + deltaX)
        );
      }
    }
  }

  function handleMouseUp() {
    if (isPanning) {
      isPanning = false;
      return;
    }
    if (mouseAction !== null && selectedLabelIndices.size === 1) {
      hasUnsavedChanges = true;
    }
    mouseAction = null;
  }
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} onkeydown={(e) => {
  if (e.code === 'Space' && !e.repeat) { e.preventDefault(); spaceHeld = true; }
  if ((e.ctrlKey || e.metaKey) && e.key === 'c' && isSelectedSingleValid() && item) { clipboard = { ...item.labels[getSingleSelectedIndex()] }; }
  if ((e.ctrlKey || e.metaKey) && e.key === 'v' && clipboard && item) { e.preventDefault(); const cx = (lastMouseNormX >= 0 && lastMouseNormX <= 1 && lastMouseNormY >= 0 && lastMouseNormY <= 1) ? Math.max(0, Math.min(1 - clipboard.width, lastMouseNormX - clipboard.width / 2)) : Math.min(clipboard.left + 0.02, 1 - clipboard.width); const cy = (lastMouseNormX >= 0 && lastMouseNormX <= 1 && lastMouseNormY >= 0 && lastMouseNormY <= 1) ? Math.max(0, Math.min(1 - clipboard.height, lastMouseNormY - clipboard.height / 2)) : Math.min(clipboard.top + 0.02, 1 - clipboard.height); item.labels.push({ ...clipboard, left: cx, top: cy }); selectedLabelIndices = new Set([item.labels.length - 1]); hasUnsavedChanges = true; }
  if ((e.key === 'Delete' || e.key === 'Backspace') && isSelectedAny() && item) { e.preventDefault(); const sorted = [...selectedLabelIndices].sort((a, b) => b - a); for (const idx of sorted) { item.labels.splice(idx, 1); } clearSelection(); hasUnsavedChanges = true; }
}} onkeyup={(e) => { if (e.code === 'Space') spaceHeld = false; }} />

<dialog
  bind:this={dialog}
  class="hidden open:grid grid-cols-[1fr_20rem] h-full w-full outline-none m-auto border border-zinc-700 bg-zinc-900 backdrop:bg-zinc-900/75"
>
  {#if item}
    <div bind:this={viewportEl} class="h-full w-full overflow-hidden relative" style="cursor: {isPanning ? 'grabbing' : spaceHeld ? 'grab' : 'default'}">
      <div
        bind:this={imageContainer}
        class="relative"
        style:width={`${imageFitWidth}px`}
        style:height={`${imageFitHeight}px`}
        style="transform-origin: 0 0; transform: translate({panX}px, {panY}px) scale({zoom})"
      >
        <button
          type="button"
          class="block w-full h-full bg-transparent border-0 p-0"
          aria-label="Cancel label selection"
          onclick={() => {
            if (selectedLabelIndices.size > 0) {
              clearSelection();
            }
          }}
        >
          <img
            bind:this={imgEl}
            class="block w-full h-full object-contain pointer-events-none"
            src={item.imageSrc}
            alt=""
            loading="lazy"
            onload={handleImageLoad}
          />
        </button>

        {#each item.labels as label, labelIndex}
          <button
            aria-label={`Bounding box ${labelIndex}`}
            class={[
              "absolute",
              numberToTailwindBg(label.classId),
              isLabelSelected(labelIndex) ? "outline outline-2" : "outline outline-1",
            ]}
            style:left={`${label.left * 100}%`}
            style:top={`${label.top * 100}%`}
            style:width={`${label.width * 100}%`}
            style:height={`${label.height * 100}%`}
            style:outline-color={numberToOutlineColor(label.classId)}
            onmousedown={(e) => handleMouseDown(e, labelIndex)}
            onclick={(e) => e.stopPropagation()}
          >
            {#if selectedLabelIndices.size === 1 && isLabelSelected(labelIndex)}
              <ResizeHandles
                classId={label.classId}
                {labelIndex}
                onHandleMouseDown={handleMouseDown}
              />
            {/if}
          </button>
        {/each}
      </div>
    </div>

    <div class="bg-zinc-900 p-5 border-l border-zinc-700 space-y-3">
      <div class="flex gap-2 items-center">
        <button
          class="py-2 px-3 bg-zinc-200 hover:bg-zinc-300 disabled:opacity-50"
          onclick={() => navigate(() => onPrev?.())}
          disabled={!onPrev}
        >
          Prev
        </button>
        <button
          class="py-2 px-3 bg-zinc-200 hover:bg-zinc-300 disabled:opacity-50"
          onclick={() => navigate(() => onNext?.())}
          disabled={!onNext}
        >
          Next
        </button>
        <button
          class="py-2 px-3 bg-zinc-200 hover:bg-zinc-300"
          onclick={handleClose}
        >
          Close
        </button>
        <span class="ml-auto text-sm text-zinc-400">Labels: {item.labels.length}{#if isSelectedAny()} ({selectedLabelIndices.size} selected){/if}</span>
      </div>

      <div class="flex gap-2 items-center text-white">
        <button
          class="py-1 px-2.5 bg-zinc-700 hover:bg-zinc-600 text-white"
          onclick={() => {
            zoom = Math.max(0.5, zoom / 1.3);
            centerImage();
          }}
        >
          &minus;
        </button>
        <span class="text-sm w-14 text-center">{Math.round(zoom * 100)}%</span>
        <button
          class="py-1 px-2.5 bg-zinc-700 hover:bg-zinc-600 text-white"
          onclick={() => {
            zoom = Math.min(20, zoom * 1.3);
            centerImage();
          }}
        >
          +
        </button>
        <button
          class="py-1 px-2.5 bg-zinc-700 hover:bg-zinc-600 text-white text-sm"
          onclick={resetView}
        >
          Fit
        </button>
      </div>

      <label class="block text-white">
        Class ID
        <input
          type="number"
          class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
          bind:value={
            () =>
              isSelectedSingleValid()
                ? item.labels[getSingleSelectedIndex()].classId
                : null,
            (v) => {
              if (isSelectedSingleValid()) {
                item.labels[getSingleSelectedIndex()].classId = v ?? 0;
                hasUnsavedChanges = true;
              }
            }
          }
          step={1}
          disabled={!isSelectedSingleValid()}
        />
      </label>

      <div class="grid grid-cols-2 gap-2">
        <label class="block text-white">
          Top
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedSingleValid()
                  ? item.labels[getSingleSelectedIndex()].top
                  : null,
              (v) => {
                if (isSelectedSingleValid()) {
                  item.labels[getSingleSelectedIndex()].top = v ?? 0;
                  hasUnsavedChanges = true;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedSingleValid()}
          />
        </label>

        <label class="block text-white">
          Left
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedSingleValid()
                  ? item.labels[getSingleSelectedIndex()].left
                  : null,
              (v) => {
                if (isSelectedSingleValid()) {
                  item.labels[getSingleSelectedIndex()].left = v ?? 0;
                  hasUnsavedChanges = true;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedSingleValid()}
          />
        </label>

        <label class="block text-white">
          Width
          {#if isSelectedSingleValid() && imgEl?.naturalWidth}
            <span class="text-zinc-500 text-xs">
              ({Math.round(item.labels[getSingleSelectedIndex()].width * imgEl.naturalWidth)}px)
            </span>
          {/if}
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedSingleValid()
                  ? item.labels[getSingleSelectedIndex()].width
                  : null,
              (v) => {
                if (isSelectedSingleValid()) {
                  item.labels[getSingleSelectedIndex()].width = v ?? 0;
                  hasUnsavedChanges = true;
                }
              }
            }
            step={0.001}
            disabled={!isSelectedSingleValid()}
            min={0}
            max={1}
          />
        </label>

        <label class="block text-white">
          Height
          {#if isSelectedSingleValid() && imgEl?.naturalHeight}
            <span class="text-zinc-500 text-xs">
              ({Math.round(item.labels[getSingleSelectedIndex()].height * imgEl.naturalHeight)}px)
            </span>
          {/if}
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedSingleValid()
                  ? item.labels[getSingleSelectedIndex()].height
                  : null,
              (v) => {
                if (isSelectedSingleValid()) {
                  item.labels[getSingleSelectedIndex()].height = v ?? 0;
                  hasUnsavedChanges = true;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedSingleValid()}
          />
        </label>
      </div>

      <div class="flex items-center gap-2">
        <button
          class="py-2 px-3 bg-green-600 text-white hover:bg-green-700 disabled:opacity-50"
          onclick={performSave}
          disabled={saveStatus === "saving"}
        >
          Save changes
        </button>
        {#if saveStatus === "saving"}
          <span class="text-yellow-500">Saving...</span>
        {:else if saveStatus === "saved"}
          <span class="text-green-500">Saved</span>
        {:else if saveStatus === "error"}
          <span class="text-red-500">Saving failed</span>
        {/if}
      </div>

      <button
        class="block py-2 px-3 bg-sky-600 text-white hover:bg-blue-700"
        onclick={() => {
          item.labels.push({
            classId: 0,
            top: 0.5,
            left: 0.5,
            height: 0.05,
            width: 0.05,
          });
          selectedLabelIndices = new Set([item.labels.length - 1]);
          hasUnsavedChanges = true;
        }}
      >
        Add label
      </button>

      <button
        class="block py-2 px-3 bg-red-600 text-white hover:bg-red-700 disabled:opacity-50"
        disabled={!isSelectedAny()}
        onclick={() => {
          if (isSelectedAny()) {
            const sorted = [...selectedLabelIndices].sort((a, b) => b - a);
            for (const idx of sorted) {
              item.labels.splice(idx, 1);
            }
            clearSelection();
            hasUnsavedChanges = true;
          }
        }}
      >
        Delete{#if selectedLabelIndices.size > 1} {selectedLabelIndices.size} selected{/if} label{#if selectedLabelIndices.size !== 1}s{/if}
      </button>

      <button
        class="block py-2 px-3 bg-zinc-200 hover:bg-zinc-300 disabled:opacity-50"
        disabled={!isSelectedAny()}
        onclick={() => {
          clearSelection();
        }}
      >
        Cancel selection
      </button>

      {#if isSelectedSingleValid()}
        <p class="block py-2 px-3 bg-zinc-200">
          Normalized area: {normArea(item.labels[getSingleSelectedIndex()])}
        </p>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  input[type="number"]::-webkit-outer-spin-button,
  input[type="number"]::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  input[type="number"] {
    appearance: textfield;
    -moz-appearance: textfield;
  }
</style>
