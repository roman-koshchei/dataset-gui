<script lang="ts">
  import type { VideoCollection, VideoEntry, VideoMask } from "./video-collection";
  import { extractYouTubeId, extractXId, extractTelegramId, extractVideoId, extractFileStem, findLocalVideo, segmentsMatchFolders, segmentToFolderName, parseTimecode, formatTimecode, sortSegments } from "./video-collection";
  import { writeTextFile, readTextFile, exists, mkdir } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import VideoPlayer from "./VideoPlayer.svelte";
  import BboxMarkDialog from "./BboxMarkDialog.svelte";
  import type { Dataset } from "./dataset";
  import { numberToAccentPalette } from "./helpers";

  let {
    dataPath,
    videosDir = "",
    datasetKitDir = "",
    onBack,
    openDatasetInNewTab,
  }: {
    dataPath: string;
    videosDir?: string;
    datasetKitDir?: string;
    onBack: () => void;
    openDatasetInNewTab?: (dataset: Dataset, label: string) => void;
  } = $props();

  let collection = $state<VideoCollection | null>(null);
  let error = $state("");
  let hasUnsaved = $state(false);
  let selectedVideoIndex = $state(-1);
  let detailsPane = $state<HTMLDivElement>();
  let selectedVideoIndices = $state<number[]>([]);
  let lastSelectionAnchorVisible = $state<number | null>(null);

  let tagInput = $state("");
  let selectedTag = $state("");
  let tagFilterOpen = $state(false);
  let availableTags = $state<string[]>([]);
  let visibleVideos = $state<{ video: VideoEntry; index: number }[]>([]);
  let hideIrrelevant = $state(true);
  let soundMuted = $state(true);

  function refreshVideoList() {
    if (!collection) {
      availableTags = [];
      visibleVideos = [];
      return;
    }

    availableTags = [...new Set(collection.videos.flatMap((video) => video.tags ?? []))]
      .sort((a, b) => a.localeCompare(b));
    visibleVideos = collection.videos
      .map((video, index) => ({ video, index }))
      .filter(({ video }) =>
        Boolean(
          video.url ||
          video.file ||
          video.file_path ||
          video.tags?.length ||
          video.keep_segments?.length
        ) &&
        (!hideIrrelevant || video.irrelevant !== true) &&
        (!selectedTag || video.tags?.includes(selectedTag))
      );
  }

  function setTagFilter(tag: string) {
    selectedTag = tag;
    tagFilterOpen = false;
    refreshVideoList();
  }

  let localFiles = $state<string[]>([]);
  let segmentFolders = $state<Map<number, string[]>>(new Map());
  let playingSegment = $state<string | null>(null);
  let highlightedSegIndex = $state(-1);

  type SplitJobState = { status: "running" | "done" | "error"; log: string[] };
  let splitJobs = $state<Map<string, SplitJobState>>(new Map());
  let splitError = $state("");
  let resolvedKitDir = $state("");
  let datasetRefresh = $state(0);

  type AnnotateJobState = { status: "running" | "done" | "error" | "cancelled"; log: string[] };
  type AnnotateDiskStatus = { status: string; totalFrames: number; processedFrames: number };
  let annotateJobs = $state<Map<string, AnnotateJobState>>(new Map());
  let annotateStatuses = $state<Map<string, AnnotateDiskStatus>>(new Map());
  let annotateError = $state("");
  let annotateClassId = $state(0);
  let annotateEveryN = $state(30);
  let annotateWriteAll = $state(true);
  let annotateScoreThreshold = $state(0);
  let annotateTrackDevice = $state("");
  let annotateSettingsOpen = $state(false);
  const PREVIEW_SUBDIR = ".preview_labels";
  let segmentPreviewReady = $state<Map<string, boolean>>(new Map());
  let segmentPreviewSaved = $state<Set<string>>(new Set());
  let previewTarget = $state<{
    videoId: string;
    segmentFolder: string;
    framesDir: string;
    previewDir: string;
  } | null>(null);
  let trackDialog = $state<{
    video: VideoEntry;
    seg: string[];
    imageSrc: string;
    framesDir: string;
    previewDir: string;
  } | null>(null);
  let anyAnnotateRunning = $derived(
    [...annotateJobs.values()].some((job) => job.status === "running")
  );

  let resolvedVideosDir = $derived(
    videosDir || (dataPath.replace(/[^/\\]+$/, "") + "videos")
  );

  function getSplitJob(videoId: string | null): SplitJobState | undefined {
    return videoId ? splitJobs.get(videoId) : undefined;
  }

  function setSplitJob(videoId: string, status: SplitJobState["status"], line?: string) {
    const next = new Map(splitJobs);
    const existing = next.get(videoId);
    const log = line !== undefined ? [...(existing?.log ?? []), line].slice(-200) : existing?.log ?? [];
    next.set(videoId, { status, log });
    splitJobs = next;
  }

  async function resolveKitDir() {
    try {
      resolvedKitDir = await invoke<string>("resolve_dataset_kit", {
        datasetKitDir: datasetKitDir || null,
      });
      splitError = "";
    } catch (err) {
      resolvedKitDir = "";
      splitError = String(err);
    }
  }

  async function startSplit(video: VideoEntry) {
    const videoId = getVideoId(video);
    if (!videoId) {
      splitError = "Cannot determine a video id for this entry";
      return;
    }
    if (splitJobs.get(videoId)?.status === "running") return;

    const dataDir = dataPath.replace(/[\\/][^\\/]+$/, "");
    splitError = "";
    if (hasUnsaved) {
      await saveCollection();
      if (hasUnsaved) return;
    }
    setSplitJob(videoId, "running", `Starting split for ${videoId}...`);
    try {
      await invoke("start_video_split", {
        dataJson: dataPath,
        projectDir: dataDir || dataPath,
        videosDir: resolvedVideosDir || null,
        videoId,
        datasetKitDir: datasetKitDir || null,
      });
      if (!resolvedKitDir) void resolveKitDir();
    } catch (err) {
      setSplitJob(videoId, "error", `Error: ${String(err)}`);
    }
  }

  async function cancelSplit(video: VideoEntry) {
    const videoId = getVideoId(video);
    if (!videoId) return;
    try {
      await invoke("cancel_video_split", { videoId });
    } catch (err) {
      splitError = String(err);
    }
  }

  function getAnnotateJob(videoId: string | null): AnnotateJobState | undefined {
    return videoId ? annotateJobs.get(videoId) : undefined;
  }

  function setAnnotateJob(videoId: string, status: AnnotateJobState["status"], line?: string) {
    const next = new Map(annotateJobs);
    const existing = next.get(videoId);
    const log = line !== undefined ? [...(existing?.log ?? []), line].slice(-200) : existing?.log ?? [];
    next.set(videoId, { status, log });
    annotateJobs = next;
  }

  async function refreshAnnotateStatuses() {
    if (!collection || !resolvedVideosDir) {
      annotateStatuses = new Map();
      return;
    }
    try {
      const result = await invoke<Record<string, AnnotateDiskStatus>>(
        "get_auto_annotate_statuses",
        { videosDir: resolvedVideosDir, everyN: annotateEveryN },
      );
      annotateStatuses = new Map(Object.entries(result));
    } catch {
      annotateStatuses = new Map();
    }
  }

  type StatusVariant = "ok" | "local" | "warn" | "error" | "neutral";
  const statusPillClass: Record<StatusVariant, string> = {
    ok: "bg-[#18382a] text-[#80d9a6]",
    local: "bg-[#1b3346] text-[#86caff]",
    warn: "bg-[#3b3020] text-[#efc477]",
    error: "bg-[#44272a] text-[#ffa0a0]",
    neutral: "bg-[#202226] text-[#a8adb8]",
  };
  const statusPillBase =
    "inline-flex items-center gap-1 text-[11px] leading-4 px-1.5 py-0.5 whitespace-nowrap";
  const statusIcon: Record<StatusVariant, string> = {
    ok: "✓",
    local: "↓",
    warn: "↻",
    error: "!",
    neutral: "○",
  };

  function annotateIndicator(videoId: string | null): { label: string; variant: StatusVariant } | null {
    if (!videoId) return null;
    const job = annotateJobs.get(videoId);
    if (job?.status === "running") {
      return { label: "tracking…", variant: "warn" };
    }
    if (job?.status === "cancelled") {
      return { label: "cancelled", variant: "neutral" };
    }
    if (job?.status === "error") {
      return { label: "track error", variant: "error" };
    }
    const disk = annotateStatuses.get(videoId);
    if (!disk) return null;
    if (disk.status === "completed") {
      return { label: "tracked", variant: "ok" };
    }
    if (disk.status === "partial") {
      return {
        label: `tracked ${disk.processedFrames}/${disk.totalFrames}`,
        variant: "warn",
      };
    }
    return { label: "not tracked", variant: "neutral" };
  }

  async function startManualTrack(video: VideoEntry, seg: string[], boxes: number[][]) {
    const videoId = getVideoId(video);
    if (!videoId) {
      annotateError = "Cannot determine a video id for this entry";
      return;
    }
    if (annotateJobs.get(videoId)?.status === "running") return;
    if (boxes.length === 0) {
      annotateError = "Mark at least one box on the last frame";
      return;
    }

    const framesDir = segmentFramesDir(video, seg);
    const previewDir = segmentPreviewDir(video, seg);
    if (!framesDir || !previewDir) {
      annotateError = "Cannot resolve segment directories";
      return;
    }

    const dataDir = dataPath.replace(/[\\/][^\\/]+$/, "");
    annotateError = "";
    if (hasUnsaved) {
      await saveCollection();
      if (hasUnsaved) return;
    }

    previewTarget = {
      videoId,
      segmentFolder: segmentToFolderName(seg),
      framesDir,
      previewDir,
    };

    setAnnotateJob(
      videoId,
      "running",
      `Tracking ${boxes.length} box(es) in ${segmentToFolderName(seg)}...`,
    );
    try {
      await invoke("start_video_annotate", {
        dataJson: dataPath,
        projectDir: dataDir || dataPath,
        videosDir: resolvedVideosDir || null,
        videoId,
        classes: [],
        classId: annotateClassId,
        everyN: annotateEveryN,
        writeAll: annotateWriteAll,
        datasetKitDir: datasetKitDir || null,
        scoreThreshold: annotateScoreThreshold,
        trackDevice: annotateTrackDevice,
        preview: true,
        segment: segmentToFolderName(seg),
        labelsSubdir: PREVIEW_SUBDIR,
        initBboxes: boxes,
      });
      if (!resolvedKitDir) void resolveKitDir();
    } catch (err) {
      previewTarget = null;
      setAnnotateJob(videoId, "error", `Error: ${String(err)}`);
    }
  }

  async function openTrackDialog(video: VideoEntry, seg: string[]) {
    const framesDir = segmentFramesDir(video, seg);
    const previewDir = segmentPreviewDir(video, seg);
    if (!framesDir || !previewDir) {
      annotateError = "Cannot resolve segment directories";
      return;
    }
    annotateError = "";
    try {
      const lastFrame = await invoke<string | null>("get_last_frame_path", {
        imagesDir: framesDir,
      });
      if (!lastFrame) {
        annotateError = "No frames found for this segment";
        return;
      }
      trackDialog = {
        video,
        seg,
        imageSrc: convertFileSrc(lastFrame),
        framesDir,
        previewDir,
      };
    } catch (err) {
      annotateError = String(err);
    }
  }

  async function confirmTrack(boxes: number[][]) {
    const dialog = trackDialog;
    trackDialog = null;
    if (!dialog || boxes.length === 0) return;
    await startManualTrack(dialog.video, dialog.seg, boxes);
  }

  function openPreviewTab(target: {
    videoId: string;
    segmentFolder: string;
    framesDir: string;
    previewDir: string;
  }) {
    if (!openDatasetInNewTab) return;
    openDatasetInNewTab(
      { dirs: [{ imagesDir: target.framesDir, labelsDir: target.previewDir }] },
      `${target.videoId}-${target.segmentFolder} preview`,
    );
  }

  async function refreshSegmentPreviews() {
    if (!collection || selectedVideoIndex < 0) {
      segmentPreviewReady = new Map();
      return;
    }
    const video = collection.videos[selectedVideoIndex];
    if (!video?.keep_segments || video.keep_segments.length === 0) {
      segmentPreviewReady = new Map();
      return;
    }
    const vid = getVideoId(video) ?? "";
    const map = new Map<string, boolean>();
    await Promise.all(
      video.keep_segments.map(async (seg) => {
        const key = segmentToFolderName(seg);
        const dir = segmentPreviewDir(video, seg);
        if (!dir) {
          map.set(key, false);
          return;
        }
        try {
          const available = await exists(dir);
          map.set(key, available && !segmentPreviewSaved.has(`${vid}::${key}`));
        } catch {
          map.set(key, false);
        }
      }),
    );
    segmentPreviewReady = map;
  }

  async function saveSegmentPreview(video: VideoEntry, seg: string[]) {
    const segDir = segmentDir(video, seg);
    const vid = getVideoId(video);
    if (!segDir || !vid) return;
    try {
      await invoke("commit_segment_preview", {
        segmentDir: segDir,
        previewSubdir: PREVIEW_SUBDIR,
        labelsSubdir: "labels",
      });
      segmentPreviewSaved = new Set([
        ...segmentPreviewSaved,
        `${vid}::${segmentToFolderName(seg)}`,
      ]);
      const next = new Map(segmentPreviewReady);
      next.set(segmentToFolderName(seg), false);
      segmentPreviewReady = next;
      void refreshAnnotateStatuses();
      datasetRefresh += 1;
    } catch (err) {
      annotateError = String(err);
    }
  }

  async function discardSegmentPreview(video: VideoEntry, seg: string[]) {
    const segDir = segmentDir(video, seg);
    const vid = getVideoId(video);
    if (!segDir || !vid) return;
    try {
      await invoke("discard_segment_preview", {
        segmentDir: segDir,
        previewSubdir: PREVIEW_SUBDIR,
      });
      segmentPreviewSaved = new Set(
        [...segmentPreviewSaved].filter(
          (key) => key !== `${vid}::${segmentToFolderName(seg)}`,
        ),
      );
      const next = new Map(segmentPreviewReady);
      next.set(segmentToFolderName(seg), false);
      segmentPreviewReady = next;
    } catch (err) {
      annotateError = String(err);
    }
  }

  async function cancelAutoAnnotate(video: VideoEntry) {
    const videoId = getVideoId(video);
    if (!videoId) return;
    try {
      await invoke("cancel_video_annotate", { videoId });
    } catch (err) {
      annotateError = String(err);
    }
  }

  $effect(() => {
    const unlistenProgress = listen<{ videoId: string; stream: string; line: string }>(
      "video-split-progress",
      (event) => {
        const { videoId, stream, line } = event.payload;
        setSplitJob(videoId, "running", `[${stream}] ${line}`);
      },
    );
    const unlistenDone = listen<{ videoId: string; success: boolean; code: number | null }>(
      "video-split-done",
      (event) => {
        const { videoId, success, code } = event.payload;
        if (success) {
          setSplitJob(videoId, "done");
          void loadSegmentFolders();
          void refreshAnnotateStatuses();
          datasetRefresh += 1;
        } else {
          setSplitJob(videoId, "error", `Exited with code ${code ?? "unknown"}`);
        }
      },
    );
    const unlistenError = listen<{ videoId: string; error: string }>(
      "video-split-error",
      (event) => {
        setSplitJob(event.payload.videoId, "error", `Error: ${event.payload.error}`);
      },
    );
    return () => {
      void unlistenProgress.then((unlisten) => unlisten());
      void unlistenDone.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  });

  $effect(() => {
    const unlistenProgress = listen<{ videoId: string; stream: string; line: string }>(
      "video-annotate-progress",
      (event) => {
        const { videoId, stream, line } = event.payload;
        setAnnotateJob(videoId, "running", `[${stream}] ${line}`);
      },
    );
    const unlistenDone = listen<{ videoId: string; success: boolean; code: number | null }>(
      "video-annotate-done",
      (event) => {
        const { videoId, success, code } = event.payload;
        const target = previewTarget;
        previewTarget = null;
        if (success) {
          setAnnotateJob(videoId, "done");
          void refreshAnnotateStatuses();
          if (target && target.videoId === videoId) {
            openPreviewTab(target);
            void refreshSegmentPreviews();
          }
        } else {
          setAnnotateJob(videoId, "error", `Exited with code ${code ?? "unknown"}`);
        }
      },
    );
    const unlistenError = listen<{ videoId: string; error: string }>(
      "video-annotate-error",
      (event) => {
        previewTarget = null;
        const error = event.payload.error;
        if (error === "cancelled") {
          setAnnotateJob(event.payload.videoId, "cancelled", "Cancelled");
        } else {
          setAnnotateJob(event.payload.videoId, "error", `Error: ${error}`);
        }
      },
    );
    return () => {
      void unlistenProgress.then((unlisten) => unlisten());
      void unlistenDone.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  });

  async function openVideoUrl(url: string) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function openFilePathInViewer(path: string) {
    try {
      await invoke("reveal_in_file_manager", { path });
    } catch {
      try {
        const { openPath } = await import("@tauri-apps/plugin-opener");
        await openPath(path);
      } catch (e) {
        error = e instanceof Error ? e.message : String(e);
      }
    }
  }

  async function openFilePath(path: string) {
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(path);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function isVideoSelected(index: number): boolean {
    return selectedVideoIndices.includes(index);
  }

  function openVideoDetails(index: number): void {
    selectedVideoIndex = index;
    detailsPane?.scrollTo({ top: 0 });
  }

  function toggleVideoSelection(index: number, visibleIndex: number, event: MouseEvent) {
    if (event.shiftKey && lastSelectionAnchorVisible !== null) {
      const start = Math.min(lastSelectionAnchorVisible, visibleIndex);
      const end = Math.max(lastSelectionAnchorVisible, visibleIndex);
      const rangeSelection = visibleVideos.slice(start, end + 1).map(({ index }) => index);
      selectedVideoIndices = [...new Set([...selectedVideoIndices, ...rangeSelection])];
    } else {
      if (isVideoSelected(index)) {
        selectedVideoIndices = selectedVideoIndices.filter((value) => value !== index);
      } else {
        selectedVideoIndices = [...selectedVideoIndices, index];
      }
    }

    lastSelectionAnchorVisible = visibleIndex;
  }

  function toggleSegmentPreview(container: HTMLElement, segKey: string) {
    const vid = container.querySelector("video") as HTMLVideoElement | null;
    if (!vid) return;
    if (vid.paused) {
      document.querySelectorAll<HTMLVideoElement>("#segment-grid video").forEach(v => {
        if (v !== vid && !v.paused) {
          v.pause();
          v.currentTime = 0;
        }
      });
      void vid.play();
      playingSegment = segKey;
    } else {
      vid.pause();
      playingSegment = null;
    }
  }

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

  function segmentDir(entry: VideoEntry, seg: string[]): string | null {
    const vid = getVideoId(entry);
    if (!vid) return null;
    return `${resolvedVideosDir}\\${vid}\\${segmentToFolderName(seg)}`;
  }

  function segmentPreviewDir(entry: VideoEntry, seg: string[]): string | null {
    const dir = segmentDir(entry, seg);
    return dir ? `${dir}\\${PREVIEW_SUBDIR}` : null;
  }

  function getVideoId(entry: VideoEntry): string | null {
    if (entry.file_path) {
      return extractFileStem(entry.file_path);
    }
    if (entry.file) {
      return extractFileStem(entry.file);
    }
    if (entry.url) {
      return extractVideoId(entry.url);
    }
    return null;
  }

  function lazyThumb(node: HTMLVideoElement, src: string) {
    let currentSrc = src;
    let loaded = false;
    const ensure = () => {
      if (loaded) return;
      loaded = true;
      node.src = currentSrc;
    };
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) ensure();
      },
      { rootMargin: "200px" },
    );
    observer.observe(node);
    const onEnter = () => ensure();
    node.addEventListener("mouseenter", onEnter);
    return {
      update(nextSrc: string) {
        if (nextSrc === currentSrc) return;
        currentSrc = nextSrc;
        loaded = false;
        node.removeAttribute("src");
        const rect = node.getBoundingClientRect();
        if (rect.bottom > -200 && rect.top < window.innerHeight + 200) ensure();
      },
      destroy() {
        observer.disconnect();
        node.removeEventListener("mouseenter", onEnter);
      },
    };
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
    if (!collection || !resolvedVideosDir) return;
    let foldersByVideo: Record<string, string[]> = {};
    try {
      foldersByVideo = await invoke<Record<string, string[]>>("get_segment_folders", {
        videosDir: resolvedVideosDir,
      });
    } catch {
      segmentFolders = new Map();
      return;
    }
    const map = new Map<number, string[]>();
    collection.videos.forEach((entry, i) => {
      const vid = getVideoId(entry);
      if (!vid) return;
      const dirs = foldersByVideo[vid];
      if (dirs && dirs.length > 0) map.set(i, dirs);
    });
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

  async function collectSegmentDatasetDirs(videos: VideoEntry[]) {
    const dirs: { imagesDir: string; labelsDir: string }[] = [];
    for (const video of videos) {
      if (!video.keep_segments) continue;
      for (const seg of video.keep_segments) {
        const fDir = segmentFramesDir(video, seg);
        const lDir = segmentLabelsDir(video, seg);
        if (!fDir || !lDir) continue;
        try {
          const fOk = await exists(fDir);
          if (!fOk) continue;
          if (!(await exists(lDir))) {
            await mkdir(lDir, { recursive: true });
          }
          dirs.push({ imagesDir: fDir, labelsDir: lDir });
        } catch { /* skip */ }
      }
    }
    return dirs;
  }

  async function openAllSegmentsAsDataset() {
    if (!collection || !openDatasetInNewTab) return;
    const dirs = await collectSegmentDatasetDirs(collection.videos);
    if (dirs.length === 0) return;
    const label = collection.collection || dataPath.split(/[/\\]/).slice(-2, -1)[0] || "Dataset";
    openDatasetInNewTab({ dirs }, label);
  }

  async function openSelectedVideoAsDataset() {
    if (!collection || !openDatasetInNewTab || selectedVideoIndex < 0) return;
    const video = collection.videos[selectedVideoIndex];
    if (!video) return;
    const dirs = await collectSegmentDatasetDirs([video]);
    if (dirs.length === 0) return;
    openDatasetInNewTab({ dirs }, getVideoId(video) ?? `Video ${selectedVideoIndex + 1}`);
  }

  $effect(() => {
    const vi = selectedVideoIndex;
    void datasetRefresh;
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
    const videoId = getVideoId(entry);
    if (!videoId) return undefined;
    const match = findLocalVideo(localFiles, videoId);
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
      for (const video of collection.videos) {
        if (video.keep_segments && video.keep_segments.length > 0) {
          video.keep_segments = sortSegments(video.keep_segments);
        }
      }
      hasUnsaved = false;
      selectedVideoIndex = -1;
      selectedVideoIndices = [];
      lastSelectionAnchorVisible = null;
      refreshVideoList();
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
    selectedVideoIndices = [selectedVideoIndex];
    lastSelectionAnchorVisible = null;
    hasUnsaved = true;
    addInput = "";
    addMode = "none";
    refreshVideoList();
  }

  function addVideoByFile() {
    if (!collection || !addInput.trim()) return;
    collection.videos.push({ url: "", file_path: addInput.trim(), tags: [] });
    selectedVideoIndex = collection.videos.length - 1;
    selectedVideoIndices = [selectedVideoIndex];
    lastSelectionAnchorVisible = null;
    hasUnsaved = true;
    addInput = "";
    addMode = "none";
    refreshVideoList();
  }

  function markSelectedIrrelevant() {
    if (!collection || selectedVideoIndices.length === 0) return;
    for (const index of selectedVideoIndices) {
      const video = collection.videos[index];
      if (video) video.irrelevant = true;
    }
    selectedVideoIndices = [];
    lastSelectionAnchorVisible = null;
    hasUnsaved = true;
    refreshVideoList();
  }

  function setVideoIrrelevant(video: VideoEntry, irrelevant: boolean) {
    if (irrelevant) video.irrelevant = true;
    else delete video.irrelevant;
    hasUnsaved = true;
    refreshVideoList();
  }

  function addTagToSelected() {
    if (!collection || selectedVideoIndex < 0 || !tagInput.trim()) return;
    const video = collection.videos[selectedVideoIndex];
    const tag = tagInput.trim().toLowerCase();
    if (!video.tags.includes(tag)) {
      video.tags.push(tag);
      hasUnsaved = true;
      refreshVideoList();
    }
    tagInput = "";
  }

  function removeTag(videoIndex: number, tagIndex: number) {
    if (!collection) return;
    collection.videos[videoIndex].tags.splice(tagIndex, 1);
    hasUnsaved = true;
    refreshVideoList();
  }

  let tagSuggestionsOpen = $state(false);

  const tagSuggestions = $derived(
    availableTags.filter((tag) => {
      if (collection && selectedVideoIndex >= 0 && collection.videos[selectedVideoIndex]?.tags.includes(tag)) {
        return false;
      }
      const q = tagInput.trim().toLowerCase();
      return !q || tag.includes(q);
    })
  );

  function selectTagSuggestion(tag: string) {
    tagInput = tag;
    addTagToSelected();
    tagSuggestionsOpen = false;
  }

  loadCollection().then(() => Promise.all([loadLocalFiles(), loadSegmentFolders()]));

  $effect(() => {
    void datasetKitDir;
    void resolveKitDir();
  });

  $effect(() => {
    void collection;
    void resolvedVideosDir;
    void annotateEveryN;
    void refreshAnnotateStatuses();
  });

  $effect(() => {
    void collection;
    void selectedVideoIndex;
    void datasetRefresh;
    void resolvedVideosDir;
    void refreshSegmentPreviews();
  });

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (!hasUnsaved || !collection) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => saveCollection(), 2000);
    return () => { if (saveTimer) clearTimeout(saveTimer); };
  });
