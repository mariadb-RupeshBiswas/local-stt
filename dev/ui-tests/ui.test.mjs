import { chromium } from "playwright-core";
import fs from "node:fs";

import { fileURLToPath } from "node:url";
import path from "node:path";

// Repo root, two levels up from dev/ui-tests.
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const UI = "file://" + ROOT + "/dev/preview.html";
// Uses an installed Chrome: CI runners ship one, so no browser download is needed.
const CHROME = process.env.CHROME_PATH
  || (process.platform === "darwin" ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    : process.platform === "win32" ? "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
    : "/usr/bin/google-chrome");
const browser = await chromium.launch({ executablePath: CHROME, headless: true });
let failed = 0;
function check(name, ok, detail) {
  if (!ok) failed++;
  console.log((ok ? "PASS " : "FAIL ") + name + (detail !== undefined ? "  " + JSON.stringify(detail) : ""));
}
async function open(query, opts = {}) {
  const ctx = await browser.newContext({
    viewport: { width: 1320, height: opts.tall ? 1500 : 900 },
    deviceScaleFactor: 1,
    colorScheme: opts.dark ? "dark" : "light",
    reducedMotion: opts.reduced ? "reduce" : "no-preference",
  });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  page.on("dialog", (d) => d.dismiss());
  if (opts.media) {
    const client = await ctx.newCDPSession(page);
    await client.send("Emulation.setEmulatedMedia", { features: opts.media });
  }
  await page.goto(UI + query);
  await page.waitForTimeout(500);
  return { page, ctx, errors };
}
const calls = (page) => page.evaluate(() => window.__calls.slice());

