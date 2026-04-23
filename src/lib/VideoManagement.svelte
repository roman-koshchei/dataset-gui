<script lang="ts">
  import type { VideoCollection, VideoEntry } from "./video-collection";
  import { extractYouTubeId, findLocalVideo, segmentsMatchFolders, isValidTimecode, segmentToFolderName, parseTimecode, formatTimecode } from "./video-collection";
  import { writeTextFile, readTextFile, exists } from "@tauri-apps/plugin-fs";
  import { openUrl, openPath } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import VideoPlayer from "./VideoPlayer.svelte";

  let {
    dataPath,
    videosDir = "",
    onBack,
    openDatasetInNewTab,
  }: {
    dataPath: string;
    videosDir?: string;
    onBack: () => void;
    openDatasetInNewTab?: (imagesDir: string, labelsDir: string, label: string) => void;
  } = $props();

  let collection = $state<VideoCollection | null>(null);
  let error = $state("");
  let hasUnsaved = $state(false);
  let selectedVideoIndex = $state(-1);

  let tagInput = $state("");

  let visibleVideos = $derived(
    collection
      ? collection.videos
          .map((v, i) => ({ video: v, index: i }))
          .filter(({ video }) => video.url || video.file_path || video.tags.length > 0 || (video.keep_segments && video.keep_segments.length > 0))
      : []
  );

  let localFiles = $state<string[]>([]);
  let segmentFolders = $state<Map<number, string[]>>(new Map());
  let playingSegment = $state<string | null>(null);
  let highlightedSegIndex = $state(-1);

  let resolvedVideosDir = $derived(
    videosDir || (dataPath.replace(/[^/\\]+$/, "") + "videos")
  );

  function segmentVideoPath(entry: VideoEntry, seg: string[]): string | null {
    const vid = getVideoId(entry);
    if (!vid) return null;
    return `${resolvedVideosDir}\\${vid}\\${segmentToFolderName(seg)}\\${segmentToFolderName(seg)}.mp4`;
  }

  function segmentFramePath(entry: VideoEntry, seg: string[]): string | null {
    const vid = getVideoId(entry);
    if (!vid) return null;
    return `${resolvedVideosDir}\\${vid}\\${segmentToFolderName(seg)}\\frames\\0001.png`;
  }

  function segmentFramesDir(entry: VideoEntry, seg: string[]): string | null {
    const vid = getVideoId(entry);
    if (!vid) return null;
    return `${resolvedVideosDir}\\${vid}\\${segmentToFolderName(seg)}\\frames`;
  }

  function segmentLabelsDir(entry: VideoEntry, seg: string[]): string | null {
    const vid = getVideoId(entry);
    if (!vid) return null;
    return `${resolvedVideosDir}\\${vid}\\${segmentToFolderName(seg)}\\labels`;
  }

  function getVideoId(entry: VideoEntry): string | null {
    if (entry.file_path) {
      const name = entry.file_path.replace(/[/\\]/g, "/").split("/").pop() ?? "";
      return name.replace(/\.[^.]+$/, "") || null;
    }
    return extractYouTubeId(entry.url);
  }

  async function loadLocalFiles() {
    if (!resolvedVideosDir) { localFiles = []; return; }
    try {
      localFiles = await invoke<string[]>("list_video_files", { dir: resolvedVideosDir });
    } catch {
      localFiles = [];
    }
  }

  async function loadSegmentFolders() {
    if (!collection) return;
    const map = new Map<number, string[]>();
    for (let i = 0; i < collection.videos.length; i++) {
      const entry = collection.videos[i];
      const vid = getVideoId(entry);
      if (!vid) continue;
      try {
        const dirs = await invoke<string[]>("list_subdirs", { dir: `${resolvedVideosDir}\\${vid}` });
        if (dirs.length > 0) map.set(i, dirs);
      } catch {
        // processing folder doesn't exist yet — expected for unprocessed videos
      }
    }
    segmentFolders = map;
  }

  function segmentStatus(entry: VideoEntry, index: number): "none" | "uptodate" | "stale" {
    const segs = entry.keep_segments;
    if (!segs || segs.length === 0) return "none";
    const folders = segmentFolders.get(index) ?? [];
    if (segmentsMatchFolders(segs, folders)) return "uptodate";
    return "stale";
  }

  let segmentHasDataset = $state<Map<string, boolean>>(new Map());

  $effect(() => {
    const vi = selectedVideoIndex;
    segmentHasDataset = new Map();
    if (!openDatasetInNewTab || vi < 0 || !collection) return;
    const video = collection.videos[vi];
    if (!video?.keep_segments) return;

    Promise.all(
      video.keep_segments.map(async (seg) => {
        const key = segmentToFolderName(seg);
        const fDir = segmentFramesDir(video, seg);
        const lDir = segmentLabelsDir(video, seg);
        if (!fDir || !lDir) return { key, available: false };
        try {
          const [fOk, lOk] = await Promise.all([exists(fDir), exists(lDir)]);
          return { key, available: fOk && lOk };
        } catch {
          return { key, available: false };
        }
      })
    ).then((results) => {
      const map = new Map<string, boolean>();
      for (const r of results) map.set(r.key, r.available);
      segmentHasDataset = map;
    });
  });

  function resolvedFilePath(entry: VideoEntry): string | undefined {
    if (entry.file_path) return entry.file_path;
    const ytId = extractYouTubeId(entry.url);
    const match = findLocalVideo(localFiles, ytId);
    return match ? `${resolvedVideosDir}\\${match}` : undefined;
  }

  async function loadCollection() {
    error = "";
    try {
      const text = await readTextFile(dataPath);
      try {
        collection = JSON.parse(text) as VideoCollection;
      } catch {
        error = `Failed to parse data.json: invalid JSON format`;
        return;
      }
      if (!collection.videos || !Array.isArray(collection.videos)) {
        error = "data.json is missing a valid 'videos' array";
        collection = null;
        return;
      }
      hasUnsaved = false;
      selectedVideoIndex = -1;
    } catch (err) {
      if (String(err).includes("not found") || String(err).includes("does not exist")) {
        error = `File not found: ${dataPath}`;
      } else {
        error = `Failed to read data.json: ${err instanceof Error ? err.message : String(err)}`;
      }
    }
  }

  async function saveCollection() {
    if (!collection) return;
    error = "";
    try {
      await writeTextFile(dataPath, JSON.stringify(collection, null, 2) + "\n");
      hasUnsaved = false;
    } catch (err) {
      error = `Failed to save: ${err instanceof Error ? err.message : String(err)}. Check file permissions.`;
    }
  }

  let addMode = $state<"none" | "url" | "local">("none");
  let addInput = $state("");

  function addVideoByUrl() {
    if (!collection || !addInput.trim()) return;
    collection.videos.push({ url: addInput.trim(), tags: [] });
    selectedVideoIndex = collection.videos.length - 1;
    hasUnsaved = true;
    addInput = "";
    addMode = "none";
  }

  function addVideoByFile() {
    if (!collection || !addInput.trim()) return;
    collection.videos.push({ url: "", file_path: addInput.trim(), tags: [] });
    selectedVideoIndex = collection.videos.length - 1;
    hasUnsaved = true;
    addInput = "";
    addMode = "none";
  }

  function removeVideo(index: number) {
    if (!collection) return;
    collection.videos.splice(index, 1);
    if (selectedVideoIndex === index) selectedVideoIndex = -1;
    else if (selectedVideoIndex > index) selectedVideoIndex--;
    hasUnsaved = true;
  }

  function removeSegment(videoIndex: number, segIndex: number) {
    if (!collection) return;
    collection.videos[videoIndex].keep_segments!.splice(segIndex, 1);
    hasUnsaved = true;
  }

  function addTagToSelected() {
    if (!collection || selectedVideoIndex < 0 || !tagInput.trim()) return;
    const video = collection.videos[selectedVideoIndex];
    const tag = tagInput.trim().toLowerCase();
    if (!video.tags.includes(tag)) {
      video.tags.push(tag);
      hasUnsaved = true;
    }
    tagInput = "";
  }

  function removeTag(videoIndex: number, tagIndex: number) {
    if (!collection) return;
    collection.videos[videoIndex].tags.splice(tagIndex, 1);
    hasUnsaved = true;
  }

  loadCollection().then(() => loadLocalFiles().then(() => loadSegmentFolders()));

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (!hasUnsaved || !collection) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => saveCollection(), 2000);
    return () => { if (saveTimer) clearTimeout(saveTimer); };
  });
