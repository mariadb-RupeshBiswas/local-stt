// Renders annotated README screenshots: each raw capture plus numbered callouts with arrows.
// Usage: node dev/annotate/annotate.mjs   (reads dev/annotate/callouts.json, writes docs/screenshots/tour-*.png)
import { chromium } from "../ui-tests/node_modules/playwright-core/index.mjs";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const RAW = path.join(ROOT, "docs/screenshots/raw");
const OUT = path.join(ROOT, "docs/screenshots");
const spec = JSON.parse(fs.readFileSync(path.join(ROOT, "dev/annotate/callouts.json"), "utf8"));
const CHROME = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

function escapeXml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Callouts sit in a margin beside the image; each arrow runs from its label to the feature.
function page(image, width, height, callouts, side) {
  const margin = 360;
  const total = width + margin;
  const left = side === "left" ? margin : 0;
  const labelX = side === "left" ? 16 : width + 24;
  const rows = callouts.map((c, i) => {
    const y = c.labelY ?? 40 + i * ((height - 80) / Math.max(1, callouts.length - 1 || 1));
    const tx = left + c.x;
    const ty = c.y;
    const ax = side === "left" ? labelX + 320 : labelX - 8;
    return `
      <path d="M ${ax} ${y} C ${(ax + tx) / 2} ${y}, ${(ax + tx) / 2} ${ty}, ${tx} ${ty}" class="arrow"/>
      <circle cx="${tx}" cy="${ty}" r="6" class="dot"/>
      <g transform="translate(${labelX}, ${y - 22})">
        <circle cx="14" cy="14" r="13" class="badge"/><text x="14" y="19" class="num">${i + 1}</text>
        <text x="36" y="13" class="title">${escapeXml(c.title)}</text>
        <text x="36" y="33" class="body">${escapeXml(c.body)}</text>
      </g>`;
  }).join("");
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    html,body{margin:0;background:transparent}
    svg{display:block;font-family:-apple-system,"SF Pro Text","Segoe UI",system-ui,sans-serif}
    .arrow{fill:none;stroke:#0a84ff;stroke-width:2.5;stroke-linecap:round}
    .dot{fill:#0a84ff;stroke:#fff;stroke-width:2}
    .badge{fill:#0a84ff}
    .num{fill:#fff;font-size:14px;font-weight:700;text-anchor:middle}
    .title{fill:#1d1d1f;font-size:15px;font-weight:600}
    .body{fill:#515154;font-size:13px}
    .bg{fill:#f5f5f7}
  </style></head><body>
  <svg width="${total}" height="${height}" viewBox="0 0 ${total} ${height}" xmlns="http://www.w3.org/2000/svg">
    <rect class="bg" x="0" y="0" width="${total}" height="${height}" rx="18"/>
    <image href="${image}" x="${left}" y="0" width="${width}" height="${height}"/>
    ${rows}
  </svg></body></html>`;
}

const browser = await chromium.launch({ executablePath: CHROME, headless: true });
for (const shot of spec) {
  const raw = path.join(RAW, shot.raw);
  if (!fs.existsSync(raw)) {
    console.log(`skip ${shot.out}: ${shot.raw} not captured`);
    continue;
  }
  const data = "data:image/png;base64," + fs.readFileSync(raw).toString("base64");
  const scale = shot.scale ?? 0.5;
  const width = Math.round(shot.width * scale);
  const height = Math.round(shot.height * scale);
  const callouts = shot.callouts.map((c) => ({ ...c, x: c.x * scale, y: c.y * scale, labelY: c.labelY }));
  const ctx = await browser.newContext({ viewport: { width: width + 360, height }, deviceScaleFactor: 2 });
  const p = await ctx.newPage();
  await p.setContent(page(data, width, height, callouts, shot.side ?? "right"));
  await p.screenshot({ path: path.join(OUT, shot.out), omitBackground: true });
  await ctx.close();
  console.log(`wrote ${shot.out}`);
}
await browser.close();
