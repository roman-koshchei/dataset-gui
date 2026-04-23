export type VideoEntry = {
  url: string;
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
    videos: [{ url: "", tags: [] }],
  };
}

export function extractYouTubeId(url: string): string | null {
  const patterns = [
    /(?:youtube\.com\/watch\?v=)([a-zA-Z0-9_-]{11})/,
    /(?:youtu\.be\/)([a-zA-Z0-9_-]{11})/,
    /(?:youtube\.com\/embed\/)([a-zA-Z0-9_-]{11})/,
  ];
  for (const pattern of patterns) {
    const match = url.match(pattern);
    if (match) return match[1];
  }
  return null;
}

export function findLocalVideo(
  files: string[],
  ytId: string | null
): string | undefined {
  if (!ytId) return undefined;
  return files.find((f) => f.includes(ytId));
}

export function parseTimecode(tc: string): number {
  const parts = tc.split(":").map(Number);
  if (parts.length === 2) return parts[0] * 60 + parts[1];
  if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2];
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
  const s = Math.round(seconds % 60);
  if (s === 60) return `${m + 1}:00`;
  return `${m}:${s.toString().padStart(2, "0")}`;
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
