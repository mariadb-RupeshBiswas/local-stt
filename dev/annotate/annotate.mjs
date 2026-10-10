// Renders annotated README screenshots: each raw capture plus numbered callouts with arrows.
// Usage: node dev/annotate/annotate.mjs   (reads dev/annotate/callouts.json, writes docs/screenshots/review/, gitignored)
// After the owner approves an image, copy it from docs/screenshots/review/ into docs/screenshots/.
import { chromium } from "../ui-tests/node_modules/playwright-core/index.mjs";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const RAW = path.join(ROOT, "docs/screenshots/raw");
const OUT = path.join(ROOT, "docs/screenshots/review");
const spec = JSON.parse(fs.readFileSync(path.join(ROOT, "dev/annotate/callouts.json"), "utf8"));
const CHROME = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

// The real pixel size, from the PNG header, so callout coordinates always match the capture.
function pngSize(buf) {
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

// Keeps only the chunks needed to draw (IHDR, PLTE, IDAT, IEND); text, time, color profile and other metadata go.
function stripPng(buf) {
  const parts = [buf.subarray(0, 8)];
  for (let at = 8; at < buf.length;) {
    const end = at + 12 + buf.readUInt32BE(at);
    const type = buf.toString("latin1", at + 4, at + 8);
    if (["IHDR", "PLTE", "IDAT", "IEND"].includes(type)) parts.push(buf.subarray(at, end));
    at = end;
  }
  return Buffer.concat(parts);
}

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
    const y = c.labelY ?? 40 + i * ((height - 80) / Math.max(1, callouts.length - 1));
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

fs.mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch({ executablePath: CHROME, headless: true });
for (const shot of spec) {
  const raw = path.join(RAW, shot.raw);
  if (!fs.existsSync(raw)) {
    console.log(`skip ${shot.out}: ${shot.raw} not captured`);
    continue;
  }
  const bytes = fs.readFileSync(raw);
  const data = "data:image/png;base64," + bytes.toString("base64");
  // callout x, y and labelY are raw-capture pixels; everything shrinks by the same scale
  const scale = shot.scale ?? 0.5;
  const size = pngSize(bytes);
  const width = Math.round(size.width * scale);
  const height = Math.round(size.height * scale);
  const callouts = shot.callouts.map((c) => ({ ...c, x: c.x * scale, y: c.y * scale, labelY: c.labelY === undefined ? undefined : c.labelY * scale }));
  const ctx = await browser.newContext({ viewport: { width: width + 360, height }, deviceScaleFactor: 2 });
  const p = await ctx.newPage();
  await p.setContent(page(data, width, height, callouts, shot.side ?? "right"));
  const png = await p.screenshot({ omitBackground: true });
  fs.writeFileSync(path.join(OUT, shot.out), stripPng(png));
  await ctx.close();
  console.log(`wrote review/${shot.out}`);
}
await browser.close();