</script>

<div class="h-full grid grid-rows-[auto_1fr]">
  <div class="border-b border-zinc-700 flex flex-wrap items-stretch text-sm whitespace-nowrap">
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
                    placeholder="https://youtube.com/watch?v=... or https://x.com/i/status/..."
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
      <button
        class="px-3 border-r border-zinc-700 py-1 whitespace-nowrap {selectedVideoIndices.length > 0 ? 'text-amber-400 hover:text-amber-300' : 'text-zinc-600'}"
        onclick={markSelectedIrrelevant}
        disabled={selectedVideoIndices.length < 1}
        title="Mark the selected videos as irrelevant instead of deleting them"
      >
        Mark selected irrelevant{selectedVideoIndices.length > 0 ? ` (${selectedVideoIndices.length})` : ""}
      </button>
        {#if openDatasetInNewTab}
          <button
            class="px-3 border-r border-zinc-700 py-1 whitespace-nowrap text-cyan-400 hover:text-cyan-300"
            onclick={openAllSegmentsAsDataset}
          >
           Open All Dataset
          </button>
        {/if}
       <button
         class="px-3 border-r border-zinc-700 py-1 {hideIrrelevant ? 'bg-zinc-600 text-zinc-100' : 'text-zinc-400 hover:text-zinc-200'}"
         onclick={() => { hideIrrelevant = !hideIrrelevant; refreshVideoList(); }}
         title="Toggle visibility of videos marked irrelevant"
       >
         {hideIrrelevant ? "Irrelevant hidden" : "Irrelevant shown"}
       </button>
       <button
         class="px-3 border-r border-zinc-700 py-1 whitespace-nowrap {soundMuted ? 'text-zinc-400 hover:text-zinc-200' : 'bg-zinc-600 text-zinc-100'}"
         onclick={() => soundMuted = !soundMuted}
         title="Toggle audio for video playback"
       >
         {soundMuted ? "Sound off" : "Sound on"}
       </button>
       <div class="relative border-r border-zinc-700">
         <button
           class="h-full px-3 py-1 hover:bg-zinc-700 {selectedTag ? 'bg-zinc-600 text-zinc-100' : ''}"
           onclick={() => tagFilterOpen = !tagFilterOpen}
         >
           {selectedTag ? `Tag: ${selectedTag}` : "All tags"}
         </button>
         {#if tagFilterOpen}
           <div class="absolute left-0 top-full z-20 min-w-full border border-zinc-600 bg-zinc-800 shadow-lg">
             <button
               class="block w-full whitespace-nowrap px-3 py-1.5 text-left hover:bg-zinc-700 {!selectedTag ? 'bg-zinc-600' : ''}"
               onclick={() => setTagFilter("")}
             >
               All tags
             </button>
             {#each availableTags as tag}
               <button
                 class="block w-full whitespace-nowrap px-3 py-1.5 text-left hover:bg-zinc-700 {selectedTag === tag ? 'bg-zinc-600' : ''}"
                 onclick={() => setTagFilter(tag)}
               >
                 {tag}
               </button>
             {/each}
           </div>
         {/if}
       </div>
       <span class="px-3 py-1 text-zinc-400">
         {visibleVideos.length} / {collection?.videos.length ?? 0} videos
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
      No local video files found in {resolvedVideosDir} — YouTube, X, and Telegram videos can still be previewed via embed
    </div>
  {/if}

  {#if collection}
    <div class="overflow-hidden grid grid-cols-[320px_minmax(0,1fr)] divide-x divide-zinc-700">
      <div class="overflow-y-auto">
        {#each visibleVideos as { video, index: i }, visibleIndex (i)}
          {@const rowSplitJob = getSplitJob(getVideoId(video))}
          {@const rowAnnotate = annotateIndicator(getVideoId(video))}
          <div
            class="w-full text-left px-3 py-2 border-b border-zinc-800 cursor-pointer {selectedVideoIndex === i ? 'bg-zinc-700' : isVideoSelected(i) ? 'bg-zinc-800/80 ring-1 ring-inset ring-zinc-500' : 'hover:bg-zinc-800'}"
            role="button"
            tabindex="0"
            onclick={() => openVideoDetails(i)}
            onkeydown={(event) => {
              if (event.key !== 'Enter' && event.key !== ' ') return;
              event.preventDefault();
              openVideoDetails(i);
            }}
          >
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="w-4 h-4 border flex-shrink-0 grid place-content-center text-[10px] {isVideoSelected(i) ? 'border-green-500 bg-green-600 text-white' : 'border-zinc-600 text-transparent'}"
                aria-pressed={isVideoSelected(i)}
                aria-label={isVideoSelected(i) ? 'Deselect video' : 'Select video'}
                onclick={(event: MouseEvent) => {
                  event.stopPropagation();
                  toggleVideoSelection(i, visibleIndex, event);
                }}
              >
                ✓
              </button>
              {#if resolvedFilePath(video)}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video
                  use:lazyThumb={`${convertFileSrc(resolvedFilePath(video)!)}#t=0.5`}
                  preload="metadata"
                  muted
                  class="w-16 h-10 object-cover flex-shrink-0"
                ></video>
              {:else if extractYouTubeId(video.url ?? "")}
                <img
                  src="https://img.youtube.com/vi/{extractYouTubeId(video.url!)}/default.jpg"
                  alt=""
                  class="w-16 h-10 object-cover flex-shrink-0"
                  loading="lazy"
                  onerror={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                />
              {:else if extractXId(video.url ?? "")}
                <div class="w-16 h-10 bg-zinc-800 flex-shrink-0 grid place-content-center">
                  <span class="text-zinc-500 text-xs font-bold">X</span>
                </div>
              {:else if extractTelegramId(video.url ?? "")}
                <div class="w-16 h-10 bg-zinc-800 flex-shrink-0 grid place-content-center">
                  <span class="text-zinc-500 text-xs font-bold">TG</span>
                </div>
              {:else}
                <div class="w-16 h-10 bg-zinc-800 flex-shrink-0 grid place-content-center text-zinc-600 text-xs">
                  N/A
                </div>
              {/if}
              <div class="min-w-0">
                <p class="truncate text-xs mb-1 {resolvedFilePath(video) ? 'text-zinc-300' : 'text-zinc-400'}">
                  {video.url || video.file || "(no url)"}
                </p>
                <div class="flex flex-wrap items-center gap-1 mb-1">
                  {#if video.irrelevant}
                    <span class="{statusPillBase} {statusPillClass.error}"><span aria-hidden="true">⊘</span>Irrelevant</span>
                  {/if}
                  {#if segmentStatus(video, i) === "uptodate"}
                    <span class="{statusPillBase} {statusPillClass.ok}"><span aria-hidden="true">✓</span>Up to date</span>
                  {:else if segmentStatus(video, i) === "stale"}
                    <span class="{statusPillBase} {statusPillClass.warn}"><span aria-hidden="true">↻</span>Outdated</span>
                  {/if}
                  {#if rowSplitJob?.status === "running"}
                    <span class="{statusPillBase} {statusPillClass.warn}"><span aria-hidden="true">↻</span>Splitting…</span>
                  {:else if rowSplitJob?.status === "done"}
                    <span class="{statusPillBase} {statusPillClass.ok}"><span aria-hidden="true">✓</span>Split done</span>
                  {:else if rowSplitJob?.status === "error"}
                    <span class="{statusPillBase} {statusPillClass.error}"><span aria-hidden="true">!</span>Split error</span>
                  {/if}
                  {#if rowAnnotate}
                    <span class="{statusPillBase} {statusPillClass[rowAnnotate.variant]}"><span aria-hidden="true">{statusIcon[rowAnnotate.variant]}</span>{rowAnnotate.label}</span>
                  {/if}
                  {#if resolvedFilePath(video)}
                    <span class="{statusPillBase} {statusPillClass.local}"><span aria-hidden="true">↓</span>Downloaded</span>
                  {/if}
                </div>
                {#if video.tags?.length}
                  <div class="flex flex-wrap items-center gap-1">
                    {#each video.tags as tag}
                      <span class="text-[11px] leading-4 text-[#a8adb8] whitespace-nowrap">#{tag}</span>
                    {/each}
                  </div>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>

      <div bind:this={detailsPane} class="min-w-0 overflow-y-auto p-4">
        {#if selectedVideoIndex >= 0 && selectedVideoIndex < collection.videos.length}
          {@const video = collection.videos[selectedVideoIndex]}
          {@const selectedVideoId = getVideoId(video)}
          {@const selectedSplitJob = getSplitJob(selectedVideoId)}
          {@const selectedAnnotateJob = getAnnotateJob(selectedVideoId)}
          {@const selectedAnnotateDisk = selectedVideoId ? annotateStatuses.get(selectedVideoId) : undefined}
          {@const selectedAnnotateIndicator = annotateIndicator(selectedVideoId)}
          <div class="space-y-4">
            <div class="space-y-3">
                <VideoPlayer
                  filePath={resolvedFilePath(video) ?? ""}
                  youtubeUrl={video.url ?? ""}
                   segments={video.keep_segments ?? []}
                   masks={video.masks ?? []}
                   muted={soundMuted}
                   highlightedSegmentIndex={highlightedSegIndex}
                  onSegmentHover={(i) => highlightedSegIndex = i}
                  onMasksChange={(masks: VideoMask[]) => {
                    video.masks = masks;
                    hasUnsaved = true;
                  }}
                  onSegmentsChange={(segs) => {
                     const seen = new Set<string>();
                    const deduped = segs.filter(s => {
                     const key = `${s[0]}|${s[1]}`;
                     if (seen.has(key)) return false;
                     seen.add(key);
                     return true;
                   });
                    video.keep_segments = sortSegments(deduped);
                   hasUnsaved = true;
                 }}
              />
            </div>

            {#if segmentStatus(video, selectedVideoIndex) === "uptodate"}
              <div class="text-sm text-green-400">Segments up to date</div>
            {:else if segmentStatus(video, selectedVideoIndex) === "stale"}
              <div class="text-sm text-yellow-400">Segments changed since last processing</div>
            {/if}

            <div class="ui-section space-y-2">
              <div class="ui-actions">
            {#if openDatasetInNewTab}
              <button
                class="px-3 py-1.5 text-sm bg-cyan-600 hover:bg-cyan-700 disabled:opacity-40 disabled:cursor-not-allowed"
                disabled={selectedVideoIndex < 0}
                onclick={openSelectedVideoAsDataset}
              >
                Open Video Dataset
              </button>
            {/if}


                <button
                  class="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed"
                  disabled={selectedSplitJob?.status === "running" || !selectedVideoId || !video.keep_segments?.length}
                  onclick={() => startSplit(video)}
                >
                  Split segments &amp; frames
                </button>

                <button
                  class="px-3 py-1.5 text-sm bg-zinc-700 hover:bg-zinc-600"
                  onclick={() => annotateSettingsOpen = !annotateSettingsOpen}
                >
                  {annotateSettingsOpen ? "Hide track settings" : "Track settings"}
                </button>
              </div>
              <div class="space-y-2">
              <div class="text-xs font-semibold text-zinc-300">Split segments &amp; frames</div>
              {#if selectedSplitJob}
              <div class="ui-actions">
                {#if selectedSplitJob?.status === "running"}
                  <button
                    class="px-3 py-1.5 text-sm bg-zinc-700 hover:bg-zinc-600"
                    onclick={() => cancelSplit(video)}
                  >
                    Cancel
                  </button>
                  <span class="text-xs text-blue-400">splitting…</span>
                {:else if selectedSplitJob?.status === "done"}
                  <span class="text-xs text-green-400">done</span>
                {:else if selectedSplitJob?.status === "error"}
                  <span class="text-xs text-red-400">failed</span>
                {/if}
              </div>
              {/if}
              {#if resolvedKitDir}
                <div class="text-[10px] text-zinc-600">dataset-kit: {resolvedKitDir}</div>
              {/if}
              {#if splitError}
                <div class="text-xs text-red-400">{splitError}</div>
              {/if}
              {#if selectedSplitJob && selectedSplitJob.log.length > 0}
                <pre class="max-h-48 overflow-y-auto bg-zinc-900 border border-zinc-700 p-2 text-[10px] leading-tight whitespace-pre-wrap">{selectedSplitJob.log.join("\n")}</pre>
              {/if}
            </div>

            <div class="space-y-2 border-t border-zinc-700 pt-3">
              <div class="flex flex-wrap items-center gap-2">
                <span class="text-xs font-semibold text-zinc-300">Tracking</span>
                {#if selectedAnnotateJob?.status === "running"}
                  <button
                    class="px-2 py-0.5 text-xs bg-zinc-700 hover:bg-zinc-600"
                    onclick={() => cancelAutoAnnotate(video)}
                  >
                    Cancel
                  </button>
                {/if}
                {#if selectedAnnotateIndicator}
                  <span class="{statusPillBase} {statusPillClass[selectedAnnotateIndicator.variant]}"><span aria-hidden="true">{statusIcon[selectedAnnotateIndicator.variant]}</span>{selectedAnnotateIndicator.label}</span>
                {/if}
                {#if selectedAnnotateDisk}
                  <span class="text-[10px] text-zinc-500">
                    {#if selectedAnnotateDisk.status === "completed"}
                      <span class="text-green-400">completed</span>
                    {:else if selectedAnnotateDisk.status === "partial"}
                      <span class="text-yellow-400">partial ({selectedAnnotateDisk.processedFrames}/{selectedAnnotateDisk.totalFrames} frames)</span>
                    {:else}
                      <span class="text-zinc-400">not started</span>
                    {/if}
                  </span>
                {/if}
                {#if anyAnnotateRunning && selectedAnnotateJob?.status !== "running"}
                  <span class="text-xs text-zinc-500">Another tracking run is running…</span>
                {/if}
              </div>
              {#if annotateSettingsOpen}
                <div class="flex flex-wrap items-center gap-2 text-xs">
                  <label class="flex items-center gap-1">
                    Class ID
                    <input
                      type="number"
                      class="w-16 px-2 py-1 border border-zinc-700 bg-zinc-800"
                      bind:value={annotateClassId}
                      min="0"
                    />
                  </label>
                  <label class="flex items-center gap-1">
                    Every N
                    <input
                      type="number"
                      class="w-16 px-2 py-1 border border-zinc-700 bg-zinc-800"
                      bind:value={annotateEveryN}
                      min="1"
                    />
                  </label>
                  <label class="flex items-center gap-1">
                    <input type="checkbox" bind:checked={annotateWriteAll} />
                    Write empty labels
                  </label>
                  <label class="flex items-center gap-1">
                    Score threshold
                    <input
                      type="number"
                      step="0.01"
                      min="0"
                      class="w-20 px-2 py-1 border border-zinc-700 bg-zinc-800"
                      bind:value={annotateScoreThreshold}
                    />
                  </label>
                  <label class="flex items-center gap-1">
                    Track device
                    <input
                      type="text"
                      class="w-24 px-2 py-1 border border-zinc-700 bg-zinc-800"
                      bind:value={annotateTrackDevice}
                      placeholder="auto"
                    />
                  </label>
                </div>
              {/if}
              {#if annotateError}
                <div class="text-xs text-red-400">{annotateError}</div>
              {/if}
              {#if selectedAnnotateJob && selectedAnnotateJob.log.length > 0}
                <pre class="max-h-48 overflow-y-auto bg-zinc-900 border border-zinc-700 p-2 text-[10px] leading-tight whitespace-pre-wrap">{selectedAnnotateJob.log.join("\n")}</pre>
              {/if}
            </div>

            </div>

            <div class="ui-section">
              <div class="min-w-0 space-y-4">
                <label class="block space-y-1">
                  <span class="text-sm text-zinc-400">URL</span>
                  <div class="ui-controls flex-nowrap">
                    <input
                      type="text"
                      class="ui-field min-w-0 flex-1 border border-zinc-700 bg-zinc-800"
                      value={video.url ?? ""}
                      oninput={(e) => { video.url = (e.target as HTMLInputElement).value || undefined; hasUnsaved = true; }}
                      placeholder="https://www.youtube.com/watch?v=... , https://x.com/i/status/... or https://t.me/channel/123"
                    />
                    {#if video.url}
                      <button
                        class="px-3 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700"
                        onclick={() => openVideoUrl(video.url!)}
                      >
                        Open
                      </button>
                    {/if}
                  </div>
                </label>

                <label class="block space-y-1">
                  <span class="text-sm text-zinc-400">Local video file</span>
                  <div class="ui-controls flex-nowrap">
                    <input
                      type="text"
                      class="ui-field min-w-0 flex-1 border border-zinc-700 bg-zinc-800"
                      value={video.file_path ?? ""}
                      oninput={(e) => { video.file_path = (e.target as HTMLInputElement).value || undefined; hasUnsaved = true; }}
                      placeholder={resolvedFilePath(video) ?? "D:\\Videos\\video.mp4"}
                    />
                    {#if resolvedFilePath(video)}
                      <button class="px-3 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700" onclick={() => navigator.clipboard.writeText(resolvedFilePath(video)!)}>Copy path</button>
                      <button class="px-3 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700" onclick={() => openFilePathInViewer(resolvedFilePath(video)!)}>Open</button>
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
                      <button class="text-zinc-400 hover:text-red-400" onclick={() => removeTag(selectedVideoIndex, ti)}>x</button>
                    </span>
                  {/each}
                  <div class="relative inline-block">
                    <form class="inline-flex" onsubmit={(e) => { e.preventDefault(); addTagToSelected(); tagSuggestionsOpen = false; }}>
                      <input type="text" class="w-24 px-2 py-0.5 text-sm border border-zinc-700 bg-zinc-800" bind:value={tagInput} placeholder="+ tag" onfocus={() => tagSuggestionsOpen = true} oninput={() => tagSuggestionsOpen = true} onblur={() => setTimeout(() => tagSuggestionsOpen = false, 100)} />
                    </form>
                    {#if tagSuggestionsOpen && tagSuggestions.length > 0}
                      <div class="absolute left-0 top-full z-20 min-w-full border border-zinc-600 bg-zinc-800 shadow-lg">
                        {#each tagSuggestions as tag}
                          <button type="button" class="block w-full whitespace-nowrap px-2 py-1 text-left text-sm hover:bg-zinc-700" onclick={() => selectTagSuggestion(tag)}>{tag}</button>
                        {/each}
                      </div>
                    {/if}
                  </div>
                </div>
              </div>
            </div>

            {#if segmentStatus(video, selectedVideoIndex) === "uptodate" && video.keep_segments && video.keep_segments.length > 0}
              <div class="space-y-2">
                <div class="text-xs text-zinc-500">Segment previews ({video.keep_segments.length}) — click to play/pause</div>
                <div class="grid grid-cols-3 gap-2" id="segment-grid">
                  {#each video.keep_segments as seg, si}
                    {@const segKey = `${selectedVideoIndex}-${si}`}
                    {@const segPath = segmentVideoPath(video, seg)}
                    {@const thumbPath = segmentFramePath(video, seg)}
                    {@const palette = numberToAccentPalette(si)}
                    {@const segFolder = segmentToFolderName(seg)}
                    {@const previewReady = segmentPreviewReady.get(segFolder) ?? false}
                    <div
                      class="relative overflow-hidden group transition-colors"
                      role="button"
                      tabindex={0}
                      aria-label={`Preview segment ${seg[0]} to ${seg[1]}`}
                      onmouseenter={() => highlightedSegIndex = si}
                      onmouseleave={() => highlightedSegIndex = -1}
                      style="background-color: {highlightedSegIndex === si ? palette.fillMuted : 'rgb(24 24 27)'};"
                      onclick={(e) => {
                        toggleSegmentPreview(e.currentTarget as HTMLElement, segKey);
                      }}
                      onkeydown={(e) => {
                        if (e.key !== "Enter" && e.key !== " ") return;
                        e.preventDefault();
                        toggleSegmentPreview(e.currentTarget as HTMLElement, segKey);
                      }}
                    >
                      {#if segPath}
                        <!-- svelte-ignore a11y_media_has_caption -->
                        <video
                          src="{convertFileSrc(segPath)}#t=0.1"
                          preload="metadata"
                          muted={soundMuted}
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
                        <button
                          type="button"
                          aria-label={`Open folder for segment ${seg[0]} to ${seg[1]}`}
                          class="absolute top-1 right-1 text-zinc-400 hover:text-zinc-200 bg-black/50 px-1 text-[10px] opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                          onclick={(e) => {
                            e.stopPropagation();
                            const dir = segPath!.replace(/[/\\][^/\\]+$/, '');
                            void openFilePath(dir);
                          }}
                        >
                          dir
                        </button>
                        {#if segmentHasDataset.get(segmentToFolderName(seg)) && openDatasetInNewTab}
                          <button
                            type="button"
                            aria-label={`Open dataset for segment ${seg[0]} to ${seg[1]}`}
                            class="absolute top-1 left-1 text-blue-400 hover:text-blue-300 bg-black/50 px-1 text-[10px] opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                            onclick={(e) => {
                              e.stopPropagation();
                              const fDir = segmentFramesDir(video, seg)!;
                              const lDir = segmentLabelsDir(video, seg)!;
                              openDatasetInNewTab!({ dirs: [{ imagesDir: fDir, labelsDir: lDir }] }, `${getVideoId(video)}-${segmentToFolderName(seg)}`);
                            }}
                          >
                            dataset
                          </button>
                        {/if}
                      {:else}
                        <div class="w-full aspect-video bg-zinc-800 grid place-content-center text-zinc-600 text-xs">
                          N/A
                        </div>
                      {/if}
                      <div
                        class="flex justify-between text-[10px] px-1 py-0.5 transition-colors"
                        style="background-color: {palette.fillMuted}; color: {palette.text};"
                      >
                        <span>{seg[0]}–{seg[1]}</span>
                        <span>{formatTimecode(parseTimecode(seg[1]) - parseTimecode(seg[0]))}</span>
                      </div>
                      <div class="flex flex-wrap items-center gap-1 px-1 py-0.5 text-[10px]">
                        <button
                          type="button"
                          class="px-1.5 py-0.5 bg-indigo-700 hover:bg-indigo-600 disabled:opacity-40 disabled:cursor-not-allowed"
                          disabled={anyAnnotateRunning}
                          title="Mark a box on the last frame and have LightFC track it across the segment"
                          onclick={(e) => {
                            e.stopPropagation();
                            void openTrackDialog(video, seg);
                          }}
                        >
                          {previewReady ? "re-track" : "track"}
                        </button>
                        {#if previewReady}
                          <button
                            type="button"
                            class="px-1.5 py-0.5 bg-green-700 hover:bg-green-600"
                            title="Copy the previewed labels into the segment's labels directory"
                            onclick={(e) => {
                              e.stopPropagation();
                              void saveSegmentPreview(video, seg);
                            }}
                          >
                            save
                          </button>
                          <button
                            type="button"
                            class="px-1.5 py-0.5 bg-zinc-700 hover:bg-zinc-600"
                            title="Delete the previewed labels"
                            onclick={(e) => {
                              e.stopPropagation();
                              void discardSegmentPreview(video, seg);
                            }}
                          >
                            discard
                          </button>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}

            <div class="ui-actions">
              {#if video.irrelevant}
                <button
                  class="bg-zinc-700 hover:bg-zinc-600 px-3 py-1.5 text-sm"
                  onclick={() => setVideoIrrelevant(video, false)}
                >
                  Unmark irrelevant
                </button>
              {:else}
                <button
                  class="bg-zinc-700 hover:bg-zinc-600 px-3 py-1.5 text-sm"
                  onclick={() => setVideoIrrelevant(video, true)}
                >
                  Mark irrelevant
                </button>
              {/if}
            </div>
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

  {#if trackDialog}
    <BboxMarkDialog
      imageSrc={trackDialog.imageSrc}
      onConfirm={confirmTrack}
      onCancel={() => (trackDialog = null)}
    />
  {/if}
</div>
