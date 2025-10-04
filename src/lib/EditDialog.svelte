<script lang="ts">
  import {
    resaveLabelsToFile,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { numberToTailwindBg, numberToTailwindBorder } from "./helpers";
  import ResizeHandles from "$lib/ResizeHandles.svelte";

  let {
    dataset,
    item = $bindable(),
    onClose,
  }: {
    dataset: Dataset;
    item: DatasetItem | null;
    onClose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  let selectedLabelIndex = $state(-1);

  let imageContainer = $state<HTMLDivElement | undefined>(undefined);
  let imageContainerRect = $derived(
    imageContainer ? imageContainer.getBoundingClientRect() : null
  );

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

  $effect(() => {
    if (!dialog) return;

    if (item != null) {
      dialog.showModal();
    } else {
      dialog.close();
    }
  });

  function handleClose() {
    onClose();
    selectedLabelIndex = -1;
  }

  function isSelectedLabelIndexValid() {
    if (!item) return false;
    return selectedLabelIndex >= 0 && selectedLabelIndex < item.labels.length;
  }

  function handleMouseDown(e: MouseEvent, labelIndex: number, handle?: string) {
    selectedLabelIndex = labelIndex;

    if (!imageContainerRect || !item) return;

    e.stopPropagation();
    e.preventDefault();

    dragStartX =
      (e.clientX - imageContainerRect.left) / imageContainerRect.width;
    dragStartY =
      (e.clientY - imageContainerRect.top) / imageContainerRect.height;

    const label = item.labels[labelIndex];
    dragStartLabel = { ...label };

    if (handle) {
      mouseAction = { type: "resizing", handle: handle as any };
    } else {
      mouseAction = { type: "dragging" };
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (
      !imageContainerRect ||
      !item ||
      !mouseAction ||
      !isSelectedLabelIndexValid()
    ) {
      return;
    }

    const currentX =
      (e.clientX - imageContainerRect.left) / imageContainerRect.width;
    const currentY =
      (e.clientY - imageContainerRect.top) / imageContainerRect.height;

    const deltaX = currentX - dragStartX;
    const deltaY = currentY - dragStartY;

    const label = item.labels[selectedLabelIndex];

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
    mouseAction = null;
  }
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<dialog
  bind:this={dialog}
  class="hidden open:grid grid-cols-[1fr_20rem] h-full w-full outline-none m-auto border border-zinc-700 bg-zinc-900 backdrop:bg-zinc-900/75"
>
  {#if item}
    <div class="h-full w-full flex justify-center items-center overflow-hidden">
      <div
        bind:this={imageContainer}
        class="relative h-fit bg-amber-100/20 w-auto"
      >
        <img
          class="w-full h-full max-h-[95vh] object-contain"
          src={item.imageSrc}
          alt=""
          loading="lazy"
        />

        {#each item.labels as label, labelIndex}
          <button
            aria-label={`Bounding box ${labelIndex}`}
            class={[
              "absolute",
              numberToTailwindBorder(label.classId),
              numberToTailwindBg(label.classId),
              selectedLabelIndex === labelIndex ? "border-2" : "border",
            ]}
            style:left={`${label.left * 100}%`}
            style:top={`${label.top * 100}%`}
            style:width={`${label.width * 100}%`}
            style:height={`${label.height * 100}%`}
            onmousedown={(e) => handleMouseDown(e, labelIndex)}
          >
            {#if selectedLabelIndex === labelIndex}
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
      <button
        class="py-2 px-3 bg-zinc-200 hover:bg-zinc-300"
        onclick={handleClose}
      >
        Close
      </button>

      <label class="block text-white">
        Class ID
        <input
          type="number"
          class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
          bind:value={
            () =>
              isSelectedLabelIndexValid()
                ? item.labels[selectedLabelIndex].classId
                : null,
            (v) => {
              if (isSelectedLabelIndexValid()) {
                item.labels[selectedLabelIndex].classId = v ?? 0;
              }
            }
          }
          step={1}
          disabled={!isSelectedLabelIndexValid()}
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
                isSelectedLabelIndexValid()
                  ? item.labels[selectedLabelIndex].top
                  : null,
              (v) => {
                if (isSelectedLabelIndexValid()) {
                  item.labels[selectedLabelIndex].top = v ?? 0;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedLabelIndexValid()}
          />
        </label>

        <label class="block text-white">
          Left
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedLabelIndexValid()
                  ? item.labels[selectedLabelIndex].left
                  : null,
              (v) => {
                if (isSelectedLabelIndexValid()) {
                  item.labels[selectedLabelIndex].left = v ?? 0;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedLabelIndexValid()}
          />
        </label>

        <label class="block text-white">
          Width
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedLabelIndexValid()
                  ? item.labels[selectedLabelIndex].width
                  : null,
              (v) => {
                if (isSelectedLabelIndexValid()) {
                  item.labels[selectedLabelIndex].width = v ?? 0;
                }
              }
            }
            step={0.001}
            disabled={!isSelectedLabelIndexValid()}
            min={0}
            max={1}
          />
        </label>

        <label class="block text-white">
          Height
          <input
            type="number"
            class="mt-1 w-full px-3 py-2 border border-zinc-700 focus:bg-zinc-800 transition-colors"
            bind:value={
              () =>
                isSelectedLabelIndexValid()
                  ? item.labels[selectedLabelIndex].height
                  : null,
              (v) => {
                if (isSelectedLabelIndexValid()) {
                  item.labels[selectedLabelIndex].height = v ?? 0;
                }
              }
            }
            step={0.001}
            min={0}
            max={1}
            disabled={!isSelectedLabelIndexValid()}
          />
        </label>
      </div>

      <button
        class="block py-2 px-3 bg-green-600 text-white hover:bg-green-700"
        onclick={async () => {
          await resaveLabelsToFile(dataset, item);
        }}
      >
        Save changes
      </button>

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
          selectedLabelIndex = item.labels.length - 1;
        }}
      >
        Add label
      </button>

      <button
        class="block py-2 px-3 bg-red-600 text-white hover:bg-red-700"
        onclick={() => {
          if (isSelectedLabelIndexValid()) {
            item.labels.splice(selectedLabelIndex, 1);
            selectedLabelIndex = -1;
          }
        }}
      >
        Delete Selected label
      </button>
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
    -moz-appearance: textfield;
  }
</style>
