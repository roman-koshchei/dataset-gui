<script lang="ts">
  import {
    resaveLabelsToFile,
    type Dataset,
    type DatasetItem,
  } from "./dataset";
  import { numberToTailwindBg, numberToTailwindBorder } from "./helper";

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
</script>

<dialog
  bind:this={dialog}
  class="hidden open:grid grid-cols-[1fr_20rem] h-full w-full outline-none m-auto border border-zinc-700 bg-zinc-900 backdrop:bg-zinc-900/75"
>
  {#if item}
    <div class="h-full w-full flex justify-center items-center">
      <div class="relative h-fit bg-amber-100/20 w-auto">
        <img
          class="w-full h-full max-h-[95vh] object-contain"
          src={item.imageSrc}
          alt=""
          loading="lazy"
        />

        {#each item.labels as label, labelIndex}
          <button
            onclick={() => {
              selectedLabelIndex = labelIndex;
            }}
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
          ></button>
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
            top: 0.1,
            left: 0.1,
            height: 0.5,
            width: 0.5,
          });
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
