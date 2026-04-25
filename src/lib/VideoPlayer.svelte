<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { buildYouTubeEmbedUrl, parseTimecode, formatTimecode, isValidTimecode } from "./video-collection";

  let {
    filePath,
    youtubeUrl = "",
    segments = [],
    onSegmentsChange,
    onSegmentHover,
  }: {
    filePath: string;
    youtubeUrl?: string;
    segments?: string[][];
    onSegmentsChange: (segments: string[][]) => void;
    onSegmentHover?: (index: number) => void;
  } = $props();

  let videoEl: HTMLVideoElement | undefined = $state();
  let currentTime = $state(0);
  let duration = $state(0);
  let playing = $state(false);
  let timelineEl: HTMLDivElement | undefined = $state();
  let videoError = $state("");

  let markIn: number | null = $state(null);
  let dragging = $state(false);
  let dragStart = $state(0);
  let dragEnd = $state(0);
  let hoveredSegment = $state(-1);

  let parsedSegments = $derived(
    segments
      .filter(([s, e]) => isValidTimecode(s) && isValidTimecode(e))
      .map(([s, e]) => ({ start: parseTimecode(s), end: parseTimecode(e) }))
  );

  let invalidSegmentCount = $derived(
    segments.filter(([s, e]) => !isValidTimecode(s) || !isValidTimecode(e)).length
  );

  let activeSegment = $derived(
    parsedSegments.findIndex(seg => currentTime >= seg.start && currentTime <= seg.end)
  );

  function isSegmentHighlighted(i: number): boolean {
    return hoveredSegment === i || activeSegment === i;
  }

  $effect(() => {
    onSegmentHover?.(hoveredSegment >= 0 ? hoveredSegment : activeSegment);
  });

  let videoSrc = $derived(filePath ? convertFileSrc(filePath) : "");
  let youtubeEmbedSrc = $derived(youtubeUrl ? buildYouTubeEmbedUrl(youtubeUrl) : "");

  function timeToRatio(t: number): number {
    return duration > 0 ? t / duration : 0;
  }

  function ratioToTime(r: number): number {
    return r * duration;
  }

  function tick() {
    if (videoEl) currentTime = videoEl.currentTime;
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
    const t = videoEl.currentTime;
    for (let i = 0; i < parsedSegments.length; i++) {
      const seg = parsedSegments[i];
      if (t > seg.start + 0.5 && t < seg.end - 0.5) {
        const updated = [...segments];
        updated.splice(i, 1, [segments[i][0], formatTimecode(t)], [formatTimecode(t), segments[i][1]]);
        onSegmentsChange(updated);
        return;
      }
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
    const newSeg: string[] = [formatTimecode(markIn), formatTimecode(out)];
    const updated = [...segments, newSeg];
    onSegmentsChange(updated);
    markIn = null;
  }

  function removeSegment(index: number) {
    const updated = segments.filter((_, i) => i !== index);
    onSegmentsChange(updated);
  }

  function timelineMousePos(e: MouseEvent): number {
    if (!timelineEl) return 0;
    const rect = timelineEl.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  }

  function handleTimelineDown(e: MouseEvent) {
    if (!duration) return;
    dragging = true;
    const ratio = timelineMousePos(e);
    dragStart = ratioToTime(ratio);
    dragEnd = ratioToTime(ratio);
  }

  function handleTimelineMove(e: MouseEvent) {
    if (!dragging) return;
    const ratio = timelineMousePos(e);
    dragEnd = ratioToTime(ratio);
  }

  function handleTimelineUp() {
    if (!dragging) return;
    dragging = false;
    const s = Math.min(dragStart, dragEnd);
    const e = Math.max(dragStart, dragEnd);
    if (e - s < 0.5) {
      seekTo(s);
      return;
    }
    const newSeg: string[] = [formatTimecode(s), formatTimecode(e)];
    const updated = [...segments, newSeg];
    onSegmentsChange(updated);
  }

  $effect(() => {
    const el = videoEl;
    if (!el) return;
    const onPlay = () => (playing = true);
    const onPause = () => (playing = false);
    const onLoaded = () => { duration = el.duration; videoError = ""; };
    const onError = () => { videoError = "Failed to load video. The file may be missing, unsupported, or the path may be incorrect."; playing = false; };
    el.addEventListener("play", onPlay);
    el.addEventListener("pause", onPause);
    el.addEventListener("loadedmetadata", onLoaded);
    el.addEventListener("error", onError);
    return () => {
      el.removeEventListener("play", onPlay);
      el.removeEventListener("pause", onPause);
      el.removeEventListener("loadedmetadata", onLoaded);
      el.removeEventListener("error", onError);
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
</script>

{#if videoSrc}
  <div class="space-y-3">
    <div class="relative bg-black aspect-video max-w-2xl mx-auto">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={videoEl}
        src={videoSrc}
        class="w-full h-full"
        onclick={togglePlay}
        ontimeupdate={tick}
      ></video>
      {#if !playing && currentTime === 0}
        <button
          class="absolute inset-0 grid place-content-center text-white/50 text-lg cursor-pointer"
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
      <div class="text-xs text-zinc-500">Timeline — click segment to seek, drag empty area to create</div>

      <div
        bind:this={timelineEl}
        class="relative h-8 bg-zinc-800 border border-zinc-700 select-none cursor-crosshair"
        role="slider"
        aria-label="Video timeline"
        aria-valuenow={Math.round(currentTime)}
        aria-valuemin={0}
        aria-valuemax={Math.round(duration)}
        tabindex="0"
        onmousedown={handleTimelineDown}
        onmousemove={handleTimelineMove}
        onmouseup={handleTimelineUp}
        onmouseleave={() => { if (dragging) handleTimelineUp(); }}
      >
        {#each parsedSegments as seg, i}
          <button
            type="button"
            class="absolute top-0 h-full bg-green-700/60 {isSegmentHighlighted(i) ? '!bg-green-500/80' : 'hover:bg-green-600/80'}"
            style="left: {timeToRatio(seg.start) * 100}%; width: {(timeToRatio(seg.end) - timeToRatio(seg.start)) * 100}%;"
            aria-label={`Seek to segment ${i + 1}: ${segments[i][0]} to ${segments[i][1]}`}
            onmouseenter={() => hoveredSegment = i}
            onmouseleave={() => hoveredSegment = -1}
            onclick={(e) => { e.stopPropagation(); seekTo(ratioToTime(timelineMousePos(e as unknown as MouseEvent))); }}
          ></button>
        {/each}

        {#if dragging && duration > 0}
          {@const ds = Math.min(dragStart, dragEnd)}
          {@const de = Math.max(dragStart, dragEnd)}
          <div
            class="absolute top-0 h-full bg-yellow-500/40 border border-yellow-400/60"
            style="left: {timeToRatio(ds) * 100}%; width: {(timeToRatio(de) - timeToRatio(ds)) * 100}%;"
          ></div>
        {/if}

        {#if duration > 0}
          <div
            class="absolute top-0 h-full w-0.5 bg-white pointer-events-none"
            style="left: {timeToRatio(currentTime) * 100}%;"
          ></div>
        {/if}
      </div>
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
            <div
              class="text-xs px-2 py-0.5 inline-flex items-center gap-1 {isSegmentHighlighted(i) ? 'bg-green-600 text-white' : 'bg-green-900 text-green-300 hover:bg-green-800'}"
              role="group"
              aria-label={`Segment ${i + 1}`}
              onmouseenter={() => hoveredSegment = i}
              onmouseleave={() => hoveredSegment = -1}
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
            <div
              class="text-xs px-2 py-0.5 inline-flex items-center gap-1 {isSegmentHighlighted(i) ? 'bg-green-600 text-white' : 'bg-green-900 text-green-300'}"
              role="group"
              aria-label={`Segment ${i + 1}`}
              onmouseenter={() => hoveredSegment = i}
              onmouseleave={() => hoveredSegment = -1}
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
