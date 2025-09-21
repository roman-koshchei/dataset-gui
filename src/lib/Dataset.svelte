<script lang="ts">
  import type { Dataset, DatasetItem } from "./dataset";
  let { dataset, items }: { dataset: Dataset; items: DatasetItem[] } = $props();

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

<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
  {#each items as item}
    <div>
      <div class="relative">
        <img
          class="w-full h-auto"
          width={1280}
          height={720}
          src={item.imageSrc}
          alt=""
        />

        {#each item.labels as label, i}
          <div
            class={[
              "absolute border",
              numberToTailwindBorder(i),
              numberToTailwindBg(i),
            ]}
            style:left={`${label.left * 100}%`}
            style:top={`${label.top * 100}%`}
            style:width={`${label.width * 100}%`}
            style:height={`${label.height * 100}%`}
          ></div>
        {/each}
      </div>

      <div class="flex flex-wrap gap-1">
        <p>{item.name}</p>
        <button>Delete</button>
      </div>
    </div>
  {/each}
</div>
