<script lang="ts">
  let {
    imageSrc,
    onConfirm,
    onCancel,
  }: {
    imageSrc: string;
    onConfirm: (boxes: number[][]) => void;
    onCancel: () => void;
  } = $props();

  type Rect = { x: number; y: number; w: number; h: number };

  let boxes = $state<Rect[]>([]);
  let drawing = $state<Rect | null>(null);
  let startPoint = $state<{ x: number; y: number } | null>(null);
  let container = $state<HTMLDivElement>();

  function normalize(event: PointerEvent) {
    const rect = container!.getBoundingClientRect();
    return {
      x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)),
      y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)),
    };
  }

  function handlePointerDown(event: PointerEvent) {
    if (event.button !== 0 || !container) return;
    container.setPointerCapture?.(event.pointerId);
    const point = normalize(event);
    startPoint = point;
    drawing = { x: point.x, y: point.y, w: 0, h: 0 };
  }

  function handlePointerMove(event: PointerEvent) {
    if (!startPoint || !drawing) return;
    const point = normalize(event);
    drawing = {
      x: Math.min(startPoint.x, point.x),
      y: Math.min(startPoint.y, point.y),
      w: Math.abs(point.x - startPoint.x),
      h: Math.abs(point.y - startPoint.y),
    };
  }

  function handlePointerUp() {
    if (drawing && drawing.w > 0.004 && drawing.h > 0.004) {
      boxes = [...boxes, drawing];
    }
    drawing = null;
    startPoint = null;
  }

  function removeBox(index: number) {
    boxes = boxes.filter((_, i) => i !== index);
  }

  function confirm() {
    onConfirm(
      boxes.map((box) => [
        box.x + box.w / 2,
        box.y + box.h / 2,
        box.w,
        box.h,
      ]),
    );
  }
</script>

<div class="fixed inset-0 z-50 bg-black/70 grid place-items-center p-4">
  <div
    class="bg-zinc-900 border border-zinc-700 p-3 space-y-3 max-w-[92vw] max-h-[92vh] flex flex-col"
  >
    <div class="flex items-center justify-between gap-6">
      <span class="text-sm text-zinc-200">
        Drag to mark the object(s) on the last frame
      </span>
      <button
        type="button"
        class="text-zinc-400 hover:text-zinc-200"
        aria-label="Close"
        onclick={onCancel}
      >
        x
      </button>
    </div>

    <div
      bind:this={container}
      role="application"
      aria-label="Bounding box marking area"
      class="relative inline-block select-none cursor-crosshair touch-none overflow-hidden bg-black"
      onpointerdown={handlePointerDown}
      onpointermove={handlePointerMove}
      onpointerup={handlePointerUp}
      onpointercancel={handlePointerUp}
    >
      <img
        src={imageSrc}
        alt="last frame"
        class="block max-w-[80vw] max-h-[68vh]"
        draggable="false"
      />
      {#each boxes as box, index (index)}
        <div
          class="absolute border-2 border-purple-400 bg-purple-400/15"
          style="left: {box.x * 100}%; top: {box.y * 100}%; width: {box.w * 100}%; height: {box.h * 100}%"
        >
          <button
            type="button"
            class="absolute -top-3 -right-3 w-5 h-5 grid place-content-center bg-red-600 text-white text-xs leading-none"
            aria-label="Remove box"
            onclick={(event) => {
              event.stopPropagation();
              removeBox(index);
            }}
          >
            x
          </button>
        </div>
      {/each}
      {#if drawing}
        <div
          class="absolute border-2 border-dashed border-purple-300"
          style="left: {drawing.x * 100}%; top: {drawing.y * 100}%; width: {drawing.w * 100}%; height: {drawing.h * 100}%"
        ></div>
      {/if}
    </div>

    <div class="flex items-center gap-2 text-xs">
      <span class="text-zinc-400">{boxes.length} box(es)</span>
      <button
        type="button"
        class="px-3 py-1 bg-zinc-700 hover:bg-zinc-600 disabled:opacity-40"
        disabled={boxes.length === 0}
        onclick={() => (boxes = [])}
      >
        Clear
      </button>
      <span class="flex-1"></span>
      <button
        type="button"
        class="px-3 py-1 bg-purple-600 hover:bg-purple-700 disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={boxes.length === 0}
        onclick={confirm}
      >
        Track
      </button>
      <button
        type="button"
        class="px-3 py-1 bg-zinc-700 hover:bg-zinc-600"
        onclick={onCancel}
      >
        Cancel
      </button>
    </div>
  </div>
</div>
