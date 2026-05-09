export function numberToTailwindBorder(n: number) {
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

export function numberToOutlineColor(n: number) {
  const colors = [
    "rgb(239 68 68)",
    "rgb(249 115 22)",
    "rgb(245 158 11)",
    "rgb(234 179 8)",
    "rgb(132 204 22)",
    "rgb(34 197 94)",
    "rgb(16 185 129)",
    "rgb(20 184 166)",
    "rgb(6 182 212)",
    "rgb(14 165 233)",
    "rgb(59 130 246)",
    "rgb(99 102 241)",
    "rgb(139 92 246)",
    "rgb(168 85 247)",
    "rgb(217 70 239)",
    "rgb(236 72 153)",
    "rgb(244 63 94)",
  ];

  const index = Math.abs(n) % colors.length;
  return colors[index];
}

export function numberToTailwindBg(n: number) {
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

export function numberToAccentPalette(n: number) {
  const hue = (Math.abs(n) * 137.508) % 360;

  return {
    solid: `hsl(${hue}, 78%, 62%)`,
    fill: `hsla(${hue}, 78%, 58%, 0.35)`,
    fillMuted: `hsla(${hue}, 78%, 58%, 0.18)`,
    fillStrong: `hsla(${hue}, 84%, 64%, 0.52)`,
    border: `hsla(${hue}, 84%, 70%, 0.75)`,
    borderStrong: `hsl(${hue}, 92%, 78%)`,
    text: `hsl(${hue}, 92%, 84%)`,
  };
}

export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