</script>

<div class="h-full grid grid-rows-[auto_1fr]">
  <div class="border-b border-zinc-700 flex items-stretch text-sm">
    <button
      class="px-3 border-r border-zinc-700 py-1"
      onclick={onBack}
    >
      Back
    </button>
     <button
       class="px-3 border-r border-zinc-700 py-1 {hasUnsaved ? 'text-yellow-400' : 'text-zinc-500'}"
       onclick={saveCollection}
     >
       {hasUnsaved ? "Save *" : "Saved"}
     </button>
     <div class="relative">
       <button
         class="px-3 border-r border-zinc-700 py-1"
         onclick={() => addMode = addMode === "none" ? "url" : "none"}
       >
         + Video
       </button>
       {#if addMode !== "none"}
         <div class="absolute top-full left-0 z-10 bg-zinc-800 border border-zinc-600 shadow-lg">
           <div class="flex border-b border-zinc-700">
             <button
               class="px-3 py-1 text-sm {addMode === 'url' ? 'bg-zinc-700' : 'hover:bg-zinc-700'}"
               onclick={() => addMode = 'url'}
             >
               By URL
             </button>
             <button
               class="px-3 py-1 text-sm {addMode === 'local' ? 'bg-zinc-700' : 'hover:bg-zinc-700'}"
               onclick={() => addMode = 'local'}
             >
               Local file
             </button>
           </div>
           <div class="p-2">
             {#if addMode === "url"}
               <form class="flex gap-1" onsubmit={(e) => { e.preventDefault(); addVideoByUrl(); }}>
                 <input
                   type="text"
                   class="w-64 px-2 py-1 text-sm border border-zinc-700 bg-zinc-900"
                   placeholder="https://youtube.com/watch?v=..."
                   bind:value={addInput}
                 />
                 <button type="submit" class="px-3 py-1 text-sm bg-green-700 hover:bg-green-600">Add</button>
               </form>
             {:else}
               <form class="flex gap-1" onsubmit={(e) => { e.preventDefault(); addVideoByFile(); }}>
                 <input
                   type="text"
                   class="w-64 px-2 py-1 text-sm border border-zinc-700 bg-zinc-900"
                   placeholder="D:\Videos\video.mp4"
                   bind:value={addInput}
                 />
                 <button type="submit" class="px-3 py-1 text-sm bg-green-700 hover:bg-green-600">Add</button>
               </form>
             {/if}
           </div>
         </div>
       {/if}
     </div>
     <button
       class="px-3 border-r border-zinc-700 py-1 text-zinc-400 hover:text-zinc-200"
       onclick={() => loadSegmentFolders()}
     >
       Refresh
     </button>
     <span class="px-3 py-1 text-zinc-500">
       {dataPath}
     </span>
  </div>

  {#if error}
    <div class="p-3 bg-red-900/50 text-red-300 text-sm border-b border-red-800/50 flex items-start gap-2">
      <span class="flex-1">{error}</span>
      <button class="text-red-400 hover:text-red-300" onclick={() => error = ""}>x</button>
    </div>
  {/if}

  {#if collection && localFiles.length === 0 && resolvedVideosDir}
    <div class="p-2 bg-yellow-900/30 text-yellow-400 text-sm border-b border-yellow-800/50">
      No video files found in {resolvedVideosDir} — videos directory may not exist or is empty
    </div>
  {/if}

  {#if collection}
    <div class="overflow-hidden grid grid-cols-[360px_1fr] divide-x divide-zinc-700">
      <div class="overflow-y-auto">
        {#each visibleVideos as { video, index: i }}
          <button
            class="w-full text-left px-3 py-2 border-b border-zinc-800 {selectedVideoIndex === i ? 'bg-zinc-700' : 'hover:bg-zinc-800'}"
            onclick={() => selectedVideoIndex = i}
          >
            <div class="flex items-center gap-2">
              {#if video.url}
                <img
                  src="https://img.youtube.com/vi/{extractYouTubeId(video.url)}/default.jpg"
                  alt=""
                  class="w-16 h-10 object-cover flex-shrink-0"
                  loading="lazy"
                  onerror={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                />
              {:else}
                <div class="w-16 h-10 bg-zinc-800 flex-shrink-0 grid place-content-center text-zinc-600 text-xs">
                  N/A
                </div>
              {/if}
              <div class="min-w-0">
                <p class="truncate text-xs {resolvedFilePath(video) ? 'text-zinc-300' : 'text-zinc-400'}">
                  {video.url || video.file_path || "(no url)"}
                </p>
                <div class="flex flex-wrap gap-1 mt-0.5">
                  {#if segmentStatus(video, i) === "uptodate"}
                    <span class="text-[10px] bg-green-900 text-green-400 px-1">up to date</span>
                  {:else if segmentStatus(video, i) === "stale"}
                    <span class="text-[10px] bg-yellow-900 text-yellow-400 px-1">stale</span>
                  {/if}
                  {#if resolvedFilePath(video)}
                    <span class="text-[10px] bg-green-900 text-green-400 px-1">downloaded</span>
                  {/if}
                  {#each video.tags as tag}
                    <span class="text-[10px] bg-zinc-700 px-1">{tag}</span>
                  {/each}
                </div>
              </div>
            </div>
          </button>
        {/each}
      </div>

      <div class="overflow-y-auto p-4">
        {#if selectedVideoIndex >= 0 && selectedVideoIndex < collection.videos.length}
          {@const video = collection.videos[selectedVideoIndex]}
          <div class="space-y-6">
            <div class="flex items-start gap-4">
              <div class="flex-1 space-y-3">
                <label class="block space-y-1">
                  <span class="text-sm text-zinc-400">URL</span>
                  <div class="flex gap-1">
                    <input
                      type="text"
                      class="flex-1 px-3 py-2 border border-zinc-700 bg-zinc-800"
                      bind:value={video.url}
                      oninput={() => hasUnsaved = true}
                      placeholder="https://www.youtube.com/watch?v=..."
                    />
                    {#if video.url}
                      <button
                        class="px-3 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700"
                        onclick={async () => { try { await openUrl(video.url) } catch (e) { error = e instanceof Error ? e.message : String(e) } }}
                      >
                        Open
                      </button>
                    {/if}
                  </div>
                </label>

                <label class="block space-y-1">
                  <span class="text-sm text-zinc-400">Local video file</span>
                  <div class="flex gap-1">
                    <input
                      type="text"
                      class="flex-1 px-3 py-2 border border-zinc-700 bg-zinc-800"
                      value={video.file_path ?? ""}
                      oninput={(e) => { video.file_path = (e.target as HTMLInputElement).value || undefined; hasUnsaved = true; }}
                      placeholder={resolvedFilePath(video) ?? "D:\\Videos\\video.mp4"}
                    />
                    {#if resolvedFilePath(video)}
                      <button
                        class="px-3 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700"
                        onclick={async () => { try { await openPath(resolvedFilePath(video)!) } catch (e) { error = e instanceof Error ? e.message : String(e) } }}
                      >
                        Play
                      </button>
                    {/if}
                  </div>
                  {#if !video.file_path && resolvedFilePath(video)}
                    <span class="text-xs text-zinc-500">Auto-detected from videos directory</span>
                  {/if}
                </label>

                <div class="flex items-center gap-2 flex-wrap">
                  {#each video.tags as tag, ti}
                    <span class="bg-zinc-700 px-2 py-0.5 text-sm flex items-center gap-1">
                      {tag}
                      <button
                        class="text-zinc-400 hover:text-red-400"
                        onclick={() => removeTag(selectedVideoIndex, ti)}
                      >
                        x
                      </button>
                    </span>
                  {/each}
                  <form
                    class="inline-flex"
                    onsubmit={(e) => { e.preventDefault(); addTagToSelected(); }}
                  >
                    <input
                      type="text"
                      class="w-24 px-2 py-0.5 text-sm border border-zinc-700 bg-zinc-800"
                      bind:value={tagInput}
                      placeholder="+ tag"
                    />
                  </form>
                </div>
              </div>

              {#if extractYouTubeId(video.url)}
                <img
                  src="https://img.youtube.com/vi/{extractYouTubeId(video.url)}/hqdefault.jpg"
                  alt=""
                  class="w-48 h-auto border border-zinc-700"
                  loading="lazy"
                />
              {/if}
            </div>

            <div class="space-y-3">
               <VideoPlayer
                 filePath={resolvedFilePath(video) ?? ""}
                 segments={video.keep_segments ?? []}
                 onSegmentHover={(i) => highlightedSegIndex = i}
                  onSegmentsChange={(segs) => {
                   const seen = new Set<string>();
                   video.keep_segments = segs.filter(s => {
                     const key = `${s[0]}|${s[1]}`;
                     if (seen.has(key)) return false;
                     seen.add(key);
                     return true;
                   });
                   hasUnsaved = true;
                 }}
              />
            </div>

            <details class="text-sm">
              <summary class="text-zinc-500 cursor-pointer hover:text-zinc-400">Manual segment edit</summary>
              <div class="mt-2 space-y-1">
                {#if video.keep_segments && video.keep_segments.length > 0}
                  {#each video.keep_segments as seg, si}
                    <div class="flex items-center gap-2">
                      <input
                        type="text"
                        class="w-20 px-2 py-1 text-sm border border-zinc-700 bg-zinc-800 text-center"
                        bind:value={video.keep_segments![si][0]}
                        oninput={() => hasUnsaved = true}
                        placeholder="0:00"
                      />
                      <span class="text-zinc-500">to</span>
                      <input
                        type="text"
                        class="w-20 px-2 py-1 text-sm border border-zinc-700 bg-zinc-800 text-center"
                        bind:value={video.keep_segments![si][1]}
                        oninput={() => hasUnsaved = true}
                        placeholder="0:00"
                      />
                      <button
                        class="text-red-400 hover:text-red-300 text-sm px-1"
                        onclick={() => removeSegment(selectedVideoIndex, si)}
                      >
                        x
                      </button>
                    </div>
                  {/each}
                {:else}
                  <p class="text-zinc-600">No segments defined</p>
                {/if}
              </div>
            </details>

            {#if segmentStatus(video, selectedVideoIndex) === "uptodate"}
              <div class="text-sm text-green-400">Segments up to date</div>
            {:else if segmentStatus(video, selectedVideoIndex) === "stale"}
              <div class="text-sm text-yellow-400">Segments changed since last processing</div>
            {/if}

            {#if segmentStatus(video, selectedVideoIndex) === "uptodate" && video.keep_segments && video.keep_segments.length > 0}
              <div class="space-y-2">
                <div class="text-xs text-zinc-500">Segment previews ({video.keep_segments.length}) — click to play/pause</div>
                <div class="grid grid-cols-3 gap-2" id="segment-grid">
                  {#each video.keep_segments as seg, si}
                    {@const segKey = `${selectedVideoIndex}-${si}`}
                    {@const segPath = segmentVideoPath(video, seg)}
                    {@const thumbPath = segmentFramePath(video, seg)}
                    <button
                      class="relative bg-zinc-900 border overflow-hidden group {highlightedSegIndex === si ? 'border-green-500' : 'border-zinc-700 hover:border-zinc-500'}"
                      onmouseenter={() => highlightedSegIndex = si}
                      onmouseleave={() => highlightedSegIndex = -1}
                      onclick={(e) => {
                        const vid = (e.currentTarget.querySelector('video') as HTMLVideoElement | null);
                        if (!vid) return;
                        if (vid.paused) {
                          document.querySelectorAll<HTMLVideoElement>('#segment-grid video').forEach(v => { if (v !== vid && !v.paused) { v.pause(); v.currentTime = 0; } });
                          vid.play();
                          playingSegment = segKey;
                        } else {
                          vid.pause();
                          playingSegment = null;
                        }
                      }}
                    >
                      {#if segPath}
                        <!-- svelte-ignore a11y_media_has_caption -->
                        <video
                          src="{convertFileSrc(segPath)}#t=0.1"
                          preload="metadata"
                          poster={thumbPath ? convertFileSrc(thumbPath) : ''}
                          class="w-full aspect-video object-contain bg-black"
                          onplay={() => playingSegment = segKey}
                          onpause={() => { if (playingSegment === segKey) playingSegment = null; }}
                          onended={(e) => { playingSegment = null; e.currentTarget.currentTime = 0; }}
                        ></video>
                        {#if playingSegment !== segKey}
                          <div class="absolute inset-0 grid place-content-center pointer-events-none transition-opacity opacity-100 group-hover:opacity-0">
                            <span class="text-white/50 text-lg cursor-pointer">Play</span>
                          </div>
                        {/if}
                        <span
                          role="button"
                          tabindex={0}
                          class="absolute top-1 right-1 text-zinc-400 hover:text-zinc-200 bg-black/50 px-1 text-[10px] opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                          onclick={(e) => {
                            e.stopPropagation();
                            const dir = segPath!.replace(/[/\\][^/\\]+$/, '');
                            openPath(dir);
                          }}
                        >
                          dir
                        </span>
                        {#if segmentHasDataset.get(segmentToFolderName(seg)) && openDatasetInNewTab}
                          <span
                            role="button"
                            tabindex={0}
                            class="absolute top-1 left-1 text-blue-400 hover:text-blue-300 bg-black/50 px-1 text-[10px] opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                            onclick={(e) => {
                              e.stopPropagation();
                              const fDir = segmentFramesDir(video, seg)!;
                              const lDir = segmentLabelsDir(video, seg)!;
                              openDatasetInNewTab(fDir, lDir, `${getVideoId(video)}-${segmentToFolderName(seg)}`);
                            }}
                          >
                            dataset
                          </span>
                        {/if}
                      {:else}
                        <div class="w-full aspect-video bg-zinc-800 grid place-content-center text-zinc-600 text-xs">
                          N/A
                        </div>
                      {/if}
                      <div class="flex justify-between text-[10px] text-zinc-400 px-1 py-0.5 bg-zinc-800/80">
                        <span>{seg[0]}–{seg[1]}</span>
                        <span>{formatTimecode(parseTimecode(seg[1]) - parseTimecode(seg[0]))}</span>
                      </div>
                    </button>
                  {/each}
                </div>
              </div>
            {/if}

            <button
              class="bg-red-700 hover:bg-red-600 px-3 py-1.5 text-sm"
              onclick={() => { if (confirm("Delete this video?")) removeVideo(selectedVideoIndex); }}
            >
              Delete video
            </button>
          </div>
        {:else}
          <div class="h-full grid place-content-center text-zinc-600">
            Select a video from the list
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="h-full grid place-content-center text-zinc-600">
      Loading...
    </div>
  {/if}
</div>
