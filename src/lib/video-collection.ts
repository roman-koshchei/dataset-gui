export type VideoEntry = {
  url?: string;
  file?: string;
  file_path?: string;
  tags: string[];
  keep_segments?: string[][];
};

export type VideoCollection = {
  collection: string;
  source: string;
  split_fps: number;
  videos: VideoEntry[];
};

export function emptyVideoCollection(): VideoCollection {
  return {
    collection: "",
    source: "",
    split_fps: 30,
    videos: [{ tags: [] }],
  };
}

export function extractYouTubeId(url: string): string | null {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.replace(/^www\./, "");
    const pathParts = parsed.pathname.split("/").filter(Boolean);

    if (host === "youtu.be" && pathParts[0]?.match(/^[a-zA-Z0-9_-]{11}$/)) {
      return pathParts[0];
    }

    if (host.endsWith("youtube.com")) {
      const videoId = parsed.searchParams.get("v");
      if (videoId?.match(/^[a-zA-Z0-9_-]{11}$/)) {
        return videoId;
      }

      if (
        pathParts.length >= 2 &&
        ["embed", "shorts", "live"].includes(pathParts[0]) &&
        pathParts[1]?.match(/^[a-zA-Z0-9_-]{11}$/)
      ) {
        return pathParts[1];
      }
    }
  } catch {
    // Fall back to regex parsing for partial or malformed URLs.
  }

  const patterns = [
    /(?:youtube\.com\/watch\?.*v=)([a-zA-Z0-9_-]{11})/,
    /(?:youtu\.be\/)([a-zA-Z0-9_-]{11})/,
    /(?:youtube\.com\/(?:embed|shorts|live)\/)([a-zA-Z0-9_-]{11})/,
  ];
  for (const pattern of patterns) {
    const match = url.match(pattern);
    if (match) return match[1];
  }
  return null;
}

export function buildYouTubeEmbedUrl(url: string): string | null {
  const videoId = extractYouTubeId(url);
  return videoId ? `https://www.youtube-nocookie.com/embed/${videoId}?rel=0` : null;
}

export function extractXId(url: string): string | null {
  const match = url.match(/x\.com\/i\/status\/(\d+)/);
  return match ? match[1] : null;
}

export function extractVideoId(url: string): string | null {
  return extractYouTubeId(url) ?? extractXId(url);
}

export function extractFileStem(file: string): string | null {
  const name = file.replace(/[/\\]/g, "/").split("/").pop() ?? "";
  const stem = name.replace(/\.[^.]+$/, "");
  return stem || null;
}

export function findLocalVideo(
  files: string[],
  videoId: string | null
): string | undefined {
  if (!videoId) return undefined;
  return files.find((f) => f.includes(videoId));
}

export function parseTimecode(tc: string): number {
  const parts = tc.split(":");
  if (parts.length === 2) return Number(parts[0]) * 60 + Number(parts[1]);
  if (parts.length === 3) return Number(parts[0]) * 3600 + Number(parts[1]) * 60 + Number(parts[2]);
  return 0;
}

export function isValidTimecode(tc: string): boolean {
  if (!tc.trim()) return false;
  const parts = tc.split(":");
  if (parts.length < 2 || parts.length > 3) return false;
  return parts.every(p => /^\d+(\.\d+)?$/.test(p.trim()));
}

export function formatTimecode(seconds: number): string {
  seconds = Math.max(0, seconds);
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  const whole = Math.floor(s);
  const frac = s - whole;
  if (frac > 0.001) {
    const ms = Math.round(frac * 1000);
    return `${m}:${whole.toString().padStart(2, "0")}.${ms.toString().padStart(3, "0")}`;
  }
  return `${m}:${whole.toString().padStart(2, "0")}`;
}

export function segmentToFolderName(segment: string[]): string {
  return `${segment[0].replace(/:/g, "_")}-${segment[1].replace(/:/g, "_")}`;
}

export function folderNameToSegment(folder: string): string[] | null {
  const parts = folder.split("-");
  if (parts.length !== 2) return null;
  return [parts[0].replace(/_/g, ":"), parts[1].replace(/_/g, ":")];
}

export function segmentsMatchFolders(segments: string[][] | undefined, folders: string[]): boolean {
  if (!segments || segments.length === 0) return false;
  const expected = [...new Set(segments.map(segmentToFolderName))].sort();
  const actual = [...folders].sort();
  if (expected.length !== actual.length) return false;
  return expected.every((name, i) => name === actual[i]);
}
