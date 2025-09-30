<script lang="ts">
    import type { DatasetItem } from "./dataset";
    import ItemWithLabels from "$lib/ItemWithLabels.svelte";

    let {
        item = $bindable(),
        onClose,
    }: {
        item: DatasetItem | null;
        onClose: () => void;
    } = $props();

    let dialog: HTMLDialogElement;

    $effect(() => {
        if (item && dialog) {
            dialog.showModal();
        } else if (!item && dialog) {
            dialog.close();
        }
    });

    function handleClose() {
        onClose();
    }

    function numberToTailwindBorder(n: number) {
        const borderClasses = [
            "border-red-500",
            "border-orange-500",
            "border-amber-500",
            "border-yellow-500",
            "border-lime-500",
            "border-green-500",
            "border-emerald-500",
            "border-teal-500",
            "border-cyan-500",
            "border-sky-500",
            "border-blue-500",
            "border-indigo-500",
            "border-violet-500",
            "border-purple-500",
            "border-fuchsia-500",
            "border-pink-500",
            "border-rose-500",
        ];

        const index = Math.abs(n) % borderClasses.length;
        return borderClasses[index];
    }

    function numberToTailwindBg(n: number) {
        const bgClasses = [
            "bg-red-500/20",
            "bg-orange-500/20",
            "bg-amber-500/20",
            "bg-yellow-500/20",
            "bg-lime-500/20",
            "bg-green-500/20",
            "bg-emerald-500/20",
            "bg-teal-500/20",
            "bg-cyan-500/20",
            "bg-sky-500/20",
            "bg-blue-500/20",
            "bg-indigo-500/20",
            "bg-violet-500/20",
            "bg-purple-500/20",
            "bg-fuchsia-500/20",
            "bg-pink-500/20",
            "bg-rose-500/20",
        ];

        const index = Math.abs(n) % bgClasses.length;
        return bgClasses[index];
    }
</script>

<dialog bind:this={dialog} class="fixed top-2/4 left-2/4 -translate-x-1/2 -translate-y-1/2 backdrop:bg-stone-900/75">
    {#if item}
        <div class="grid grid-cols-[1fr_20rem] w-max h-full">
            <ItemWithLabels {item} labelClassName="border-2" />

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
