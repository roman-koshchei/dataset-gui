<script lang="ts">
    import type { DatasetItem } from "./dataset";
    import { numberToTailwindBg, numberToTailwindBorder } from "./helper";

    let {
        item = $bindable(),
        onClose,
    }: {
        item: DatasetItem | null;
        onClose: () => void;
    } = $props();

    let dialog: HTMLDialogElement;

    $effect(() => {
        if (!dialog) return;
        if (!item) {
            dialog.close();
            return;
        }
        dialog.showModal();
    });

    function handleClose() {
        onClose();
    }
</script>

<dialog bind:this={dialog} class="fixed top-2/4 left-2/4 -translate-x-1/2 -translate-y-1/2 backdrop:bg-stone-900/75">
    {#if item}
        <div class="grid grid-cols-[1fr_20rem] w-max h-full">
            <div class="relative">
                <img
                    class="w-full h-auto"
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

            <div class="bg-zinc-900 p-5">
                <button
                        class="py-2 px-3 bg-zinc-200 text-black cursor-pointer"
                        onclick={handleClose}
                >
                    Close
                </button>
            </div>
        </div>
    {/if}
</dialog>
