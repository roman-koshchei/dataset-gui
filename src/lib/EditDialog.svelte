<script lang="ts">
  import type { DatasetItem } from "./dataset";
  import { numberToTailwindBg, numberToTailwindBorder } from "./helper";

  let {
    item,
    onClose,
  }: {
    item: DatasetItem | null;
    onClose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;

  $effect(() => {
    if (!dialog) return;

    if (item != null) {
      console.log("show modal", JSON.stringify(item));
      console.log("item:", JSON.stringify(item));
      dialog.showModal();
    } else {
      console.log("close", dialog);
      dialog.close();
    }
  });

  function handleClose() {
    onClose();
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

        {#each item.labels as label}
          <div
            class={[
              "absolute border-2",
              numberToTailwindBorder(label.classId),
              numberToTailwindBg(label.classId),
            ]}
            style:left={`${label.left * 100}%`}
            style:top={`${label.top * 100}%`}
            style:width={`${label.width * 100}%`}
            style:height={`${label.height * 100}%`}
          ></div>
        {/each}
      </div>
    </div>

    <div class="bg-zinc-900 p-5 border-l border-zinc-700">
      <button class="py-2 px-3 bg-zinc-200 text-black" onclick={handleClose}>
        Close
      </button>
    </div>
  {/if}
</dialog>
