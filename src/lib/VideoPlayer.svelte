<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { buildYouTubeEmbedUrl, parseTimecode, formatTimecode, isValidTimecode } from "./video-collection";
  import type { VideoMask } from "./video-collection";
  import { numberToAccentPalette } from "./helpers";

  type ZoomRange = {
    start: number;
    end: number;
  };

  type EdgeDrag = { segIndex: number; edge: "start" | "end" };

  type ParsedSegment = {
    index: number;
    start: number;
    end: number;
  };

  type MaskDrag = {
    index: number;
    mode: "move" | "resize";
    handle: string;
    startX: number;
    startY: number;
    initial: VideoMask;
  };

  function clamp(value: number, min = 0, max = 1): number {
    return Math.max(min, Math.min(max, value));
  }

  function sameSegment(a?: string[], b?: string[]): boolean {
    return (a?.[0] ?? "") === (b?.[0] ?? "") && (a?.[1] ?? "") === (b?.[1] ?? "");
  }

  function normalizeMask(mask: VideoMask): VideoMask {
    const width = clamp(mask.width, 0.01, 1);
    const height = clamp(mask.height, 0.01, 1);
    return {
      ...mask,
      width,
      height,
      left: clamp(mask.left, 0, 1 - width),
      top: clamp(mask.top, 0, 1 - height),
    };
  }

  function clampRange(start: number, end: number): ZoomRange {
    return {
      start: Math.max(0, start),
      end: Math.min(1, end),
    };
  }

  function formatTickLabel(seconds: number): string {
    seconds = Math.max(0, seconds);
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    const frac = seconds - Math.floor(seconds);

    if (h > 0) {
      return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }

    if (frac > 0.001) {
      const ms = Math.round(frac * 1000);
      return `${m}:${s.toString().padStart(2, "0")}.${ms.toString().padStart(3, "0")}`;
    }

    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function timeToRatio(duration: number, time: number): number {
    return duration > 0 ? time / duration : 0;
  }

  function timeToZoomedRatio(
    duration: number,
    zoomStart: number,
    zoomEnd: number,
    time: number,
  ): number {
    if (duration <= 0) return 0;

    const zoomRange = zoomEnd - zoomStart;
    if (zoomRange <= 0) return 0;

    return (time / duration - zoomStart) / zoomRange;
  }

  function zoomedRatioToTime(
    duration: number,
    zoomStart: number,
    zoomEnd: number,
    ratio: number,
  ): number {
    return (zoomStart + ratio * (zoomEnd - zoomStart)) * duration;
  }

  function zoomInRange(zoomStart: number, zoomEnd: number): ZoomRange {
    const zoomRange = zoomEnd - zoomStart;
    const shrink = zoomRange * 0.25;
    const center = (zoomStart + zoomEnd) / 2;
    return clampRange(center - shrink, center + shrink);
  }

  function zoomOutRange(zoomStart: number, zoomEnd: number): ZoomRange {
    const zoomRange = zoomEnd - zoomStart;
    const expand = zoomRange * 0.5;
    return clampRange(zoomStart - expand, zoomEnd + expand);
  }

  function panZoomRange(
    panAnchor: number,
    mouseRatio: number,
    panZoomStart: number,
    panZoomEnd: number,
  ): ZoomRange {
    const zoomRange = panZoomEnd - panZoomStart;
    const delta = (mouseRatio - panAnchor) * zoomRange;

    let start = panZoomStart - delta;
    let end = panZoomEnd - delta;

    if (start < 0) {
      end -= start;
      start = 0;
    }
    if (end > 1) {
      start -= end - 1;
      end = 1;
    }

    return clampRange(start, end);
  }

  function zoomRangeFromWheel(
    zoomStart: number,
    zoomEnd: number,
    mouseRatio: number,
    deltaY: number,
  ): ZoomRange {
    const mouseNorm = zoomStart + mouseRatio * (zoomEnd - zoomStart);
    const factor = deltaY > 0 ? 1.15 : 1 / 1.15;
    const range = zoomEnd - zoomStart;
    const nextRange = Math.min(1, Math.max(0.001, range * factor));

    let start = mouseNorm - (mouseNorm - zoomStart) * (nextRange / range);
    let end = mouseNorm + (zoomEnd - mouseNorm) * (nextRange / range);

    if (start < 0) {
      end -= start;
      start = 0;
    }
    if (end > 1) {
      start -= end - 1;
      end = 1;
    }

    return clampRange(start, end);
  }

  function centerZoomRange(zoomStart: number, zoomEnd: number, center: number): ZoomRange {
    const range = zoomEnd - zoomStart;

    let start = center - range / 2;
    let end = center + range / 2;

    if (start < 0) {
      end -= start;
      start = 0;
    }
    if (end > 1) {
      start -= end - 1;
      end = 1;
    }

    return clampRange(start, end);
  }

  function moveZoomRange(zoomStart: number, zoomEnd: number, nextStart: number): ZoomRange {
    const range = zoomEnd - zoomStart;

    let start = nextStart;
    let end = nextStart + range;

    if (start < 0) {
      end -= start;
      start = 0;
    }
    if (end > 1) {
      start -= end - 1;
      end = 1;
    }

    return clampRange(start, end);
  }

  function followPlayheadRange(
    duration: number,
    zoomStart: number,
    zoomEnd: number,
    currentTime: number,
  ): ZoomRange {
    if (duration <= 0) {
      return { start: zoomStart, end: zoomEnd };
    }

    const range = zoomEnd - zoomStart;
    if (range <= 0 || range >= 0.999) {
      return { start: zoomStart, end: zoomEnd };
    }

    const normalizedTime = Math.max(0, Math.min(1, currentTime / duration));
    const leftPadding = range * 0.12;
    const rightPadding = range * 0.18;
    const minVisible = zoomStart + leftPadding;
    const maxVisible = zoomEnd - rightPadding;

    if (normalizedTime < minVisible) {
      return moveZoomRange(zoomStart, zoomEnd, normalizedTime - leftPadding);
    }

    if (normalizedTime > maxVisible) {
      return moveZoomRange(zoomStart, zoomEnd, normalizedTime + rightPadding - range);
    }

    return { start: zoomStart, end: zoomEnd };
  }

  function splitSegmentsAtTime(
    segments: string[][],
    parsedSegments: ParsedSegment[],
    currentTime: number,
  ): string[][] | null {
    for (const segment of parsedSegments) {
      if (currentTime > segment.start + 0.5 && currentTime < segment.end - 0.5) {
        const updated = [...segments];
        updated.splice(
          segment.index,
          1,
          [segments[segment.index][0], formatTimecode(currentTime)],
          [formatTimecode(currentTime), segments[segment.index][1]],
        );
        return updated;
      }
    }

    return null;
  }

  function addSegment(segments: string[][], start: number, end: number): string[][] {
    return [...segments, [formatTimecode(start), formatTimecode(end)]];
  }

  function removeSegmentAt(segments: string[][], index: number): string[][] {
    return segments.filter((_, segmentIndex) => segmentIndex !== index);
  }

  function updateSegmentEdge(
    segments: string[][],
    edgeDrag: EdgeDrag,
    time: number,
  ): string[][] {
    return segments.map((segment, index) => {
      if (index !== edgeDrag.segIndex) {
        return segment;
      }

      if (edgeDrag.edge === "start") {
        return [formatTimecode(time), segment[1]];
      }

      return [segment[0], formatTimecode(time)];
    });
  }

  let {
    filePath,
    youtubeUrl = "",
    segments = [],
    masks = [],
    onSegmentsChange,
    onMasksChange,
    onSegmentHover,
    highlightedSegmentIndex = -1,
  }: {
    filePath: string;
    youtubeUrl?: string;
    segments?: string[][];
    masks?: VideoMask[];
    onSegmentsChange: (segments: string[][]) => void;
    onMasksChange?: (masks: VideoMask[]) => void;
    onSegmentHover?: (index: number) => void;
    highlightedSegmentIndex?: number;
  } = $props();

  let videoEl: HTMLVideoElement | undefined = $state();
  let videoFrameEl: HTMLDivElement | undefined = $state();
  let currentTime = $state(0);
  let duration = $state(0);
  let playing = $state(false);
  let timelineEl: HTMLDivElement | undefined = $state();
  let videoError = $state("");
  let playbackRate = $state(1);
  let showMasks = $state(true);
  let selectedMaskIndex = $state(-1);
  let maskDrag: MaskDrag | null = $state(null);

  let markIn: number | null = $state(null);
  let dragging = $state(false);
  let dragStart = $state(0);
  let dragEnd = $state(0);
  let hoveredSegment = $state(-1);

  let zoomStart = $state(0);
  let zoomEnd = $state(1);
  let isPanning = $state(false);
  let panAnchor = $state(0);
  let panZoomStart = $state(0);
  let panZoomEnd = $state(0);

  let edgeDrag: EdgeDrag | null = $state(null);

  let overviewEl: HTMLDivElement | undefined = $state();
  let overviewDragging = $state(false);
  let playbackFrame: number | null = null;

  let timelineTicks = $derived.by(() => {
    if (duration <= 0) return [];
    const visibleDuration = (zoomEnd - zoomStart) * duration;
    if (visibleDuration <= 0) return [];

    const niceIntervals = [
      0.1, 0.2, 0.5,
      1, 2, 5, 10, 15, 30,
      60, 120, 300, 600, 900, 1800, 3600,
    ];
    const minPixelGap = 50;
    const approximatePixelWidth = timelineEl?.getBoundingClientRect().width ?? 600;
    const pixelsPerSecond = approximatePixelWidth / visibleDuration;
    let interval = niceIntervals[niceIntervals.length - 1];
    for (const ni of niceIntervals) {
      if (ni * pixelsPerSecond >= minPixelGap) {
        interval = ni;
        break;
      }
    }

    const startTime = zoomStart * duration;
    const endTime = zoomEnd * duration;
    const firstTick = Math.ceil(startTime / interval) * interval;
    const ticks: { time: number; label: string; major: boolean }[] = [];
    for (let t = firstTick; t <= endTime; t += interval) {
      const rounded = Math.round(t * 1000) / 1000;
      const major = interval >= 1 ? Math.abs(rounded % (interval * (interval >= 60 ? 1 : 5 < interval ? 2 : 5))) < 0.001 : false;
      ticks.push({ time: rounded, label: formatTickLabel(rounded), major });
    }
    return ticks;
  });

  let parsedSegments = $derived(
    segments
      .flatMap(([s, e], index) => {
        if (!isValidTimecode(s) || !isValidTimecode(e)) {
          return [];
        }

        return [{ index, start: parseTimecode(s), end: parseTimecode(e) }];
      })
  );

  let invalidSegmentCount = $derived(
    segments.filter(([s, e]) => !isValidTimecode(s) || !isValidTimecode(e)).length
  );

  let activeSegment = $derived(
    parsedSegments.find(seg => currentTime >= seg.start && currentTime <= seg.end)?.index ?? -1
  );

  let visibleMasks = $derived(
    masks
      .map((mask, index) => ({ mask: normalizeMask(mask), index }))
      .filter(({ mask }) => !mask.segment || parsedSegments.some(seg => seg.index === activeSegment && sameSegment(mask.segment, segments[seg.index])))
  );

  let isTimelineInteracting = $derived(
    dragging || isPanning || edgeDrag !== null || overviewDragging
  );

  function isSegmentHighlighted(i: number): boolean {
    return hoveredSegment === i || activeSegment === i || highlightedSegmentIndex === i;
  }

  $effect(() => {
    onSegmentHover?.(hoveredSegment >= 0 ? hoveredSegment : activeSegment);
  });

  let videoSrc = $derived(filePath ? convertFileSrc(filePath) : "");
  let youtubeEmbedSrc = $derived(youtubeUrl ? buildYouTubeEmbedUrl(youtubeUrl) : "");

  function zoomToFull() {
    zoomStart = 0;
    zoomEnd = 1;
  }

  function zoomIn() {
    ({ start: zoomStart, end: zoomEnd } = zoomInRange(zoomStart, zoomEnd));
  }

  function zoomOut() {
    ({ start: zoomStart, end: zoomEnd } = zoomOutRange(zoomStart, zoomEnd));
  }

  function tick() {
    if (videoEl) currentTime = videoEl.currentTime;
  }

  function cancelPlaybackLoop() {
    if (playbackFrame !== null) {
      cancelAnimationFrame(playbackFrame);
      playbackFrame = null;
    }
  }

  function startPlaybackLoop() {
    cancelPlaybackLoop();

    const updatePlaybackTime = () => {
      const el = videoEl;
      if (!el || el.paused || el.ended) {
        playbackFrame = null;
        return;
      }

      currentTime = el.currentTime;
      playbackFrame = requestAnimationFrame(updatePlaybackTime);
    };

    currentTime = videoEl?.currentTime ?? currentTime;
    playbackFrame = requestAnimationFrame(updatePlaybackTime);
  }

  function togglePlay() {
    if (!videoEl) return;
    if (videoEl.paused) videoEl.play();
    else videoEl.pause();
  }

  function seekTo(t: number) {
    if (!videoEl) return;
    videoEl.currentTime = t;
    currentTime = t;
  }

  function splitAtPlayhead() {
    if (!videoEl) return;
    const updated = splitSegmentsAtTime(segments, parsedSegments, videoEl.currentTime);
    if (updated) {
      onSegmentsChange(updated);
    }
  }

  let canSplit = $derived(() => {
    if (!videoEl) return false;
    const t = currentTime;
    return parsedSegments.some(seg => t > seg.start + 0.5 && t < seg.end - 0.5);
  });

  function handleMarkIn() {
    if (!videoEl) return;
    markIn = videoEl.currentTime;
  }

  function handleMarkOut() {
    if (!videoEl || markIn === null) return;
    const out = videoEl.currentTime;
    if (out <= markIn) return;
    onSegmentsChange(addSegment(segments, markIn, out));
    markIn = null;
  }

  function removeSegment(index: number) {
    onSegmentsChange(removeSegmentAt(segments, index));
  }

  function updateMasks(nextMasks: VideoMask[]) {
    onMasksChange?.(nextMasks.map(normalizeMask));
  }

  function addMask(segmentScoped: boolean) {
    const active = activeSegment >= 0 ? segments[activeSegment] : undefined;
    const next: VideoMask = {
      left: 0.35,
      top: 0.35,
      width: 0.3,
      height: 0.18,
      ...(segmentScoped && active ? { segment: [...active] } : {}),
    };
    updateMasks([...masks, next]);
    selectedMaskIndex = masks.length;
    showMasks = true;
  }

  function removeMask(index: number) {
    updateMasks(masks.filter((_, i) => i !== index));
    selectedMaskIndex = -1;
  }

  function videoFramePoint(e: MouseEvent): { x: number; y: number } {
    if (!videoFrameEl) return { x: 0, y: 0 };
    const rect = videoFrameEl.getBoundingClientRect();
    return {
      x: clamp((e.clientX - rect.left) / rect.width),
      y: clamp((e.clientY - rect.top) / rect.height),
    };
  }

  function startMaskDrag(e: MouseEvent, index: number, mode: "move" | "resize", handle = "") {
    e.stopPropagation();
    e.preventDefault();
    const point = videoFramePoint(e);
    selectedMaskIndex = index;
    maskDrag = { index, mode, handle, startX: point.x, startY: point.y, initial: normalizeMask(masks[index]) };
  }

  function handleMaskMove(e: MouseEvent) {
    if (!maskDrag) return;
    const point = videoFramePoint(e);
    const dx = point.x - maskDrag.startX;
    const dy = point.y - maskDrag.startY;
    const initial = maskDrag.initial;
    let left = initial.left;
    let top = initial.top;
    let width = initial.width;
    let height = initial.height;

    if (maskDrag.mode === "move") {
      left = initial.left + dx;
      top = initial.top + dy;
    } else {
      if (maskDrag.handle.includes("l")) {
        left = initial.left + dx;
        width = initial.width - dx;
      }
      if (maskDrag.handle.includes("r")) width = initial.width + dx;
      if (maskDrag.handle.includes("t")) {
        top = initial.top + dy;
        height = initial.height - dy;
      }
      if (maskDrag.handle.includes("b")) height = initial.height + dy;
    }

    const next = [...masks];
    next[maskDrag.index] = normalizeMask({ ...initial, left, top, width, height });
    updateMasks(next);
  }

  function stopMaskDrag() {
    maskDrag = null;
  }

  function timelineMousePos(e: MouseEvent): number {
    if (!timelineEl) return 0;
    const rect = timelineEl.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  }

  function handleTimelineDown(e: MouseEvent) {
    if (!duration) return;
    if (e.button === 1) {
      e.preventDefault();
      isPanning = true;
      panAnchor = timelineMousePos(e);
      panZoomStart = zoomStart;
      panZoomEnd = zoomEnd;
      return;
    }
    if (edgeDrag) {
      dragging = false;
      return;
    }
    dragging = true;
    const ratio = timelineMousePos(e);
    dragStart = zoomedRatioToTime(duration, zoomStart, zoomEnd, ratio);
    dragEnd = zoomedRatioToTime(duration, zoomStart, zoomEnd, ratio);
  }

  function handleTimelineMove(e: MouseEvent) {
    if (isPanning) {
      ({ start: zoomStart, end: zoomEnd } = panZoomRange(
        panAnchor,
        timelineMousePos(e),
        panZoomStart,
        panZoomEnd,
      ));
      return;
    }
    if (edgeDrag) {
      const time = zoomedRatioToTime(duration, zoomStart, zoomEnd, timelineMousePos(e));
      onSegmentsChange(updateSegmentEdge(segments, edgeDrag, time));
      return;
    }
    if (!dragging) return;
    const ratio = timelineMousePos(e);
    dragEnd = zoomedRatioToTime(duration, zoomStart, zoomEnd, ratio);
  }

  function handleTimelineUp() {
    isPanning = false;
    if (edgeDrag) {
      edgeDrag = null;
      return;
    }
    if (!dragging) return;
    dragging = false;
    const s = Math.min(dragStart, dragEnd);
    const e = Math.max(dragStart, dragEnd);
    if (e - s < 0.5) {
      seekTo(s);
      return;
    }
    onSegmentsChange(addSegment(segments, s, e));
  }

  function handleTimelineWheel(e: WheelEvent) {
    if (!duration) return;
    e.preventDefault();
    ({ start: zoomStart, end: zoomEnd } = zoomRangeFromWheel(
      zoomStart,
      zoomEnd,
      timelineMousePos(e),
      e.deltaY,
    ));
  }

  function overviewMousePos(e: MouseEvent): number {
    if (!overviewEl) return 0;
    const rect = overviewEl.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  }

  function handleOverviewDown(e: MouseEvent) {
    if (!duration) return;
    e.preventDefault();
    ({ start: zoomStart, end: zoomEnd } = centerZoomRange(zoomStart, zoomEnd, overviewMousePos(e)));
    overviewDragging = true;
  }

  function handleOverviewMove(e: MouseEvent) {
    if (!overviewDragging) return;
    ({ start: zoomStart, end: zoomEnd } = centerZoomRange(zoomStart, zoomEnd, overviewMousePos(e)));
  }

  function handleOverviewUp() {
    overviewDragging = false;
  }

  $effect(() => {
    if (!playing || duration <= 0 || isTimelineInteracting) return;

    const nextRange = followPlayheadRange(duration, zoomStart, zoomEnd, currentTime);
    if (Math.abs(nextRange.start - zoomStart) < 0.0001 && Math.abs(nextRange.end - zoomEnd) < 0.0001) {
      return;
    }

    zoomStart = nextRange.start;
    zoomEnd = nextRange.end;
  });

  $effect(() => {
    const el = videoEl;
    if (!el) return;
    el.playbackRate = playbackRate;
  });

  $effect(() => {
    const el = videoEl;
    if (!el) return;
    const onPlay = () => {
      playing = true;
      startPlaybackLoop();
    };
    const onPause = () => {
      playing = false;
      currentTime = el.currentTime;
      cancelPlaybackLoop();
    };
    const onLoaded = () => {
      duration = el.duration;
      currentTime = el.currentTime;
      videoError = "";
    };
    const onError = () => {
      videoError = "Failed to load video. The file may be missing, unsupported, or the path may be incorrect.";
      playing = false;
      cancelPlaybackLoop();
    };
    const onEnded = () => {
      playing = false;
      currentTime = el.currentTime;
      cancelPlaybackLoop();
    };
    el.addEventListener("play", onPlay);
    el.addEventListener("pause", onPause);
    el.addEventListener("loadedmetadata", onLoaded);
    el.addEventListener("error", onError);
    el.addEventListener("ended", onEnded);
    return () => {
      cancelPlaybackLoop();
      el.removeEventListener("play", onPlay);
      el.removeEventListener("pause", onPause);
      el.removeEventListener("loadedmetadata", onLoaded);
      el.removeEventListener("error", onError);
      el.removeEventListener("ended", onEnded);
    };
  });

  function handleKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      seekTo(Math.max(0, currentTime - 1));
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      seekTo(Math.min(duration, currentTime + 1));
    } else if (e.key === " ") {
      e.preventDefault();
      togglePlay();
    }
  }

  $effect(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  $effect(() => {
    window.addEventListener("mousemove", handleMaskMove);
    window.addEventListener("mouseup", stopMaskDrag);
    return () => {
      window.removeEventListener("mousemove", handleMaskMove);
      window.removeEventListener("mouseup", stopMaskDrag);
    };
  });
</script>

{#if videoSrc}
  <div class="space-y-3">
    <div bind:this={videoFrameEl} class="relative bg-black aspect-video max-w-2xl mx-auto overflow-hidden">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={videoEl}
        src={videoSrc}
        class="w-full h-full"
        onclick={togglePlay}
        ontimeupdate={tick}
      ></video>
      {#if showMasks}
        {#each visibleMasks as { mask, index }}
          <div
            class="absolute bg-black cursor-move {selectedMaskIndex === index ? 'ring-2 ring-white' : 'ring-1 ring-white/40'}"
            style="left: {mask.left * 100}%; top: {mask.top * 100}%; width: {mask.width * 100}%; height: {mask.height * 100}%;"
            role="button"
            tabindex="-1"
            aria-label={`Mask rectangle ${index + 1}`}
            onmousedown={(e) => startMaskDrag(e, index, "move")}
          >
            {#if selectedMaskIndex === index}
              {#each ["tl", "tr", "bl", "br"] as handle}
                <div
                  class="absolute h-3 w-3 rounded-sm border border-white bg-zinc-900 {handle === 'tl' ? '-left-1.5 -top-1.5 cursor-nw-resize' : handle === 'tr' ? '-right-1.5 -top-1.5 cursor-ne-resize' : handle === 'bl' ? '-left-1.5 -bottom-1.5 cursor-sw-resize' : '-right-1.5 -bottom-1.5 cursor-se-resize'}"
                  role="button"
                  tabindex="-1"
                  aria-label={`Resize mask ${handle}`}
                  onmousedown={(e) => startMaskDrag(e, index, "resize", handle)}
                ></div>
              {/each}
            {/if}
          </div>
        {/each}
      {/if}
      {#if !playing && currentTime === 0}
        <button
          class="absolute inset-0 grid place-content-center text-white/50 text-lg cursor-pointer {showMasks && visibleMasks.length > 0 ? 'pointer-events-none' : ''}"
          onclick={togglePlay}
        >
          Play
        </button>
      {/if}
    </div>

    {#if videoError}
      <div class="text-sm text-red-400 bg-red-900/30 px-3 py-2 border border-red-800/50">
        {videoError}
      </div>
    {/if}

    <div class="flex items-center gap-3 text-sm">
      <button
        class="px-2 py-1 bg-zinc-700 hover:bg-zinc-600"
        onclick={() => seekTo(Math.max(0, currentTime - 1))}
        disabled={!videoEl}
      >
        -1s
      </button>
      <button
        class="px-3 py-1 bg-zinc-700 hover:bg-zinc-600 min-w-[60px]"
        onclick={togglePlay}
      >
        {playing ? "Pause" : "Play"}
      </button>
      <button
        class="px-2 py-1 bg-zinc-700 hover:bg-zinc-600"
        onclick={() => seekTo(Math.min(duration, currentTime + 1))}
        disabled={!videoEl}
      >
        +1s
      </button>
      <span class="text-zinc-400 tabular-nums">
        {formatTimecode(currentTime)} / {formatTimecode(duration)}
      </span>
      <div class="flex items-center gap-0.5 ml-auto">
        {#each [1, 1.25, 1.5, 2] as rate}
          <button
            class="px-1.5 py-1 text-xs {playbackRate === rate ? 'bg-zinc-500 text-white' : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600'}"
            onclick={() => playbackRate = rate}
          >
            {rate}x
          </button>
        {/each}
      </div>
    </div>

    <div class="flex items-center gap-2 text-sm flex-wrap">
      <button
        class="px-3 py-1 {showMasks ? 'bg-zinc-500 text-white' : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600'}"
        onclick={() => showMasks = !showMasks}
      >
        {showMasks ? "Hide Masks" : "Show Masks"}
      </button>
      <button
        class="px-3 py-1 bg-zinc-700 hover:bg-zinc-600 disabled:text-zinc-500"
        onclick={() => addMask(false)}
        disabled={!onMasksChange}
      >
        + Whole Video Mask
      </button>
      <button
        class="px-3 py-1 bg-zinc-700 hover:bg-zinc-600 disabled:text-zinc-500"
        onclick={() => addMask(true)}
        disabled={!onMasksChange || activeSegment < 0}
        title={activeSegment < 0 ? "Move playhead inside a segment first" : "Add mask only for the active segment"}
      >
        + Segment Mask
      </button>
      {#if selectedMaskIndex >= 0 && masks[selectedMaskIndex]}
        {@const selectedMask = masks[selectedMaskIndex]}
        <span class="text-xs text-zinc-500">
          Selected: {selectedMask.segment ? `${selectedMask.segment[0]}-${selectedMask.segment[1]}` : "whole video"}
        </span>
        <button
          class="px-2 py-1 text-xs bg-red-900/70 text-red-300 hover:bg-red-900"
          onclick={() => removeMask(selectedMaskIndex)}
        >
          Delete Mask
        </button>
      {/if}
      {#if masks.length > 0}
        <span class="text-xs text-zinc-500">{visibleMasks.length}/{masks.length} mask{masks.length !== 1 ? 's' : ''} visible</span>
      {/if}
    </div>

    <div
      class="space-y-1"
      role="group"
    >
      <div class="flex items-center gap-2 text-sm">
        <button
          class="px-3 py-1 bg-blue-700 hover:bg-blue-600 disabled:bg-zinc-700 disabled:text-zinc-500"
          onclick={handleMarkIn}
          disabled={!videoEl}
        >
          Mark In{markIn !== null ? ` (${formatTimecode(markIn)})` : ""}
        </button>
        <button
          class="px-3 py-1 bg-green-700 hover:bg-green-600 disabled:bg-zinc-700 disabled:text-zinc-500"
          onclick={handleMarkOut}
          disabled={markIn === null}
        >
          Mark Out
        </button>
        <button
          class="px-3 py-1 bg-amber-700 hover:bg-amber-600 disabled:bg-zinc-700 disabled:text-zinc-500"
          onclick={splitAtPlayhead}
          disabled={!canSplit()}
        >
          Split
        </button>
      </div>
    </div>

    <div class="space-y-1">
      <div class="flex items-center justify-between gap-2 text-xs">
        <span class="text-zinc-500">Timeline — click segment to seek, drag empty area to create, scroll to zoom</span>
        <div class="flex items-center gap-1">
          <button class="px-1.5 py-0.5 bg-zinc-700 hover:bg-zinc-600" onclick={zoomOut}>-</button>
          <button class="px-1.5 py-0.5 bg-zinc-700 hover:bg-zinc-600" onclick={zoomToFull}>Fit</button>
          <button class="px-1.5 py-0.5 bg-zinc-700 hover:bg-zinc-600" onclick={zoomIn}>+</button>
          <span class="text-zinc-600">{Math.round((zoomEnd - zoomStart) * 100)}%</span>
        </div>
      </div>

      <div
        id="video-timeline"
        bind:this={timelineEl}
        class="relative h-12 bg-zinc-800 border border-zinc-700 select-none cursor-crosshair overflow-hidden"
        role="slider"
        aria-label="Video timeline"
        aria-valuenow={Math.round(currentTime)}
        aria-valuemin={0}
        aria-valuemax={Math.round(duration)}
        tabindex="0"
        onmousedown={handleTimelineDown}
        onmousemove={handleTimelineMove}
        onmouseup={handleTimelineUp}
        onmouseleave={() => { if (dragging) handleTimelineUp(); if (isPanning) isPanning = false; if (edgeDrag) edgeDrag = null; }}
        onwheel={handleTimelineWheel}
      >
        {#each timelineTicks as tick}
          <div
            class="absolute bottom-0 w-px bg-zinc-600 pointer-events-none"
            style="left: {timeToZoomedRatio(duration, zoomStart, zoomEnd, tick.time) * 100}%; height: {tick.major ? '40%' : '20%'};"
          ></div>
          <span
            class="absolute bottom-0 text-[9px] text-zinc-500 pointer-events-none tabular-nums"
            style="left: {timeToZoomedRatio(duration, zoomStart, zoomEnd, tick.time) * 100}%; transform: translateX(-50%); padding-bottom: 2px;"
          >
            {tick.label}
          </span>
        {/each}

        {#each parsedSegments as seg}
          {@const palette = numberToAccentPalette(seg.index)}
          <div
            class="absolute top-0 h-[60%] transition-colors"
            style="left: {timeToZoomedRatio(duration, zoomStart, zoomEnd, seg.start) * 100}%; width: {(timeToZoomedRatio(duration, zoomStart, zoomEnd, seg.end) - timeToZoomedRatio(duration, zoomStart, zoomEnd, seg.start)) * 100}%; background-color: {isSegmentHighlighted(seg.index) ? palette.fillStrong : palette.fill};"
            role="button"
            tabindex="-1"
            aria-label={`Segment ${seg.index + 1}: ${segments[seg.index][0]} to ${segments[seg.index][1]}`}
            onmouseenter={() => hoveredSegment = seg.index}
            onmouseleave={() => hoveredSegment = -1}
            onclick={(e) => { e.stopPropagation(); seekTo(zoomedRatioToTime(duration, zoomStart, zoomEnd, timelineMousePos(e as unknown as MouseEvent))); }}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); seekTo((seg.start + seg.end) / 2); } }}
          >
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="absolute left-0 top-0 w-1.5 h-full cursor-ew-resize {hoveredSegment === seg.index ? 'hover:bg-white/30' : ''}"
              onmousedown={(e) => { e.stopPropagation(); edgeDrag = { segIndex: seg.index, edge: "start" }; }}
            ></div>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="absolute right-0 top-0 w-1.5 h-full cursor-ew-resize {hoveredSegment === seg.index ? 'hover:bg-white/30' : ''}"
              onmousedown={(e) => { e.stopPropagation(); edgeDrag = { segIndex: seg.index, edge: "end" }; }}
            ></div>
          </div>
        {/each}

        {#if dragging && duration > 0}
          {@const ds = Math.min(dragStart, dragEnd)}
          {@const de = Math.max(dragStart, dragEnd)}
          <div
            class="absolute top-0 h-[60%] bg-yellow-500/40 border border-yellow-400/60"
            style="left: {timeToZoomedRatio(duration, zoomStart, zoomEnd, ds) * 100}%; width: {(timeToZoomedRatio(duration, zoomStart, zoomEnd, de) - timeToZoomedRatio(duration, zoomStart, zoomEnd, ds)) * 100}%;"
          ></div>
        {/if}

        {#if duration > 0}
          <div
            class="absolute top-0 h-[60%] w-0.5 bg-white pointer-events-none"
            style="left: {timeToZoomedRatio(duration, zoomStart, zoomEnd, currentTime) * 100}%;"
          ></div>
        {/if}
      </div>

      {#if zoomEnd - zoomStart < 0.99}
        <!-- svelte-ignore a11y_interactive_supports_focus -->
        <div
          bind:this={overviewEl}
          class="relative h-3 bg-zinc-900 border border-zinc-700 select-none cursor-pointer"
          role="scrollbar"
          aria-controls="video-timeline"
          aria-valuenow={Math.round(zoomStart * 100)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Timeline scroll overview"
          onmousedown={handleOverviewDown}
          onmousemove={handleOverviewMove}
          onmouseup={handleOverviewUp}
          onmouseleave={handleOverviewUp}
        >
          {#each parsedSegments as seg}
            {@const palette = numberToAccentPalette(seg.index)}
            <div
              class="absolute top-0 h-full pointer-events-none transition-colors"
              style="left: {timeToRatio(duration, seg.start) * 100}%; width: {(timeToRatio(duration, seg.end) - timeToRatio(duration, seg.start)) * 100}%; background-color: {isSegmentHighlighted(seg.index) ? palette.fillStrong : palette.fillMuted};"
            ></div>
          {/each}
          <div
            class="absolute top-0 h-full bg-zinc-600/60 border-x border-zinc-500"
            style="left: {zoomStart * 100}%; width: {(zoomEnd - zoomStart) * 100}%;"
          ></div>
          {#if duration > 0}
            <div
              class="absolute top-0 h-full w-px bg-white/70 pointer-events-none"
              style="left: {timeToRatio(duration, currentTime) * 100}%;"
            ></div>
          {/if}
        </div>
      {/if}
    </div>

    {#if invalidSegmentCount > 0}
      <div class="text-sm text-yellow-400 bg-yellow-900/30 px-3 py-2 border border-yellow-800/50">
        {invalidSegmentCount} segment{invalidSegmentCount > 1 ? 's have' : ' has'} invalid timecode format
      </div>
    {/if}

    {#if segments.length > 0}
      <div class="space-y-1">
        <div class="text-xs text-zinc-500">Segments ({segments.length})</div>
        <div class="flex flex-wrap gap-1">
          {#each segments as seg, i}
            {@const palette = numberToAccentPalette(i)}
            <div
              class="text-xs px-2 py-0.5 inline-flex items-center gap-1 transition-colors"
              role="group"
              aria-label={`Segment ${i + 1}`}
              onmouseenter={() => hoveredSegment = i}
              onmouseleave={() => hoveredSegment = -1}
              style="background-color: {isSegmentHighlighted(i) ? palette.fillStrong : palette.fillMuted}; color: {isSegmentHighlighted(i) ? 'white' : palette.text};"
            >
              <button
                class="hover:underline"
                onclick={() => seekTo(parseTimecode(seg[0]))}
              >
                {seg[0]}–{seg[1]}
              </button>
              <button
                class="text-red-400 hover:text-red-300 text-[10px]"
                tabindex={-1}
                onclick={(e: Event) => { e.stopPropagation(); removeSegment(i); }}
              >x</button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{:else if youtubeEmbedSrc}
  <div class="space-y-3">
    <div class="relative bg-black aspect-video max-w-2xl mx-auto border border-zinc-700">
      <iframe
        src={youtubeEmbedSrc}
        title="YouTube video player"
        class="w-full h-full"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
        referrerpolicy="strict-origin-when-cross-origin"
        allowfullscreen
      ></iframe>
    </div>

    <div class="text-sm text-zinc-400 bg-zinc-900/50 px-3 py-2 border border-zinc-800">
      YouTube preview is view-only. Timeline editing still requires a local video file.
    </div>

    {#if invalidSegmentCount > 0}
      <div class="text-sm text-yellow-400 bg-yellow-900/30 px-3 py-2 border border-yellow-800/50">
        {invalidSegmentCount} segment{invalidSegmentCount > 1 ? 's have' : ' has'} invalid timecode format
      </div>
    {/if}

    {#if segments.length > 0}
      <div class="space-y-1">
        <div class="text-xs text-zinc-500">Segments ({segments.length})</div>
        <div class="flex flex-wrap gap-1">
          {#each segments as seg, i}
            {@const palette = numberToAccentPalette(i)}
            <div
              class="text-xs px-2 py-0.5 inline-flex items-center gap-1 transition-colors"
              role="group"
              aria-label={`Segment ${i + 1}`}
              onmouseenter={() => hoveredSegment = i}
              onmouseleave={() => hoveredSegment = -1}
              style="background-color: {isSegmentHighlighted(i) ? palette.fillStrong : palette.fillMuted}; color: {isSegmentHighlighted(i) ? 'white' : palette.text};"
            >
              <span>{seg[0]}–{seg[1]}</span>
              <button
                class="text-red-400 hover:text-red-300 text-[10px]"
                tabindex={-1}
                onclick={(e: Event) => { e.stopPropagation(); removeSegment(i); }}
              >x</button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{:else}
  <div class="text-zinc-600 text-sm">No local video file or YouTube URL found</div>
{/if}
