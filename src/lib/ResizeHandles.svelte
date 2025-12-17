<script lang="ts">
  import { numberToTailwindBorder } from "./helpers";

  let {
    classId,
    labelIndex,
    onHandleMouseDown,
  }: {
    classId: number;
    labelIndex: number;
    onHandleMouseDown: (
      e: MouseEvent,
      labelIndex: number,
      handle: string
    ) => void;
  } = $props();

  const handles = [
    {
      position: "tl",
      cursor: "cursor-nw-resize",
      label: "Resize top-left",
      classes: "-left-1.5 -top-1.5",
    },
    {
      position: "tr",
      cursor: "cursor-ne-resize",
      label: "Resize top-right",
      classes: "-right-1.5 -top-1.5",
    },
    {
      position: "bl",
      cursor: "cursor-sw-resize",
      label: "Resize bottom-left",
      classes: "-left-1.5 -bottom-1.5",
    },
    {
      position: "br",
      cursor: "cursor-se-resize",
      label: "Resize bottom-right",
      classes: "-right-1.5 -bottom-1.5",
    },
    {
      position: "t",
      cursor: "cursor-n-resize",
      label: "Resize top",
      classes: "left-1/2 -translate-x-1/2 -top-1.5",
    },
    {
      position: "b",
      cursor: "cursor-s-resize",
      label: "Resize bottom",
      classes: "left-1/2 -translate-x-1/2 -bottom-1.5",
    },
    {
      position: "l",
      cursor: "cursor-w-resize",
      label: "Resize left",
      classes: "-left-1.5 top-1/2 -translate-y-1/2",
    },
    {
      position: "r",
      cursor: "cursor-e-resize",
      label: "Resize right",
      classes: "-right-1.5 top-1/2 -translate-y-1/2",
    },
  ];
</script>

{#each handles as handle}
  <div
    role="button"
    tabindex="-1"
    aria-label={handle.label}
    class={[
      numberToTailwindBorder(classId),
      "absolute w-3 h-3 bg-white border-2 rounded-md",
      handle.cursor,
      handle.classes,
    ]}
    onmousedown={(e) => onHandleMouseDown(e, labelIndex, handle.position)}
  ></div>
{/each}
