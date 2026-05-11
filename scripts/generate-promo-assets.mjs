import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const outDir = join(root, "docs/assets");
const frameDir = join(outDir, "promo-frames");

mkdirSync(outDir, { recursive: true });
rmSync(frameDir, { recursive: true, force: true });
mkdirSync(frameDir, { recursive: true });

const W = 1600;
const H = 1000;

function esc(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function pill(x, y, text, fill = "#e0f2fe", stroke = "#1d4ed8", color = "#0f172a") {
  const width = Math.max(112, text.length * 10 + 34);
  return `
    <rect x="${x}" y="${y}" width="${width}" height="42" rx="21" fill="${fill}" stroke="${stroke}" stroke-width="2"/>
    <text x="${x + width / 2}" y="${y + 27}" text-anchor="middle" font-size="18" font-weight="700" fill="${color}">${esc(text)}</text>
  `;
}

function arrow(x1, y1, x2, y2, color = "#0f766e") {
  return `
    <path d="M ${x1} ${y1} C ${(x1 + x2) / 2} ${y1}, ${(x1 + x2) / 2} ${y2}, ${x2} ${y2}" fill="none" stroke="${color}" stroke-width="8" stroke-linecap="round"/>
    <path d="M ${x2 - 22} ${y2 - 16} L ${x2} ${y2} L ${x2 - 22} ${y2 + 16}" fill="none" stroke="${color}" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
  `;
}

function shell(title, subtitle, body, footer = "") {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#fbf3e6"/>
      <stop offset="52%" stop-color="#eefcf9"/>
      <stop offset="100%" stop-color="#e7f0ff"/>
    </linearGradient>
    <linearGradient id="card" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#ffffff" stop-opacity="0.96"/>
      <stop offset="100%" stop-color="#f8fafc" stop-opacity="0.92"/>
    </linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="22" stdDeviation="24" flood-color="#0f172a" flood-opacity="0.16"/>
    </filter>
    <style>
      .brand { font-family: ui-rounded, "SF Pro Rounded", "Avenir Next", "Trebuchet MS", sans-serif; }
      .mono { font-family: "SF Mono", Menlo, Consolas, monospace; }
      .body { font-family: "Avenir Next", "SF Pro Display", system-ui, sans-serif; }
    </style>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#bg)"/>
  <circle cx="1330" cy="130" r="300" fill="#99f6e4" opacity="0.28"/>
  <circle cx="230" cy="860" r="340" fill="#fed7aa" opacity="0.32"/>
  <rect x="70" y="56" width="1460" height="888" rx="52" fill="url(#card)" filter="url(#shadow)" stroke="#ffffff" stroke-width="10"/>
  <text x="136" y="148" class="brand" font-size="64" font-weight="800" fill="#173b71">${esc(title)}</text>
  <text x="140" y="196" class="body" font-size="26" font-weight="600" fill="#0f766e">${esc(subtitle)}</text>
  ${body}
  ${footer}
</svg>`;
}

function mainScreenshot() {
  const body = `
    <g transform="translate(130 250)">
      <rect x="0" y="0" width="440" height="570" rx="34" fill="#102a43"/>
      <text x="42" y="74" class="brand" font-size="34" font-weight="800" fill="#f8fafc">Open Typeless</text>
      <text x="43" y="110" class="mono" font-size="18" font-weight="700" fill="#67e8f9">HARNESS</text>
      ${pill(42, 158, "Recording", "#dcfce7", "#16a34a", "#14532d")}
      ${pill(206, 158, "Learning", "#ccfbf1", "#0f766e", "#134e4a")}
      <rect x="42" y="246" width="356" height="110" rx="24" fill="#1e3a5f"/>
      <text x="72" y="292" class="body" font-size="24" font-weight="700" fill="#f8fafc">Voice Input</text>
      <text x="72" y="326" class="body" font-size="18" fill="#bae6fd">Hold hotkey. Speak naturally.</text>
      <rect x="42" y="382" width="356" height="110" rx="24" fill="#164e63"/>
      <text x="72" y="428" class="body" font-size="24" font-weight="700" fill="#f8fafc">Speech Skills</text>
      <text x="72" y="462" class="body" font-size="18" fill="#ccfbf1">Corrections are local memory.</text>
    </g>
    <g transform="translate(640 250)">
      <rect x="0" y="0" width="800" height="570" rx="34" fill="#ffffff" stroke="#dbeafe" stroke-width="3"/>
      <text x="52" y="70" class="body" font-size="30" font-weight="800" fill="#0f172a">Focused field demo</text>
      <text x="52" y="112" class="body" font-size="20" fill="#64748b">Dictate into the app you already use.</text>
      <rect x="52" y="158" width="690" height="156" rx="24" fill="#f8fafc" stroke="#cbd5e1" stroke-width="2"/>
      <text x="82" y="212" class="body" font-size="28" fill="#334155">我刚看了知呼上的回答，type script 泛型很好用。</text>
      <text x="82" y="268" class="body" font-size="18" fill="#94a3b8">Raw ASR transcript</text>
      ${arrow(390, 348, 390, 414)}
      <rect x="52" y="444" width="690" height="84" rx="24" fill="#ecfeff" stroke="#0f766e" stroke-width="3"/>
      <text x="82" y="497" class="body" font-size="30" font-weight="700" fill="#0f172a">我刚看了知乎上的回答，TypeScript 泛型很好用。</text>
    </g>
  `;
  return shell("Open Typeless Harness", "Speak, polish, insert, then learn from the correction.", body);
}

function learningScreenshot() {
  const body = `
    <g transform="translate(130 270)">
      <rect x="0" y="0" width="390" height="210" rx="32" fill="#ecfeff" stroke="#0f766e" stroke-width="4"/>
      <text x="44" y="72" class="body" font-size="34" font-weight="800" fill="#134e4a">1. Speak</text>
      <text x="44" y="128" class="body" font-size="24" fill="#0f172a">type script 泛型</text>
      <text x="44" y="166" class="body" font-size="20" fill="#64748b">Natural voice input</text>
      ${arrow(390, 105, 520, 105)}
    </g>
    <g transform="translate(650 270)">
      <rect x="0" y="0" width="390" height="210" rx="32" fill="#eff6ff" stroke="#2563eb" stroke-width="4"/>
      <text x="44" y="72" class="body" font-size="34" font-weight="800" fill="#1e3a8a">2. Polish</text>
      <text x="44" y="128" class="body" font-size="24" fill="#0f172a">TypeScript 泛型</text>
      <text x="44" y="166" class="body" font-size="20" fill="#64748b">LLM + retrieved skills</text>
      ${arrow(390, 105, 520, 105)}
    </g>
    <g transform="translate(1170 270)">
      <rect x="0" y="0" width="300" height="210" rx="32" fill="#fff7ed" stroke="#f59e0b" stroke-width="4"/>
      <text x="44" y="72" class="body" font-size="34" font-weight="800" fill="#92400e">3. Edit</text>
      <text x="44" y="128" class="body" font-size="24" fill="#0f172a">知呼 -> 知乎</text>
      <text x="44" y="166" class="body" font-size="20" fill="#64748b">User correction</text>
    </g>
    <g transform="translate(260 600)">
      <rect x="0" y="0" width="1080" height="150" rx="36" fill="#0f172a"/>
      <text x="54" y="62" class="body" font-size="30" font-weight="800" fill="#f8fafc">Local speech skill memory</text>
      <text x="54" y="108" class="mono" font-size="28" fill="#99f6e4">type script -> TypeScript     |     知呼 -> 知乎</text>
    </g>
    ${arrow(1260, 480, 1260, 600, "#f59e0b")}
    ${arrow(260, 675, 190, 380, "#0f766e")}
  `;
  return shell("A voice input loop that improves", "The correction after insertion becomes the memory before the next polish.", body);
}

function demoFrame(t) {
  const stages = [
    { k: "voice", title: "Voice input", text: "type script 泛型很好用", active: 0 },
    { k: "polish", title: "LLM polish", text: "TypeScript 泛型很好用。", active: 1 },
    { k: "edit", title: "User correction", text: "知呼 -> 知乎", active: 2 },
    { k: "learn", title: "Local learning", text: "Speech skill saved locally", active: 3 },
  ];
  const stage = stages[Math.min(stages.length - 1, Math.floor(t / 30))];
  const progress = Math.min(1, (t % 30) / 30);
  const cards = stages.map((s, i) => {
    const x = 150 + i * 330;
    const on = i <= stage.active;
    const fill = on ? ["#ecfeff", "#eff6ff", "#fff7ed", "#dcfce7"][i] : "#f8fafc";
    const stroke = on ? ["#0f766e", "#2563eb", "#f59e0b", "#16a34a"][i] : "#cbd5e1";
    return `
      <rect x="${x}" y="280" width="260" height="150" rx="30" fill="${fill}" stroke="${stroke}" stroke-width="4"/>
      <text x="${x + 130}" y="344" text-anchor="middle" class="body" font-size="26" font-weight="800" fill="#0f172a">${esc(s.title)}</text>
      <text x="${x + 130}" y="386" text-anchor="middle" class="body" font-size="18" fill="#475569">${esc(s.text)}</text>
      ${i < 3 ? arrow(x + 260, 355, x + 330, 355, on ? "#0f766e" : "#cbd5e1") : ""}
    `;
  }).join("");
  const body = `
    <text x="140" y="250" class="body" font-size="34" font-weight="800" fill="#0f172a">${esc(stage.title)}</text>
    <text x="140" y="735" class="body" font-size="34" font-weight="800" fill="#0f172a">Open Typeless Harness learns from edits after the text lands.</text>
    <rect x="140" y="780" width="1320" height="22" rx="11" fill="#e2e8f0"/>
    <rect x="140" y="780" width="${1320 * ((t + 1) / 120)}" height="22" rx="11" fill="#0f766e"/>
    ${cards}
    <g opacity="${0.25 + progress * 0.45}">
      <path d="M 330 540 C 520 480, 670 600, 860 540 S 1200 500, 1320 560" fill="none" stroke="#67e8f9" stroke-width="12" stroke-linecap="round"/>
    </g>
  `;
  return shell("Speak · Polish · Learn Locally", "A correction-native dictation loop for real work vocabulary.", body);
}

function writeSvg(name, content) {
  const path = join(outDir, name);
  writeFileSync(path, content, "utf8");
  return path;
}

function svgToPng(svgPath, pngPath) {
  execFileSync("sips", ["-s", "format", "png", svgPath, "--out", pngPath], { stdio: "ignore" });
}

const screenshots = [
  ["open-typeless-harness-product.svg", mainScreenshot()],
  ["open-typeless-harness-learning-loop.svg", learningScreenshot()],
];

for (const [name, svg] of screenshots) {
  const svgPath = writeSvg(name, svg);
  svgToPng(svgPath, join(outDir, name.replace(/\.svg$/, ".png")));
}

for (let i = 0; i < 120; i++) {
  const svgPath = join(frameDir, `frame-${String(i).padStart(4, "0")}.svg`);
  const pngPath = join(frameDir, `frame-${String(i).padStart(4, "0")}.png`);
  writeFileSync(svgPath, demoFrame(i), "utf8");
  svgToPng(svgPath, pngPath);
}

execFileSync("ffmpeg", [
  "-hide_banner", "-y",
  "-framerate", "12",
  "-i", join(frameDir, "frame-%04d.png"),
  "-vf", "scale=1280:-2",
  "-c:v", "libx264",
  "-pix_fmt", "yuv420p",
  "-movflags", "+faststart",
  join(outDir, "open-typeless-harness-demo.mp4"),
], { stdio: "inherit" });

execFileSync("ffmpeg", [
  "-hide_banner", "-y",
  "-i", join(outDir, "open-typeless-harness-demo.mp4"),
  "-vf", "fps=8,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
  "-loop", "0",
  join(outDir, "open-typeless-harness-demo.gif"),
], { stdio: "inherit" });

rmSync(frameDir, { recursive: true, force: true });

console.log("Generated promo assets in docs/assets");