// ---------- overlay: live capsule ----------
{
  const { page, ctx, errors } = await open("?view=overlays&scheme=light");
  const live = page.locator(".pv-live").first();
  const ov = live.locator(".ov");
  const cap = live.locator(".ov-capsule");
  const read = () => cap.evaluate((c) => { const cs = getComputedStyle(c); return { opacity: +cs.opacity, transform: cs.transform, filter: cs.filter, cursor: cs.cursor }; });
  const btn = (name) => live.getByText(name, { exact: true });

  check("overlay starts hidden", (await read()).opacity === 0);
  await btn("Dictate").click();
  await page.waitForTimeout(70);
  const mid = await read();
  check("entrance is an opacity-only fade (no scale, no blur)", mid.opacity > 0 && mid.opacity < 1 && mid.transform === "none" && mid.filter === "none", mid);
  await page.waitForTimeout(400);
  check("entrance settled", (await read()).opacity === 1);
  check("recording state + label", (await ov.getAttribute("data-state")) === "recording" && (await live.locator(".ov-label").textContent()) === "Recording");
  check("label is role=status aria-live=polite", (await live.locator(".ov-label").getAttribute("role")) === "status" && (await live.locator(".ov-label").getAttribute("aria-live")) === "polite");
  check("capsule is a drag region", (await cap.getAttribute("data-tauri-drag-region")) !== null);
  check("dot breathes while recording", (await live.locator(".ov-dot").evaluate((d) => getComputedStyle(d).animationName)) === "ov-breathe");
  check("timer shows m:ss", /^\d:\d\d$/.test(await live.locator(".ov-time").textContent()));
  const timerInk = await live.locator(".ov-time").evaluate((t) => getComputedStyle(t).color);
  const labelInk = await live.locator(".ov-label").evaluate((t) => getComputedStyle(t).color);
  check("timer text is as opaque as the label (contrast fix)", timerInk === labelInk, { timerInk, labelInk });
  await page.waitForTimeout(3700);
  check("transcribing after recording", (await ov.getAttribute("data-state")) === "transcribing");
  await page.waitForTimeout(1200);
  check("done shows Pasted", (await ov.getAttribute("data-state")) === "done" && (await live.locator(".ov-label").textContent()) === "Pasted");
  await page.waitForTimeout(700);
  check("auto-hides after done", (await ov.getAttribute("data-visible")) === "false" && (await read()).opacity === 0);

  // warning: amber triangle, label from payload, wraps, about 3 s, fades at ~2850 ms
  await btn("Warn").click();
  await page.waitForTimeout(4400 + 500);
  check("warning state with payload label", (await ov.getAttribute("data-state")) === "warning" && (await live.locator(".ov-label").textContent()) === "Copied. Allow Accessibility to paste");
  const warn = await live.evaluate((root) => {
    const lab = root.querySelector(".ov-label");
    return {
      warnOpacity: +getComputedStyle(root.querySelector(".ov-warn")).opacity,
      dotOpacity: +getComputedStyle(root.querySelector(".ov-dot")).opacity,
      meter: getComputedStyle(root.querySelector(".ov-meter")).display,
      clipped: lab.scrollHeight > lab.clientHeight + 1,
      lines: Math.round(lab.clientHeight / parseFloat(getComputedStyle(lab).lineHeight)),
      timeHidden: root.querySelector(".ov-time").hidden,
    };
  });
  check("warning: triangle shown, dot and meter hidden, timer hidden", warn.warnOpacity > 0.9 && warn.dotOpacity === 0 && warn.meter === "none" && warn.timeHidden, warn);
  check("warning text wraps to 2 lines and is not clipped", warn.lines === 2 && !warn.clipped, warn);
  await page.waitForTimeout(2100); // about 2.6 s after the state
  check("warning still visible at ~2.6 s", (await ov.getAttribute("data-visible")) === "true");
  await page.waitForTimeout(550); // about 3.15 s
  check("warning faded out by ~3 s", (await ov.getAttribute("data-visible")) === "false");

  // positioning: no timer, no auto-hide, grab cursor
  await btn("Move Pill").click();
  await page.waitForTimeout(500);
  const pos = await read();
  check("positioning shows Drag me anywhere", (await ov.getAttribute("data-state")) === "positioning" && (await live.locator(".ov-label").textContent()) === "Drag me anywhere" && pos.opacity === 1, pos);
  check("positioning cursor is grab", pos.cursor === "grab", pos.cursor);
  check("positioning hides meter and timer", (await live.locator(".ov-meter").evaluate((m) => getComputedStyle(m).display)) === "none" && (await live.locator(".ov-time").isHidden()));
  await page.waitForTimeout(4500);
  check("positioning never auto-hides", (await ov.getAttribute("data-visible")) === "true");

  // the app hides the window natively on Done, then the next recording must fade in again
  await btn("Hide (cancel or Done)").click();
  await btn("Dictate").click();
  await page.waitForTimeout(70);
  const again = await read();
  check("entrance runs again after a native hide from positioning", again.opacity > 0 && again.opacity < 1, again);
  await page.waitForTimeout(500);
  await btn("Hide (cancel or Done)").click(); // Esc cancel: recording hidden natively
  await btn("Dictate").click();
  await page.waitForTimeout(70);
  const again2 = await read();
  check("entrance runs again after a cancelled recording", again2.opacity > 0 && again2.opacity < 1, again2);

  check("no page errors (overlay live)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- overlay: solid flag and reduced motion ----------
{
  const { page, ctx } = await open("?view=overlays&scheme=light");
  const solidCount = await page.locator(".pv-panel .ov.solid").count();
  check("no solid capsule by default", solidCount === 0, solidCount);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=overlays&scheme=light&solid=1");
  const info = await page.evaluate(() => {
    const cells = [...document.querySelectorAll(".pv-grid .ov")];
    const bg = getComputedStyle(cells[0].querySelector(".ov-capsule")).backgroundColor;
    return { total: cells.length, solid: cells.filter((c) => c.classList.contains("solid")).length, bg };
  });
  check("overlay-theme solid:true switches every capsule to the opaque style", info.total > 0 && info.solid === info.total && /^rgb\(/.test(info.bg), info);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=overlays&scheme=light", { reduced: true });
  const live = page.locator(".pv-live").first();
  await live.getByText("Dictate", { exact: true }).click();
  await page.waitForTimeout(500);
  const info = await live.evaluate((root) => {
    const ov = root.querySelector(".ov");
    return {
      cls: ov.classList.contains("reduce-motion"),
      dotAnim: getComputedStyle(ov.querySelector(".ov-dot")).animationName,
      barsDisplay: getComputedStyle(ov.querySelector(".ov-bars-pill")).display,
      levelDisplay: getComputedStyle(ov.querySelector(".ov-level")).display,
    };
  });
  check("reduce-motion: class, no dot animation, static level bar", info.cls && info.dotAnim === "none" && info.barsDisplay === "none" && info.levelDisplay === "block", info);
  await ctx.close();
}

// ---------- overlay: hands-free label ----------
{
  const { page, ctx, errors } = await open("?view=overlays&scheme=light");
  const live = page.locator(".pv-live").first();
  const ov = live.locator(".ov");
  const label = live.locator(".ov-label");
  // the preview glass is 220 px for every theme and the app's waveform window is 260 px, so waveform is measured on the tighter box
  const fit = () => live.evaluate((root) => {
    const lab = root.querySelector(".ov-label");
    const time = root.querySelector(".ov-time");
    const meter = root.querySelector(".ov-meter");
    const cap = root.querySelector(".ov-capsule");
    const box = (n) => n.getBoundingClientRect();
    return {
      label: lab.textContent,
      clipped: lab.scrollWidth > lab.clientWidth,
      capsuleOverflow: cap.scrollWidth > cap.clientWidth,
      labelEndsBeforeMeter: box(lab).right <= box(meter).left,
      timeShown: !time.hidden && getComputedStyle(time).display !== "none",
      timeEndsBeforeMeter: box(time).right <= box(meter).left,
    };
  });
  await live.getByRole("button", { name: "Hands-free" }).click();
  await page.waitForTimeout(400);
  check("hands-free demo starts as Recording", (await label.textContent()) === "Recording" && (await ov.getAttribute("data-state")) === "recording");
  const minOpacity = await live.evaluate((root) => new Promise((resolve) => {
    const cap = root.querySelector(".ov-capsule");
    const t0 = performance.now();
    let min = 1;
    (function tick() {
      min = Math.min(min, +getComputedStyle(cap).opacity);
      if (performance.now() - t0 < 1500) requestAnimationFrame(tick); else resolve(min);
    })();
  }));
  check("switching to hands-free keeps the pill on screen (no fade restart)", minOpacity === 1, minOpacity);
  check("hands-free label with state recording", (await label.textContent()) === "Hands-free" && (await ov.getAttribute("data-state")) === "recording");
  check("timer keeps running in hands-free", /^\d:\d\d$/.test(await live.locator(".ov-time").textContent()));
  const pill = await fit();
  check("Hands-free fits the pill with its timer", pill.label === "Hands-free" && !pill.clipped && !pill.capsuleOverflow && pill.timeShown && pill.labelEndsBeforeMeter && pill.timeEndsBeforeMeter, pill);

  await live.getByRole("button", { name: "waveform", exact: true }).click();
  await page.waitForTimeout(300);
  const wave = await fit();
  check("Hands-free fits the waveform theme", wave.label === "Hands-free" && !wave.clipped && !wave.capsuleOverflow && wave.labelEndsBeforeMeter, wave);

  await live.getByRole("button", { name: "minimal", exact: true }).click();
  await page.waitForTimeout(300);
  const dot = await live.evaluate((root) => {
    const text = root.querySelector(".ov-text").getBoundingClientRect();
    const cap = root.querySelector(".ov-capsule").getBoundingClientRect();
    return { textSize: [Math.round(text.width), Math.round(text.height)], capsule: [Math.round(cap.width), Math.round(cap.height)], dotOpacity: +getComputedStyle(root.querySelector(".ov-dot")).opacity, meter: getComputedStyle(root.querySelector(".ov-meter")).display };
  });
  check("minimal theme shows only the dot in hands-free", dot.capsule.join("x") === "44x44" && dot.textSize.join("x") === "1x1" && dot.dotOpacity > 0 && dot.meter === "none", dot);
  check("minimal keeps the hands-free label for screen readers", (await label.textContent()) === "Hands-free" && (await label.getAttribute("role")) === "status");
  check("no page errors (overlay hands-free)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- live transcript popover ----------
for (const dark of [false, true]) {
  const { page, ctx, errors } = await open("?view=live&scheme=" + (dark ? "dark" : "light"), { dark });
  const mode = dark ? "dark" : "light";
  const cell = (name) => page.locator(".pv-livecell[data-name=" + name + "]");
  const emit = (name, payload) => page.evaluate(([n, p]) => window.__live.bus.emit(n, p), [name, payload]);
  const demoText = cell("demo").locator(".lv-text");
  const box = (loc) => loc.evaluate((e) => { const r = e.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, width: r.width, height: r.height }; });
  const lineClamp = (loc) => loc.evaluate((e) => ({ height: e.clientHeight, scrollHeight: e.scrollHeight, atEnd: e.scrollTop + e.clientHeight >= e.scrollHeight - 1, clipped: e.classList.contains("clipped") }));

  // placeholder, live region, window size
  const empty = cell("listening").locator(".lv-text");
  check(mode + ": empty state is a quiet Listening... placeholder", (await empty.textContent()) === "Listening..." && (await empty.evaluate((e) => e.classList.contains("is-empty"))));
  check(mode + ": text is a polite status region", (await empty.getAttribute("role")) === "status" && (await empty.getAttribute("aria-live")) === "polite");
  const card = await box(cell("short").locator(".lv-card"));
  check(mode + ": card fills the 380 x 96 window", Math.round(card.width) === 380 && Math.round(card.height) === 96, card);
  check(mode + ": initial smartFormatActive from get_state shows the tag before any text", await cell("listening").locator(".lv-tag").isVisible());

  // replacement, not delta
  await emit("live-text", { text: "hello wor", formatted: true });
  check(mode + ": live-text shows the text", (await demoText.textContent()) === "hello wor");
  await emit("live-text", { text: "hello world", formatted: true });
  check(mode + ": next live-text replaces, it does not append", (await demoText.textContent()) === "hello world" && (await demoText.locator(".lv-new").textContent()) === "ld");
  await emit("live-text", { text: "hallo world", formatted: true });
  check(mode + ": a revised earlier word replaces the old text", (await demoText.textContent()) === "hallo world" && (await demoText.locator(".lv-new").textContent()) === "allo world");
  await emit("live-text", { text: "   spaced out", formatted: true });
  check(mode + ": leading spaces are dropped", (await demoText.textContent()) === "spaced out");
  await emit("live-text", { text: "one two three four five six", formatted: true });
  await emit("live-text", { text: "three four five six seven", formatted: true });
  check(mode + ": when old words drop off the front, only the new tail fades in", (await demoText.textContent()) === "three four five six seven" && (await demoText.locator(".lv-new").textContent()) === " seven", await demoText.textContent());
  const fade = await demoText.locator(".lv-new").evaluate((e) => { const cs = getComputedStyle(e); return { name: cs.animationName, duration: cs.animationDuration }; });
  check(mode + ": new words fade in over 120 ms", fade.name === "lv-in" && fade.duration === "0.12s", fade);
  await emit("live-reset", {});
  check(mode + ": live-reset clears the text back to the placeholder", (await demoText.textContent()) === "Listening..." && (await demoText.locator(".lv-new").count()) === 0);
  await emit("live-text", { text: "<img src=x onerror=window.__pwned2=1> & <b>bold</b>", formatted: true });
  check(mode + ": markup in a transcript is shown as text", (await demoText.textContent()).startsWith("<img src=x onerror") && (await cell("demo").locator(".lv-text img, .lv-text b").count()) === 0 && (await page.evaluate(() => window.__pwned2)) === undefined);

  // three lines, newest in view
  const short = await lineClamp(cell("short").locator(".lv-text"));
  check(mode + ": short text takes one line and is not clipped", short.height === 18 && !short.clipped, short);
  const long = await lineClamp(cell("long").locator(".lv-text"));
  check(mode + ": long text clamps to 3 lines, clips at the top and keeps the newest line in view", long.height === 54 && long.scrollHeight > long.height && long.atEnd && long.clipped, long);
  const mask = await cell("long").locator(".lv-text").evaluate((e) => getComputedStyle(e).maskImage || getComputedStyle(e).webkitMaskImage);
  check(mode + ": clipped text fades at the top", /linear-gradient/.test(mask), mask);

  // the tag is fixed in the corner away from the pill and follows formatted
  const tagShort = cell("short").locator(".lv-tag");
  const tagOff = cell("unformatted").locator(".lv-tag");
  check(mode + ": tag says Formats on paste when formatted is true", (await tagShort.isVisible()) && (await tagShort.textContent()) === "Formats on paste");
  check(mode + ": tag is hidden when formatted is false", await tagOff.isHidden());
  const tagBefore = await box(cell("demo").locator(".lv-tag"));
  await emit("live-text", { text: "one", formatted: true });
  const tagA = await box(cell("demo").locator(".lv-tag"));
  await emit("live-text", { text: "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty", formatted: true });
  const tagB = await box(cell("demo").locator(".lv-tag"));
  check(mode + ": tag does not move as the text grows", tagA.top === tagB.top && tagA.right === tagB.right && tagBefore.top === tagA.top, { tagBefore, tagA, tagB });
  await emit("live-text", { text: "one two", formatted: false });
  check(mode + ": tag follows each live-text formatted flag", await cell("demo").locator(".lv-tag").isHidden());
  await emit("live-text", { text: "one two", formatted: true });
  check(mode + ": tag comes back with formatted true", await cell("demo").locator(".lv-tag").isVisible());
  const textBox = await box(cell("long").locator(".lv-text"));
  const tagBox = await box(cell("long").locator(".lv-tag"));
  check(mode + ": tag never overlaps three lines of text above the pill", tagBox.bottom <= textBox.top, { tagBox, textBox });

  // placement: below class follows live-place and the text sits next to the pill
  check(mode + ": above-the-pill popover has no below class", (await cell("long").locator(".lv").evaluate((e) => e.classList.contains("below"))) === false);
  check(mode + ": below variant has the below class", await cell("below").locator(".lv").evaluate((e) => e.classList.contains("below")));
  await emit("live-place", { below: true });
  const flipped = await cell("demo").locator(".lv").evaluate((e) => e.classList.contains("below"));
  await emit("live-place", { below: false });
  const back = await cell("demo").locator(".lv").evaluate((e) => e.classList.contains("below"));
  check(mode + ": live-place toggles the below class both ways", flipped === true && back === false, { flipped, back });
  const aboveCard = await box(cell("short").locator(".lv-card"));
  const aboveText = await box(cell("short").locator(".lv-text"));
  const belowCard = await box(cell("below-short").locator(".lv-card"));
  const belowText = await box(cell("below-short").locator(".lv-text"));
  check(mode + ": text hugs the pill edge (bottom above, top below)", Math.round(aboveCard.bottom - aboveText.bottom) === 14 && Math.round(belowText.top - belowCard.top) === 14, { aboveCard, aboveText, belowCard, belowText });
  const belowTag = await box(cell("below").locator(".lv-tag"));
  const belowLong = await box(cell("below").locator(".lv-text"));
  check(mode + ": below variant moves the tag to the far corner, clear of the text", belowTag.top >= belowLong.bottom, { belowTag, belowLong });

  // solid card for Windows
  const solidInfo = await cell("solid").evaluate((c) => {
    const card = c.querySelector(".lv-card");
    const parse = (s) => { const m = s.match(/rgba?\(([^)]+)\)/)[1].split(/[ ,/]+/).filter(Boolean).map(Number); return { r: m[0], g: m[1], b: m[2], a: m[3] === undefined ? 1 : m[3] }; };
    return { solid: c.querySelector(".lv").classList.contains("solid"), alpha: parse(getComputedStyle(card).backgroundColor).a, radius: getComputedStyle(card).borderRadius };
  });
  check(mode + ": solid flag draws an opaque rounded card", solidInfo.solid && solidInfo.alpha === 1 && solidInfo.radius === "22px", solidInfo);
  await page.evaluate(() => window.__live.bus.emit("overlay-theme", { theme: "pill", solid: true, reducedMotion: false }));
  await emit("live-reset", {});
  const contrast = await page.evaluate(() => {
    const parse = (c) => { const m = c.match(/rgba?\(([^)]+)\)/)[1].split(/[ ,/]+/).filter(Boolean).map(Number); return { r: m[0], g: m[1], b: m[2], a: m[3] === undefined ? 1 : m[3] }; };
    const over = (fg, bg) => ({ r: fg.r * fg.a + bg.r * (1 - fg.a), g: fg.g * fg.a + bg.g * (1 - fg.a), b: fg.b * fg.a + bg.b * (1 - fg.a), a: 1 });
    const lin = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    const lum = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    const ratio = (a, b) => { const l = [lum(a), lum(b)].sort((x, y) => y - x); return +((l[0] + 0.05) / (l[1] + 0.05)).toFixed(2); };
    const c = document.querySelector(".pv-livecell[data-name=demo]");
    const cardBg = parse(getComputedStyle(c.querySelector(".lv-card")).backgroundColor);
    const ph = c.querySelector(".lv-text");
    const tag = c.querySelector(".lv-tag");
    const tagBg = over(parse(getComputedStyle(tag).backgroundColor), cardBg);
    return { placeholder: ratio(over(parse(getComputedStyle(ph).color), cardBg), cardBg), tag: ratio(over(parse(getComputedStyle(tag).color), tagBg), tagBg) };
  });
  check(mode + ": placeholder and tag pass 4.5:1 on the solid card", contrast.placeholder >= 4.5 && contrast.tag >= 4.5, contrast);
  check(mode + ": no page errors (live popover)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=live&scheme=light&reduce=1", { reduced: true });
  const info = await page.evaluate(() => {
    const c = document.querySelector(".pv-livecell[data-name=short]");
    return { cls: c.querySelector(".lv").classList.contains("reduce-motion"), anim: getComputedStyle(c.querySelector(".lv-new")).animationName };
  });
  check("live popover: reduced motion turns the word fade off", info.cls && info.anim === "none", info);
  await ctx.close();
}

// ---------- main: History ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light&xss=1");
  const first = page.locator(".hrow-text").first();
  check("transcript markup is shown as text", (await first.textContent()).startsWith("<img src=x onerror") && (await page.locator(".hrow-text img, .hrow-text b").count()) === 0);
  check("injected handler did not run", (await page.evaluate(() => window.__pwned)) === undefined);

  const labels = await page.locator(".hrow .copy").evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
  check("every copy button has its own name", new Set(labels).size === labels.length && labels.every((l) => l.startsWith("Copy dictation")), labels);
  const copy = page.locator(".hrow .copy").nth(1);
  const before = await copy.getAttribute("aria-label");
  await copy.click();
  await page.waitForTimeout(250);
  check("copy shows a Copied state", (await copy.getAttribute("aria-label")) === "Copied");
  check("copy is announced through the status region", (await page.locator(".sr-only[role=status]").textContent()) === "Copied to the clipboard.");
  await page.waitForTimeout(1100);
  check("copy restores its own name after 1 s", (await copy.getAttribute("aria-label")) === before, before);

  await page.fill(".search-input", "zzzz");
  check("no-match message has no quotes", (await page.locator(".empty-body").textContent()) === "Nothing matches your search.");
  const clearBox = await page.locator(".search-clear").boundingBox();
  check("search clear button is 28 pt", clearBox.width >= 28 && clearBox.height >= 28, clearBox);
  await page.fill(".search-input", "");

  const more = page.locator(".linklike:visible").first();
  const moreBox = await more.boundingBox();
  check("Show more is a 28 pt target", moreBox.height >= 28, moreBox);
  const dotBefore = await page.locator(".hrow-meta .dot-sep:not([hidden])").evaluateAll((els) => els.length);
  check("Show more has a separator dot", dotBefore >= 1);

  await page.getByText("Clear History", { exact: true }).first().click();
  await page.waitForTimeout(250);
  check("clear history asks first", (await page.locator("dialog.dlg[open]").count()) === 1);
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  check("cancel keeps history", (await page.locator(".hrow").count()) > 0);
  await page.getByText("Clear History", { exact: true }).first().click();
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(300);
  check("confirm clears history", (await page.locator(".empty-title").textContent()) === "No dictations yet");
  const hint = await page.locator(".empty-body .key").allTextContents();
  check("macOS empty-state keycaps use Mac names for both shortcuts", hint.join("+") === "fn+Shift+fn+Shift+Space", hint);
  const hintText = await page.locator(".empty-body").textContent();
  check("empty-state hint names push to talk and hands-free", hintText === "Hold fnShift and speak, or press fnShiftSpace for hands-free. Your text appears here.", hintText);

  // tabs by keyboard
  const tabs = page.locator("#tab-history");
  await tabs.focus();
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(250);
  check("arrow moves to Model", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("End");
  await page.waitForTimeout(250);
  check("End moves to the last tab", (await page.locator("#tab-settings").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Home");
  await page.waitForTimeout(250);
  check("Home moves to the first tab", (await page.locator("#tab-history").getAttribute("aria-selected")) === "true");
  const seg = await page.locator(".toolbar-seg .seg-item").first().boundingBox();
  check("tab buttons are 28 pt tall", seg.height >= 28, seg);

  await page.keyboard.press("Meta+2");
  await page.waitForTimeout(250);
  check("Cmd+2 opens Model", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Meta+3");
  await page.waitForTimeout(250);
  check("Cmd+3 opens Settings", (await page.locator("#tab-settings").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Meta+f");
  await page.waitForTimeout(250);
  check("Cmd+F opens History and focuses the search box", (await page.locator("#tab-history").getAttribute("aria-selected")) === "true" && (await page.evaluate(() => document.activeElement.className)) === "search-input");
  check("no page errors (history)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: History select and delete ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light");
  const rows = () => page.locator(".hrow").count();
  const count = () => page.locator(".select-count").textContent();
  const total = await rows();
  check("select bar and checkboxes stay hidden until Select", (await page.locator(".select-bar").isHidden()) && (await page.locator(".hrow-check").count()) === 0);
  await page.locator(".history-bar > .btn", { hasText: "Select" }).click();
  check("Select shows a checkbox per row and hides Clear History", (await page.locator(".hrow-check").count()) === total && (await page.getByText("Clear History", { exact: true }).isHidden()));
  check("Select turns into Done and copy buttons go away", (await page.locator(".history-bar > .btn").first().textContent()) === "Done" && (await page.locator(".hrow .copy").count()) === 0);
  check("Delete is off while nothing is picked", (await count()) === "None selected" && (await page.locator(".select-bar .btn-danger").isDisabled()));

  await page.locator(".hrow-text").first().click();
  check("clicking a row picks it", (await count()) === "1 selected" && (await page.locator(".hrow").first().getAttribute("class")).includes("is-selected"));
  await page.locator(".hrow-check").nth(2).click();
  check("a checkbox adds to the pick and shows its tick", (await count()) === "2 selected" && (await page.locator(".hrow-check").nth(2).isChecked()));
  await page.locator(".hrow-check").nth(5).click({ modifiers: ["Shift"] });
  const ticks = await page.locator(".hrow-check").evaluateAll((els) => els.map((e) => e.checked));
  check("Shift-click picks the run between, ticks included", (await count()) === "5 selected" && ticks.slice(0, 6).join() === "true,false,true,true,true,true", ticks);
  check("the day box shows a mixed state", await page.locator(".group-check").first().evaluate((b) => b.indeterminate || b.checked));

  await page.locator("#select-all").click();
  check("Select All picks every row shown", (await count()) === total + " selected");
  await page.locator("#select-all").click();
  check("Select All again clears the pick", (await count()) === "None selected" && (await page.locator(".select-bar .btn-danger").isDisabled()));

  await page.locator(".hrow-check").first().focus();
  await page.keyboard.press("Meta+a");
  check("Cmd+A picks every row shown", (await count()) === total + " selected");
  await page.fill(".search-input", "domain");
  await page.waitForTimeout(100);
  check("search drops picks it hides, so Delete never removes unseen rows", (await count()) === "1 selected");
  await page.fill(".search-input", "");
  await page.waitForTimeout(100);

  await page.locator(".hrow-check").first().focus();
  await page.keyboard.press("Delete");
  await page.waitForTimeout(250);
  check("Delete key asks first, naming one dictation", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Delete this dictation?");
  check("delete dialog says it is permanent", (await page.locator("dialog.dlg[open] .dlg-body").textContent()) === "It will be removed from this Mac. This can't be undone.");
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  check("cancel keeps every row", (await rows()) === total);

  await page.locator(".hrow-check").first().focus();
  await page.keyboard.press("Escape");
  check("Esc leaves select mode", (await page.locator(".select-bar").isHidden()) && (await page.locator(".hrow-check").count()) === 0);

  await page.locator(".history-bar > .btn", { hasText: "Select" }).click();
  const dayRows = await page.locator(".card").first().locator(".hrow").count();
  await page.locator(".group-check").first().click();
  check("the day box picks that whole day", (await count()) === dayRows + " selected");
  await page.locator(".select-bar .btn-danger").click();
  await page.waitForTimeout(250);
  const title = dayRows === 1 ? "Delete this dictation?" : "Delete " + dayRows + " dictations?";
  check("delete dialog counts the pick", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === title);
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(400);
  check("confirm deletes exactly the picked rows", (await rows()) === total - dayRows);
  check("the app is asked to delete them by id", (await page.evaluate(() => window.__calls)).includes("delete_history:" + dayRows));
  check("select mode ends after a delete", await page.locator(".select-bar").isHidden());
  const said = await page.locator(".sr-only[role=status]").textContent();
  check("the delete is announced", said === "Deleted " + (dayRows === 1 ? "1 dictation" : dayRows + " dictations") + ".", said);
  check("no page errors (history select)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- README images carry no metadata ----------
{
  // Lists a PNG's chunk types; anything beyond what is needed to draw (text, time, profile, EXIF) is metadata.
  const chunks = (buf) => {
    const types = [];
    for (let at = 8; at < buf.length; at += 12 + buf.readUInt32BE(at)) types.push(buf.toString("latin1", at + 4, at + 8));
    return types;
  };
  const dir = path.join(ROOT, "docs/screenshots");
  const extra = {};
  for (const name of fs.readdirSync(dir).filter((n) => n.endsWith(".png"))) {
    const odd = chunks(fs.readFileSync(path.join(dir, name))).filter((t) => !["IHDR", "PLTE", "IDAT", "IEND"].includes(t));
    if (odd.length) extra[name] = [...new Set(odd)];
  }
  check("README screenshots hold only image data, no metadata chunks", Object.keys(extra).length === 0, extra);
}

// ---------- main: Model ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=model&scheme=light");
  const row = (name) => page.locator(".mrow", { hasText: name });
  check("model rows are not radios and have no tab stops", (await page.locator(".mrow[role=radio], .card[role=radiogroup], .mrow[tabindex]").count()) === 0);
  check("Active shown for the loaded model", (await row("Small").locator(".pill-active").count()) === 1 && (await row("Medium").locator(".pill-active").count()) === 0);
  check("unsupported row has no button and keeps its reasons", (await row("Large v3").locator("button").count()) === 0 && (await row("Large v3").locator(".reasons li").count()) === 2);
  const act = row("Medium").locator("button");
  const box = await act.boundingBox();
  check("Download is a real 28 pt button with the size", (await act.textContent()) === "Download 514 MB" && box.height >= 28, box);

  await act.click();
  await page.waitForTimeout(250);
  const body = await page.locator("dialog.dlg[open]").textContent();
  check("download asks first and names the size", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Download Medium?" && body.includes("514 MB"), body);
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  await page.waitForTimeout(150);
  check("cancel does not start a download", !(await calls(page)).some((c) => c.startsWith("choose_model")), await calls(page));

  await act.click();
  await page.locator("dialog.dlg .btn-action").click();
  await page.waitForTimeout(900);
  check("confirmed download shows progress", (await row("Medium").locator("[role=progressbar]").count()) === 1);
  await page.waitForTimeout(5500);
  check("Medium becomes Active and Small offers Use", (await row("Medium").locator(".pill-active").count()) === 1 && (await row("Small").locator("button").textContent()) === "Use");
  await row("Small").locator("button").click();
  await page.waitForTimeout(200);
  check("Use shows Loading, no confirmation for a downloaded model", (await row("Small").locator(".pill-busy").textContent()) === "Loading" && (await page.locator("dialog.dlg[open]").count()) === 0);
  await page.waitForTimeout(800);
  check("Small is Active again after the load event", (await row("Small").locator(".pill-active").count()) === 1);
  check("no page errors (model)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&active=none");
  const small = page.locator(".mrow", { hasText: "Small" });
  check("Active follows the loaded model, not config.model", (await small.locator(".pill-active").count()) === 0 && (await small.locator("button").textContent()) === "Use");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&downloading=medium");
  const med = page.locator(".mrow", { hasText: "Medium" });
  check("a download already running shows at once", (await med.locator(".pill-busy").textContent()) === "Downloading" && (await med.locator("[role=progressbar]").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&fail=medium");
  const med = page.locator(".mrow", { hasText: "Medium" });
  await med.locator("button").click();
  await page.locator("dialog.dlg .btn-action").click();
  await page.waitForTimeout(3200);
  check("failed download shows the error, offers Download again, keeps Small active",
    (await med.locator(".model-error").count()) === 1 && (await med.locator("button").textContent()) === "Download 514 MB" && (await page.locator(".mrow", { hasText: "Small" }).locator(".pill-active").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&platform=windows");
  check("hardware row has no jargon on Windows", (await page.locator(".hw-item", { hasText: "System" }).locator("dd").textContent()) === "Windows 11, 64-bit");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light");
  check("hardware row says Apple silicon on macOS", (await page.locator(".hw-item", { hasText: "System" }).locator("dd").textContent()) === "macOS 15.1, Apple silicon");
  check("hardware shows - for unknown free disk", (await page.locator(".hw-item", { hasText: "Free disk" }).locator("dd").textContent()) === "-");
  await ctx.close();
}

// ---------- main: Settings ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const move = page.getByRole("button", { name: "Move Pill" });
  check("Pill position row has Move pill and Reset position", (await page.locator(".srow-title", { hasText: "Pill position" }).count()) === 1 && (await move.count()) === 1 && (await page.getByRole("button", { name: "Reset Position" }).count()) === 1);
  await move.click();
  await page.waitForTimeout(200);
  check("Move pill calls move_overlay on:true and becomes Done", (await calls(page)).includes("move_overlay:true") && (await page.getByRole("button", { name: "Done", exact: true }).count()) === 1);
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.waitForTimeout(200);
  check("Done calls move_overlay on:false", (await calls(page)).join() === "move_overlay:true,move_overlay:false", await calls(page));
  await page.getByRole("button", { name: "Move Pill" }).click();
  await page.locator("#tab-history").click();
  await page.waitForTimeout(200);
  check("leaving the Settings tab stops positioning", (await calls(page)).slice(-1)[0] === "move_overlay:false", await calls(page));
  await page.locator("#tab-settings").click();
  check("button is back to Move pill after returning", (await page.getByRole("button", { name: "Move Pill" }).count()) === 1);
  await page.getByRole("button", { name: "Reset Position" }).click();
  await page.waitForTimeout(250);
  check("Reset position resets and says so", (await calls(page)).includes("reset_overlay_position") && (await page.locator(".toast").textContent()) === "Position reset.");

  // Reset while the pill is up re-places it: hide, then show again
  await page.getByRole("button", { name: "Move Pill" }).click();
  await page.getByRole("button", { name: "Reset Position" }).click();
  await page.waitForTimeout(600);
  check("Reset while positioning hides and shows the pill again", (await calls(page)).slice(-4).join() === "move_overlay:true,reset_overlay_position,move_overlay:false,move_overlay:true", await calls(page));
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.waitForTimeout(150);

  // history off asks first
  const history = page.getByRole("switch", { name: "Save history" });
  await history.click();
  await page.waitForTimeout(250);
  check("turning history off asks first", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Turn off history?" && (await page.locator("dialog.dlg[open] .dlg-body").textContent()) === "Saved dictations will be deleted from this Mac.");
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  await page.waitForTimeout(150);
  check("cancel leaves history on", (await history.getAttribute("aria-checked")) === "true");
  await history.click();
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(300);
  check("confirm turns history off", (await history.getAttribute("aria-checked")) === "false");
  await page.locator("#tab-history").click();
  await page.waitForTimeout(200);
  check("history list is emptied and says History is off", (await page.locator(".empty-title").textContent()) === "History is off");
  await page.locator("#tab-settings").click();
  await history.click(); // turning it back on needs no dialog
  await page.waitForTimeout(200);
  check("turning history on needs no confirmation", (await page.locator("dialog.dlg[open]").count()) === 0 && (await history.getAttribute("aria-checked")) === "true");

  check("start at login copy has no shell command", (await page.locator(".srow-desc", { hasText: "uv tool" }).count()) === 0);
  const hk = page.locator(".hk[data-slot=hold]");
  check("Mac keycaps in settings", (await hk.locator(".key").allTextContents()).join("+") === "fn+Shift");
  await hk.click();
  await page.waitForTimeout(150);
  check("capture start is announced", (await page.locator(".sr-only[role=status]").textContent()) === "Press your shortcut now. Escape cancels.");
  await page.waitForTimeout(2400);
  check("captured combo uses Mac names", (await hk.locator(".key").allTextContents()).join("+") === "Control+Option+Space", await hk.locator(".key").allTextContents());
  await page.getByRole("button", { name: "Reset Push to talk shortcut" }).click();
  await page.waitForTimeout(250);
  check("reset restores fn + Shift", (await hk.locator(".key").allTextContents()).join("+") === "fn+Shift");

  // theme group Home/End
  await page.getByRole("radio", { name: /Pill/ }).focus();
  await page.keyboard.press("End");
  await page.waitForTimeout(200);
  check("End selects the last theme", (await page.getByRole("radio", { name: /Minimal/ }).getAttribute("aria-checked")) === "true");
  await page.keyboard.press("Home");
  await page.waitForTimeout(200);
  check("Home selects the first theme", (await page.getByRole("radio", { name: /Pill/ }).getAttribute("aria-checked")) === "true");

  const sw = await page.getByRole("switch", { name: "Sounds" }).evaluate((el) => {
    const cs = getComputedStyle(el, "::before");
    return { w: parseFloat(cs.width), h: parseFloat(cs.height) };
  });
  check("switch press target is at least 44 x 28", sw.w >= 44 && sw.h >= 28, sw);

  const ring = await page.evaluate(() => {
    document.querySelector("#tab-history").focus();
    const e = document.querySelector(".mw .btn") || document.body;
    e.focus();
    return null;
  });
  void ring;
  check("no page errors (settings)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows&h=1200", { tall: true });
  const hk = page.locator(".hk[data-slot=hold]");
  check("Windows keycaps keep Ctrl and Alt", (await hk.locator(".key").allTextContents()).join("+") === "Ctrl+Alt");
  check("Windows hands-free keycaps keep Ctrl, Alt and Space", (await page.locator(".hk[data-slot=toggle] .key").allTextContents()).join("+") === "Ctrl+Alt+Space");
  await page.keyboard.press("Control+2");
  await page.waitForTimeout(250);
  check("Ctrl+2 opens Model on Windows", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true");
  await page.locator("#tab-settings").click();
  await page.getByRole("switch", { name: "Save history" }).click();
  await page.waitForTimeout(250);
  check("Windows wording says this PC", (await page.locator("dialog.dlg[open] .dlg-body").textContent()) === "Saved dictations will be deleted from this PC.");
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  await ctx.close();
}
for (const how of ["visibilitychange", "pagehide"]) {
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  await page.getByRole("button", { name: "Move Pill" }).click();
  await page.waitForTimeout(150);
  await page.evaluate((kind) => {
    if (kind === "visibilitychange") {
      Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
      document.dispatchEvent(new Event("visibilitychange"));
    } else {
      window.dispatchEvent(new Event("pagehide"));
    }
  }, how);
  await page.waitForTimeout(150);
  check("closing the window (" + how + ") stops positioning", (await calls(page)).join() === "move_overlay:true,move_overlay:false", await calls(page));
  await ctx.close();
}

// ---------- main: Settings, shortcuts ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const push = page.locator(".hk[data-slot=hold]");
  const free = page.locator(".hk[data-slot=toggle]");
  const pushWrap = page.locator(".hk-wrap").nth(0);
  const freeWrap = page.locator(".hk-wrap").nth(1);
  const cfg = () => page.evaluate(() => window.__cfg.slice());
  const keys = async (loc) => (await loc.locator(".key").allTextContents()).join("+");
  const resetBtn = (name) => page.getByRole("button", { name: "Reset " + name + " shortcut" });
  const turnOff = page.getByRole("button", { name: "Turn off hands-free" });
  const isCapturing = (loc) => loc.evaluate((e) => e.classList.contains("capturing"));

  const titles = await page.locator(".srow-title").allTextContents();
  check("Dictation has Push to talk and Hands-free rows, no Shortcut or Mode", titles.includes("Push to talk") && titles.includes("Hands-free") && !titles.includes("Shortcut") && !titles.includes("Mode"), titles);
  const rowDesc = (title) => page.locator(".srow", { has: page.locator(".srow-title", { hasText: title }) }).locator(".srow-desc").first().textContent();
  check("Push to talk copy", (await rowDesc("Push to talk")) === "Hold to talk, release to paste.");
  check("Hands-free copy", (await rowDesc("Hands-free")) === "Press once to start listening, again to stop. Adding the extra key while holding push to talk switches to hands-free.");
  check("no dictation mode control is left", (await page.locator("[aria-label='Dictation mode']").count()) === 0);
  check("both shortcut rows render keycaps", (await keys(push)) === "fn+Shift" && (await keys(free)) === "fn+Shift+Space");
  check("each shortcut field has its own name", (await push.getAttribute("aria-label")).startsWith("Push to talk shortcut: fn plus Shift") && (await free.getAttribute("aria-label")).startsWith("Hands-free shortcut: fn plus Shift plus Space") && (await page.getByRole("button", { name: "Hands-free shortcut" }).count()) === 1 && (await page.getByRole("button", { name: "Push to talk shortcut" }).count()) === 1);
  check("hands-free Reset is hidden at the default and Turn off is shown", (await resetBtn("Hands-free").isHidden()) && (await turnOff.isVisible()));
  const turnBox = await turnOff.boundingBox();
  const freeBox = await free.boundingBox();
  check("shortcut field and Turn off keep 28 pt targets", turnBox.height >= 28 && freeBox.height >= 28, { turnBox, freeBox });

  // one capture at a time, and a slot never marks the other
  await free.click();
  await page.waitForTimeout(150);
  check("capturing hands-free does not mark push to talk", (await isCapturing(free)) && !(await isCapturing(push)) && (await push.getAttribute("aria-label")).startsWith("Push to talk shortcut: fn plus Shift"));
  check("capture asks the app for the toggle slot", (await cfg()).slice(-1)[0] === "start_hotkey_capture:toggle", await cfg());
  check("Turn off hides while hands-free captures", await turnOff.isHidden());
  await push.click();
  await page.waitForTimeout(150);
  check("starting a second capture ends the first", !(await isCapturing(free)) && (await isCapturing(push)) && (await cfg()).slice(-2).join() === "cancel_hotkey_capture,start_hotkey_capture:hold", await cfg());
  await page.keyboard.press("Escape");
  await page.waitForTimeout(150);
  check("Escape leaves capture mode and shows no error", !(await isCapturing(push)) && (await page.locator(".field-error:visible").count()) === 0 && (await cfg()).slice(-1)[0] === "cancel_hotkey_capture", await cfg());

  // a captured toggle combo is saved by the app: the UI re-reads state and never sends it back
  await page.evaluate(() => { window.__cfg.length = 0; });
  await free.click();
  await page.waitForTimeout(2500);
  const afterCapture = await cfg();
  check("captured hands-free combo triggers get_state and not set_config", afterCapture.includes("get_state") && !afterCapture.some((c) => c.startsWith("set_config")), afterCapture);
  check("captured hands-free combo shows in its field only", (await keys(free)) === "Control+Option+H" && (await keys(push)) === "fn+Shift");
  check("capture ends after the event", !(await isCapturing(free)));
  check("Reset appears once hands-free differs from its default", await resetBtn("Hands-free").isVisible());

  await page.evaluate(() => { window.__cfg.length = 0; });
  await resetBtn("Hands-free").click();
  await page.waitForTimeout(300);
  check("hands-free Reset patches the platform default toggle combo", (await cfg()).includes('set_config:{"toggle_hotkey":{"modifiers":["Fn","Shift"],"key":"Space"}}') && (await keys(free)) === "fn+Shift+Space", await cfg());

  // errors land under the slot that asked; Cancelled is silent
  await page.evaluate(() => { window.__captureError = "That shortcut is already used for the other mode."; });
  await free.click();
  await page.waitForTimeout(2500);
  check("a capture error shows under the hands-free field only", (await freeWrap.locator(".field-error").isVisible()) && (await freeWrap.locator(".field-error").textContent()) === "That shortcut is already used for the other mode." && (await pushWrap.locator(".field-error").isHidden()) && !(await isCapturing(free)));
  await page.evaluate(() => { window.__captureError = "Cancelled"; });
  await free.click();
  await page.waitForTimeout(2500);
  check("Cancelled from the app clears the error and shows nothing", (await page.locator(".field-error:visible").count()) === 0 && !(await isCapturing(free)));
  await page.evaluate(() => { window.__captureError = ""; });

  // Turn off, then turn back on by clicking the field
  await page.evaluate(() => { window.__cfg.length = 0; });
  await turnOff.click();
  await page.waitForTimeout(300);
  check("Turn off sends toggle_hotkey null", (await cfg()).includes('set_config:{"toggle_hotkey":null}'), await cfg());
  check("when off the field shows Off and Turn off is gone", (await free.textContent()) === "Off" && (await free.locator(".key").count()) === 0 && (await turnOff.isHidden()) && (await free.getAttribute("aria-label")) === "Hands-free shortcut: off. Click to turn on.");
  check("Reset is offered while hands-free is off", await resetBtn("Hands-free").isVisible());
  await free.click();
  await page.waitForTimeout(2500);
  check("clicking Off captures a new hands-free shortcut", (await keys(free)) === "Control+Option+H" && (await turnOff.isVisible()));
  check("no page errors (shortcuts)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&empty=1&toggle=off");
  const text = await page.locator(".empty-body").textContent();
  check("empty hint with hands-free off names only push to talk", text === "Hold fnShift and speak. Your text appears here." && (await page.locator(".empty-body .key").count()) === 2, text);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&empty=1&platform=windows");
  const hint = await page.locator(".empty-body .key").allTextContents();
  check("Windows empty hint shows both shortcuts", hint.join("+") === "Ctrl+Alt+Ctrl+Alt+Space", hint);
  await ctx.close();
}

// ---------- main: Settings, install and notices ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Install" }) });
  const button = row.locator(".btn");
  check("Install row copy on macOS", (await row.locator(".srow-desc").textContent()) === "Add local-stt to Applications so Spotlight finds it, and add the local-stt terminal command.");
  check("Install is an enabled 28 pt button", (await button.textContent()) === "Install" && (await button.isEnabled()) && (await button.boundingBox()).height >= 28);
  const loginDesc = await page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Start at login" }) }).locator(".srow-desc").textContent();
  check("Start at login copy says it installs first", loginDesc === "Opens local-stt when you log in. Turning it on installs local-stt first if needed.", loginDesc);
  await button.click();
  await page.waitForTimeout(400);
  check("Install calls install_app", (await calls(page)).includes("install_app"), await calls(page));
  check("Install toasts the returned sentence", (await page.locator(".toast").textContent()) === "Installed local-stt in Applications and set up the local-stt command.");
  check("after Install the row shows a disabled Installed button with a check", (await button.textContent()) === "Installed" && (await button.isDisabled()) && (await button.locator("svg.ic-check").count()) === 1);
  check("Install refreshed state through get_state", (await page.evaluate(() => window.__cfg.filter((c) => c === "get_state").length)) >= 2);
  check("installed copy on macOS", (await row.locator(".srow-desc").textContent()) === "local-stt is in your Applications folder (Spotlight finds it) and the local-stt terminal command is set up.");
  check("no page errors (install)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&installed=1&h=1200", { tall: true });
  const button = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Install" }) }).locator(".btn");
  check("installed state shows a disabled Installed button with a check", (await button.textContent()) === "Installed" && (await button.isDisabled()) && (await button.locator("svg.ic-check").count()) === 1);
  check("installed state never calls install_app", !(await calls(page)).includes("install_app"));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Install" }) });
  check("Install row copy on Windows", (await row.locator(".srow-desc").textContent()) === "Add local-stt to the Start menu so Windows search finds it, and add the local-stt terminal command.");
  await row.locator(".btn").click();
  await page.waitForTimeout(400);
  check("installed copy on Windows", (await row.locator(".srow-desc").textContent()) === "local-stt is in your Start menu (Windows search finds it) and the local-stt terminal command is set up.");
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  await page.evaluate(() => { window.__emit("notice", {}); window.__emit("notice", null); window.__emit("notice", { message: "" }); });
  await page.waitForTimeout(300);
  check("a notice without a message shows nothing", (await page.locator(".toast").count()) === 0);
  await page.evaluate(() => window.__emit("notice", { message: "Installed local-stt and added the local-stt command." }));
  await page.waitForTimeout(300);
  check("notice event toasts its message", (await page.locator(".toast").textContent()) === "Installed local-stt and added the local-stt command.");
  check("toast lives in a polite status region", (await page.locator(".toast-host").getAttribute("role")) === "status" && (await page.locator(".toast-host").getAttribute("aria-live")) === "polite");
  await page.waitForTimeout(5800);
  await page.getByRole("switch", { name: "Start at login" }).click();
  await page.waitForTimeout(400);
  check("turning Start at login on toasts the app's notice and flips Install to Installed", (await page.locator(".toast").textContent()) === "Installed local-stt and set it to open at login." && (await page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Install" }) }).locator(".btn").textContent()) === "Installed");
  check("no page errors (notices)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Settings, smart formatting and live transcription ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const cfg = () => page.evaluate(() => window.__cfg.slice());
  const titles = await page.locator(".srow-title").allTextContents();
  const descOf = (title) => page.locator(".srow", { has: page.locator(".srow-title", { hasText: title }) }).locator(".srow-desc").first().textContent();
  const smart = page.getByRole("switch", { name: "Smart formatting" });
  check("Smart formatting sits right after Paste into the focused app", titles.indexOf("Smart formatting") === titles.indexOf("Paste into the focused app") + 1, titles);
  check("Smart formatting copy", (await descOf("Smart formatting")) === 'Turns spoken "point one, point two" into a numbered list, removes "uh" and "um", and fixes spacing.');
  check("Smart formatting starts on", (await smart.getAttribute("aria-checked")) === "true");
  await smart.click();
  await page.waitForTimeout(250);
  check("Smart formatting toggle sends smart_format false", (await cfg()).includes('set_config:{"smart_format":false}') && (await smart.getAttribute("aria-checked")) === "false", await cfg());

  const live = page.getByRole("switch", { name: "Live transcription" });
  const overlay = page.getByRole("switch", { name: "Show overlay" });
  check("Live transcription sits right after Show overlay", titles.indexOf("Live transcription") === titles.indexOf("Show overlay") + 1, titles);
  check("Live transcription copy", (await descOf("Live transcription")) === "Shows your words above the pill while you speak.");
  check("Live transcription starts on and enabled", (await live.getAttribute("aria-checked")) === "true" && (await live.isEnabled()));
  await live.click();
  await page.waitForTimeout(250);
  check("Live transcription toggle sends live_transcription false", (await cfg()).includes('set_config:{"live_transcription":false}') && (await live.getAttribute("aria-checked")) === "false", await cfg());
  await live.click();
  await page.waitForTimeout(250);
  await overlay.click();
  await page.waitForTimeout(250);
  check("with the overlay off, Live transcription is disabled and says it needs the overlay", (await live.isDisabled()) && (await descOf("Live transcription")) === "Needs the overlay. Turn on Show overlay to use it.");
  await overlay.click();
  await page.waitForTimeout(250);
  check("turning the overlay back on re-enables Live transcription", (await live.isEnabled()) && (await descOf("Live transcription")) === "Shows your words above the pill while you speak.");
  check("no page errors (smart formatting and live transcription)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- look: tokens, contrast, modes ----------
function contrastScript() {
  return () => {
    const parse = (c) => {
      const m = c.match(/rgba?\(([^)]+)\)/);
      const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
      return { r: p[0], g: p[1], b: p[2], a: p[3] === undefined ? 1 : p[3] };
    };
    const over = (fg, bg) => ({ r: fg.r * fg.a + bg.r * (1 - fg.a), g: fg.g * fg.a + bg.g * (1 - fg.a), b: fg.b * fg.a + bg.b * (1 - fg.a), a: 1 });
    const lin = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    const lum = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    const ratio = (a, b) => { const l = [lum(a), lum(b)].sort((x, y) => y - x); return (l[0] + 0.05) / (l[1] + 0.05); };
    const surface = parse(getComputedStyle(document.querySelector(".card")).backgroundColor);
    const out = {};
    const sel = { "tag-blue": ".tag-blue", "tag-green": ".tag-green", "tag-red": ".tag-red", "tag-grey": ".tag-grey", "pill-active": ".pill-active", "btn-action": ".btn-action", "btn-danger": ".btn-danger" };
    for (const [name, q] of Object.entries(sel)) {
      const el = document.querySelector(q);
      if (!el) { out[name] = null; continue; }
      const cs = getComputedStyle(el);
      let bg = parse(cs.backgroundColor);
      bg = over(bg, surface);
      out[name] = +ratio(over(parse(cs.color), bg), bg).toFixed(2);
    }
    return out;
  };
}
for (const dark of [false, true]) {
  const { page, ctx } = await open("?view=main&tab=model&scheme=" + (dark ? "dark" : "light"), { dark });
  const r = await page.evaluate(contrastScript());
  const vals = Object.entries(r).filter(([, v]) => v !== null);
  check((dark ? "dark" : "light") + ": model tags, pills and buttons pass 4.5:1", vals.length >= 5 && vals.every(([, v]) => v >= 4.5), r);
  await ctx.close();
}
for (const dark of [false, true]) {
  const { page, ctx } = await open("?view=main&tab=history&scheme=" + (dark ? "dark" : "light") + "&xss=0", { dark });
  const r = await page.evaluate(contrastScript());
  check((dark ? "dark" : "light") + ": history Failed tag and Clear history pass 4.5:1", r["tag-red"] >= 4.5 && r["btn-danger"] >= 4.5, r);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light");
  await page.locator("#tab-model").focus();
  await page.keyboard.press("Tab");
  const info = await page.evaluate(() => {
    const cs = getComputedStyle(document.activeElement);
    const toolbar = getComputedStyle(document.querySelector(".mw-toolbar"));
    const body = getComputedStyle(document.querySelector(".mrow-name"));
    return { outline: cs.outlineStyle + " " + cs.outlineWidth + " " + cs.outlineColor, backdrop: toolbar.backdropFilter, tracking: body.letterSpacing };
  });
  check("focus ring is a 2 px opaque blue", info.outline.startsWith("solid 2px rgb(0, 122, 255)"), info.outline);
  check("toolbar has no fake blur", info.backdrop === "none", info.backdrop);
  check("letter-spacing is the system default", info.tracking === "normal", info.tracking);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true, media: [{ name: "prefers-contrast", value: "more" }] });
  const info = await page.evaluate(() => ({
    label2: getComputedStyle(document.querySelector(".group-title")).color,
    card: getComputedStyle(document.querySelector(".card")).boxShadow,
    hairline: +getComputedStyle(document.querySelector(".mw-toolbar"), "::after").opacity,
  }));
  check("Increase Contrast: opaque secondary text, 1 px card border, toolbar hairline", info.label2 === "rgb(60, 60, 67)" && info.card.includes("1px") && info.hairline === 1, info);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=dark&h=1200", { tall: true, dark: true, media: [{ name: "prefers-contrast", value: "more" }] });
  const info = await page.evaluate(() => ({ label2: getComputedStyle(document.querySelector(".group-title")).color, card: getComputedStyle(document.querySelector(".card")).boxShadow }));
  check("Increase Contrast, dark: opaque light secondary text and 1 px card border", info.label2 === "rgb(235, 235, 245)" && info.card.includes("1px"), info);
  await ctx.close();
}

// ---------- button colours ----------
const inkScript = ([sel, token, surfaceSel]) => {
  const parse = (c) => { const m = c.match(/rgba?\(([^)]+)\)/)[1].split(/[ ,/]+/).filter(Boolean).map(Number); return { r: m[0], g: m[1], b: m[2], a: m[3] === undefined ? 1 : m[3] }; };
  const over = (fg, bg) => ({ r: fg.r * fg.a + bg.r * (1 - fg.a), g: fg.g * fg.a + bg.g * (1 - fg.a), b: fg.b * fg.a + bg.b * (1 - fg.a), a: 1 });
  const lin = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
  const lum = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
  const ratio = (a, b) => { const l = [lum(a), lum(b)].sort((x, y) => y - x); return +((l[0] + 0.05) / (l[1] + 0.05)).toFixed(2); };
  const el = document.querySelector(sel);
  if (!el) return null;
  const surface = parse(getComputedStyle(document.querySelector(surfaceSel)).backgroundColor);
  const cs = getComputedStyle(el);
  const bg = over(parse(cs.backgroundColor), surface);
  const probe = document.createElement("span");
  probe.style.color = token === "white" ? "#fff" : "var(--" + token + ")";
  el.appendChild(probe);
  const expected = getComputedStyle(probe).color;
  probe.remove();
  return { color: cs.color, expected, ratio: ratio(over(parse(cs.color), bg), bg) };
};
for (const dark of [false, true]) {
  const mode = dark ? "dark" : "light";
  const scheme = "&scheme=" + mode;
  const probe = async (query, sel, token, surface, opts = {}) => {
    const { page, ctx } = await open(query + scheme, { dark, ...opts });
    if (opts.before) await opts.before(page);
    const r = await page.evaluate(inkScript, [sel, token, surface]);
    await ctx.close();
    return r;
  };
  const verdict = (r) => r !== null && r.color === r.expected && r.ratio >= 4.5;
  const settings = "?view=main&tab=settings&installed=1&toggle=custom&h=1200";
  const quiet = await probe(settings, ".btn-quiet", "accent-ink", ".card", { tall: true });
  check(mode + ": Reset gets the accent ink at 4.5:1", verdict(quiet), quiet);
  const turnOff = await probe(settings, "[aria-label='Turn off hands-free']", "accent-ink", ".card", { tall: true });
  check(mode + ": Turn off gets the accent ink at 4.5:1", verdict(turnOff), turnOff);
  const done = await probe(settings, ".btn-done", "green-ink", ".card", { tall: true });
  check(mode + ": Installed gets the green ink at 4.5:1", verdict(done), done);
  const action = await probe("?view=main&tab=model", ".btn-action", "accent-ink", ".card");
  check(mode + ": Download gets the accent ink at 4.5:1", verdict(action), action);
  const danger = await probe("?view=main&tab=history", ".btn-danger", "red-ink", ".card");
  check(mode + ": Clear history gets the red ink at 4.5:1", verdict(danger), danger);
  const link = await probe("?view=main&tab=history", ".linklike:not([hidden])", "accent-ink", ".card");
  check(mode + ": Show more gets the accent ink at 4.5:1", verdict(link), link);
  const icon = await probe("?view=main&tab=history", ".hrow .copy", "label-2", ".card");
  check(mode + ": copy button keeps its secondary ink at 4.5:1", verdict(icon), icon);
  const solid = await probe("?view=main&tab=history", ".btn-danger-solid", "white", ".dlg", { before: async (page) => { await page.getByText("Clear History", { exact: true }).first().click(); await page.waitForTimeout(300); } });
  check(mode + ": the red confirm button has white text at 4.5:1", verdict(solid), solid);
}

// ---------- main: show-tab event ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light");
  const selected = () => page.evaluate(() => [...document.querySelectorAll("[role=tab]")].filter((t) => t.getAttribute("aria-selected") === "true").map((t) => t.id).join());
  const visiblePanels = () => page.evaluate(() => [...document.querySelectorAll("[role=tabpanel]")].filter((p) => !p.hidden).map((p) => p.id).join());
  await page.evaluate(() => window.__emit("show-tab", { tab: "model" }));
  check("show-tab model switches to the Model tab", (await selected()) === "tab-model" && (await visiblePanels()) === "panel-model");
  await page.evaluate(() => window.__emit("show-tab", { tab: "settings" }));
  check("show-tab settings switches to Settings", (await selected()) === "tab-settings" && (await visiblePanels()) === "panel-settings");
  await page.evaluate(() => { for (const p of [{ tab: "bogus" }, { tab: "" }, { tab: 3 }, { tab: "constructor" }, {}, null, "model"]) window.__emit("show-tab", p); });
  check("show-tab ignores unknown tabs and bad payloads", (await selected()) === "tab-settings" && (await visiblePanels()) === "panel-settings");
  await page.evaluate(() => window.__emit("show-tab", { tab: "history" }));
  check("show-tab history comes back", (await selected()) === "tab-history" && (await visiblePanels()) === "panel-history");
  check("no page errors (show-tab)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: update banner ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light&update=1");
  const banner = page.locator(".mw-banner");
  const btn = (name) => banner.getByRole("button", { name, exact: true });
  const focusedClass = () => page.evaluate(() => document.activeElement.className);
  check("update from get_state shows the banner with both versions", (await banner.isVisible()) && (await banner.locator(".banner-text").textContent()) === "local-stt 0.2.0 is available. You have 0.1.0.");
  check("banner is a labelled region", (await banner.getAttribute("role")) === "region" && (await banner.getAttribute("aria-label")) === "Software update");
  check("banner offers Install and Restart, Release Notes, Later, Skip This Version", (await banner.locator("button").allTextContents()).join("|") === "Install and Restart|Release Notes|Later|Skip This Version");
  const heights = await banner.locator("button").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().height));
  check("banner buttons are 28 pt targets", heights.every((h) => h >= 28), heights);
  const layout = await page.evaluate(() => ({
    toolbar: document.querySelector(".mw-toolbar").getBoundingClientRect().bottom,
    banner: document.querySelector(".mw-banner").getBoundingClientRect(),
    content: document.querySelector(".history-bar").getBoundingClientRect().top,
  }));
  check("banner sits under the tabs and above the content, not over either", layout.banner.top >= layout.toolbar && layout.content >= layout.banner.bottom, layout);
  check("Install and Restart is the primary action", (await btn("Install and Restart").evaluate((e) => e.classList.contains("btn-action"))) && (await btn("Later").evaluate((e) => e.classList.contains("btn-quiet"))));

  await btn("Release Notes").click();
  await page.waitForTimeout(150);
  check("Release Notes calls open_release_notes and keeps the banner", (await calls(page)).includes("open_release_notes") && (await banner.isVisible()));
  await btn("Later").click();
  await page.waitForTimeout(150);
  check("Later hides the banner and gives the content its room back", (await banner.isHidden()) && (await page.evaluate(() => !document.querySelector(".mw").classList.contains("has-banner"))));
  check("Later calls neither skip nor install", !(await calls(page)).some((c) => c === "skip_update" || c === "install_update"));
  check("focus moves to the tab when the banner closes", (await page.evaluate(() => document.activeElement.id)) === "tab-history");
  await page.evaluate(() => window.__emit("show-update", {}));
  await page.waitForTimeout(150);
  check("show-update brings the banner back and focuses Install and Restart", (await banner.isVisible()) && (await focusedClass()).includes("btn-install"), await focusedClass());
  await btn("Later").click();
  await page.evaluate(() => window.__emit("update-available", { current: "0.1.0", latest: "0.2.0", available: true, notesUrl: "https://example.invalid/n" }));
  await page.waitForTimeout(150);
  check("a fresh update-available shows the banner again after Later", await banner.isVisible());
  check("no page errors (banner)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&update=1");
  const banner = page.locator(".mw-banner");
  await banner.getByRole("button", { name: "Skip This Version", exact: true }).click();
  await page.waitForTimeout(300);
  check("Skip This Version calls skip_update and hides the banner", (await calls(page)).includes("skip_update") && (await banner.isHidden()));
  check("skipping re-reads state", (await page.evaluate(() => window.__cfg.filter((c) => c === "get_state").length)) >= 2);
  check("a skipped version stays hidden after state is re-read", await banner.isHidden());
  await page.evaluate(() => window.__emit("show-tab", { tab: "settings" }));
  await page.getByRole("switch", { name: "Sounds" }).click();
  await page.waitForTimeout(250);
  check("a skipped version stays hidden after other settings change", await banner.isHidden());
  await page.evaluate(() => window.__emit("update-available", { current: "0.1.0", latest: "0.2.0", available: true, notesUrl: "https://example.invalid/n" }));
  await page.waitForTimeout(150);
  check("a manual check that finds the skipped version shows the banner again", await banner.isVisible());
  await banner.getByRole("button", { name: "Skip This Version", exact: true }).click();
  await page.waitForTimeout(300);
  check("skipping again hides it again", await banner.isHidden());
  await page.evaluate(() => window.__emit("update-available", { current: "0.1.0", latest: "0.3.0", available: true, notesUrl: "https://example.invalid/n" }));
  await page.waitForTimeout(150);
  check("a newer version than the skipped one shows the banner", (await banner.isVisible()) && (await banner.locator(".banner-text").textContent()) === "local-stt 0.3.0 is available. You have 0.1.0.");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&update=1");
  const banner = page.locator(".mw-banner");
  await banner.getByRole("button", { name: "Install and Restart", exact: true }).click();
  await page.waitForTimeout(200);
  const busy = await banner.evaluate((b) => ({ install: b.querySelector(".btn-install").textContent, spinner: b.querySelectorAll(".btn-install .spin").length, disabled: [...b.querySelectorAll("button")].map((x) => x.disabled).join() }));
  check("Install shows Installing... with a spinner and disables every button", busy.install === "Installing..." && busy.spinner === 1 && busy.disabled === "true,true,true,true", busy);
  check("Install calls install_update once", (await calls(page)).filter((c) => c === "install_update").length === 1);
  const spin = await banner.locator(".spin").evaluate((e) => getComputedStyle(e).animationName);
  check("the spinner turns when motion is allowed", spin === "spin", spin);
  await page.waitForTimeout(800);
  check("Install ends with a toast and the banner steps aside", (await page.locator(".toast").textContent()) === "Updated to 0.2.0. local-stt will restart in a moment." && (await banner.isHidden()));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&update=1&reduce=1", { reduced: true });
  await page.locator(".btn-install").click();
  await page.waitForTimeout(150);
  const spin = await page.locator(".mw-banner .spin").evaluate((e) => getComputedStyle(e).animationName);
  check("reduced motion stops the spinner", spin === "none", spin);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&update=1");
  const banner = page.locator(".mw-banner");
  await page.evaluate(() => { window.__installError = "Couldn't download the update. Check your connection."; });
  await banner.locator(".btn-install").click();
  await page.waitForTimeout(900);
  const failed = await banner.evaluate((b) => ({ install: b.querySelector(".btn-install").textContent, error: b.querySelector(".banner-error") && b.querySelector(".banner-error").textContent, role: b.querySelector(".banner-error") && b.querySelector(".banner-error").getAttribute("role"), disabled: [...b.querySelectorAll("button")].map((x) => x.disabled).join(), focus: document.activeElement.className }));
  check("a failed install shows the error inline with Try Again and re-enables the buttons", failed.install === "Try Again" && failed.error === "Couldn't download the update. Check your connection." && failed.role === "alert" && failed.disabled === "false,false,false,false", failed);
  check("focus lands on Try Again", failed.focus.includes("btn-install"), failed.focus);
  await banner.locator(".btn-install").click();
  await page.waitForTimeout(900);
  check("Try Again installs and clears the error", (await calls(page)).filter((c) => c === "install_update").length === 2 && (await banner.isHidden()) && (await page.locator(".toast").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light");
  const banner = page.locator(".mw-banner");
  check("no update means no banner", await banner.isHidden());
  await page.evaluate(() => { window.__emit("update-available", null); window.__emit("update-available", "x"); window.__emit("update-available", { available: true }); window.__emit("show-update", {}); });
  await page.waitForTimeout(200);
  check("bad update payloads and show-update without an offer change nothing", await banner.isHidden());
  await page.evaluate(() => window.__emit("update-available", { current: "0.1.0", latest: "0.2.0", available: false, notesUrl: "https://example.invalid/n" }));
  check("update-available with available false shows no banner", await banner.isHidden());
  await page.evaluate(() => window.__emit("update-available", { current: "0.1.0", latest: "0.2.0", available: true, notesUrl: "https://example.invalid/n" }));
  await page.waitForTimeout(200);
  check("update-available shows the banner", (await banner.isVisible()) && (await banner.locator(".banner-text").textContent()) === "local-stt 0.2.0 is available. You have 0.1.0.");
  check("the new offer is announced to screen readers", (await page.locator(".sr-only[role=status]").textContent()) === "local-stt 0.2.0 is available.");
  check("no page errors (update events)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Settings, updates ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Updates" }) });
  const button = row.locator(".btn");
  const titles = await page.locator(".srow-title").allTextContents();
  check("Settings > General has Updates and Check automatically rows", titles.includes("Updates") && titles.includes("Check automatically"), titles);
  check("Updates row shows the version", (await row.locator(".srow-desc").textContent()) === "local-stt 0.1.0");
  check("Check for Updates is an enabled 28 pt button", (await button.textContent()) === "Check for Updates" && (await button.isEnabled()) && (await button.boundingBox()).height >= 28);
  await button.click();
  await page.waitForTimeout(250);
  check("Check for Updates asks the app and shows Checking...", (await page.evaluate(() => window.__cfg.includes("check_for_updates"))) && (await button.textContent()) === "Checking..." && (await button.isDisabled()));
  await page.evaluate(() => window.__emit("notice", { message: "Start at login is off." }));
  check("an unrelated notice leaves the check running", (await button.textContent()) === "Checking..." && (await button.isDisabled()));
  await page.waitForTimeout(900);
  check("a notice ends the check, toasts it and restores the button", (await page.locator(".toast").textContent()) === "local-stt 0.1.0 is up to date." && (await button.textContent()) === "Check for Updates" && (await button.isEnabled()));
  const auto = page.getByRole("switch", { name: "Check automatically" });
  check("Check automatically copy", (await page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Check automatically" }) }).locator(".srow-desc").textContent()) === "Looks for a new version once a day. Only the request is sent.");
  check("Check automatically starts on", (await auto.getAttribute("aria-checked")) === "true");
  await auto.click();
  await page.waitForTimeout(250);
  check("Check automatically sends check_updates false", (await page.evaluate(() => window.__cfg.includes('set_config:{"check_updates":false}'))) && (await auto.getAttribute("aria-checked")) === "false");
  check("no page errors (updates row)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200&checkfinds=1", { tall: true });
  const button = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Updates" }) }).locator(".btn");
  await button.click();
  await page.waitForTimeout(250);
  check("Checking... shows while the app looks", (await button.textContent()) === "Checking...");
  await page.waitForTimeout(900);
  check("update-available ends the check and shows the banner", (await button.textContent()) === "Check for Updates" && (await page.locator(".mw-banner").isVisible()));
  await ctx.close();
}
for (const dark of [false, true]) {
  const mode = dark ? "dark" : "light";
  const probe = async (query, sel, token, before) => {
    const { page, ctx } = await open(query + "&scheme=" + mode, { dark });
    if (before) await before(page);
    const r = await page.evaluate(inkScript, [sel, token, ".mw-banner"]);
    await ctx.close();
    return r;
  };
  const verdict = (r) => r !== null && r.color === r.expected && r.ratio >= 4.5;
  const install = await probe("?view=main&tab=history&update=1", ".btn-install", "accent-ink");
  check(mode + ": Install and Restart passes 4.5:1", verdict(install), install);
  const quiet = await probe("?view=main&tab=history&update=1", ".mw-banner .btn-quiet", "accent-ink");
  check(mode + ": banner quiet buttons pass 4.5:1", verdict(quiet), quiet);
  const error = await probe("?view=main&tab=history&update=1", ".banner-error", "red-ink", async (page) => {
    await page.evaluate(() => { window.__installError = "Couldn't download the update."; });
    await page.locator(".btn-install").click();
    await page.waitForTimeout(900);
  });
  check(mode + ": banner error text passes 4.5:1", verdict(error), error);
}

// ---------- static hygiene ----------
{
  const files = ["main.js", "overlay.js", "live.js", "main.css", "overlay.css", "live.css", "tokens.css", "main.html", "overlay.html", "live.html"].map((f) => [f, fs.readFileSync(ROOT + "/ui/" + f, "utf8")]);
  const preview = fs.readFileSync(ROOT + "/dev/preview.html", "utf8");
  const bad = [];
  for (const [name, text] of files) {
    if (/\binnerHTML\b|insertAdjacentHTML|setAttribute\(\s*["']style["']|\sstyle=["']/.test(text)) bad.push(name + ": inline style or innerHTML");
    if (/<script(?![^>]*\bsrc=)[^>]*>/.test(text)) bad.push(name + ": inline script");
    if (text.includes("\u2014")) bad.push(name + ": em dash");
  }
  if (preview.includes("\u2014")) bad.push("preview: em dash");
  check("ui/ has no innerHTML, inline style, inline script or em dash", bad.length === 0, bad);
  check("preview lives outside ui/", !fs.existsSync(ROOT + "/ui/preview.html") && fs.existsSync(ROOT + "/dev/preview.html"));
  const refs = [...preview.matchAll(/(?:src|href)="(\.\.\/ui\/[^"]+)"/g)].map((m) => m[1]);
  check("preview loads ../ui/ files that exist", refs.length === 7 && refs.every((r) => fs.existsSync(ROOT + "/dev/" + r)), refs);
}

await browser.close();
console.log(failed ? `${failed} FAILED` : "ALL PASSED");
process.exit(failed ? 1 : 0);
