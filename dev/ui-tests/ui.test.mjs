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
  if (opts.init) await page.addInitScript(opts.init);
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
  check("the model in use shows under the label, out of the announced status", (await live.locator(".ov-model").isVisible()) && (await live.locator(".ov-model").textContent()) === "Whisper Small"
    && (await live.locator(".ov-model").getAttribute("aria-hidden")) === "true" && (await live.locator(".ov-model").evaluate((m) => m.getBoundingClientRect().bottom <= m.closest(".ov-capsule").getBoundingClientRect().bottom)));
  const timerInk = await live.locator(".ov-time").evaluate((t) => getComputedStyle(t).color);
  const labelInk = await live.locator(".ov-label").evaluate((t) => getComputedStyle(t).color);
  check("timer text is as opaque as the label (contrast fix)", timerInk === labelInk, { timerInk, labelInk });
  // waits for each scripted switch instead of fixed sleeps, which flaked on loaded CI runners
  const reach = (state) => ov.evaluate((el, want) => new Promise((resolve) => {
    if (el.getAttribute("data-state") === want) return resolve(true);
    const watch = new MutationObserver(() => {
      if (el.getAttribute("data-state") === want) { watch.disconnect(); resolve(true); }
    });
    watch.observe(el, { attributes: true, attributeFilter: ["data-state"] });
    setTimeout(() => { watch.disconnect(); resolve(false); }, 8000);
  }), state);
  check("transcribing after recording", await reach("transcribing"));
  // the label crossfades a moment after the state flips, so wait for its text as well
  const pasted = (await reach("done")) && await live.locator(".ov-label", { hasText: "Pasted" }).waitFor({ state: "attached", timeout: 3000 }).then(() => true, () => false);
  check("done shows Pasted", pasted);
  await page.waitForTimeout(900);
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

  await page.fill("#panel-history .search-input", "zzzz");
  check("no-match message has no quotes", (await page.locator("#panel-history .empty-body").textContent()) === "Nothing matches your search.");
  const clearBox = await page.locator("#panel-history .search-clear").boundingBox();
  check("search clear button is 28 pt", clearBox.width >= 28 && clearBox.height >= 28, clearBox);
  await page.fill("#panel-history .search-input", "");

  const more = page.locator("#panel-history .linklike:visible").first();
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
  check("confirm clears history", (await page.locator("#panel-history .empty-title").textContent()) === "No dictations yet");
  const hint = await page.locator("#panel-history .empty-body .key").allTextContents();
  check("macOS empty-state keycaps use Mac names for both shortcuts", hint.join("+") === "fn+Shift+fn+Shift+Space", hint);
  const hintText = await page.locator("#panel-history .empty-body").textContent();
  check("empty-state hint names push to talk and hands-free", hintText === "Hold fnShift and speak, or press fnShiftSpace for hands-free. Your text appears here.", hintText);

  // tabs by keyboard
  const tabs = page.locator("#tab-history");
  await tabs.focus();
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(250);
  check("arrow moves to Notes", (await page.locator("#tab-notes").getAttribute("aria-selected")) === "true");
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
  check("Cmd+2 opens Notes", (await page.locator("#tab-notes").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Meta+3");
  await page.waitForTimeout(250);
  check("Cmd+3 opens Insights", (await page.locator("#tab-insights").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Meta+4");
  await page.waitForTimeout(250);
  check("Cmd+4 opens Model", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true");
  await page.keyboard.press("Meta+5");
  await page.waitForTimeout(250);
  check("Cmd+5 opens Settings", (await page.locator("#tab-settings").getAttribute("aria-selected")) === "true");
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
  const count = () => page.locator("#panel-history .select-count").textContent();
  const total = await rows();
  check("select bar and checkboxes stay hidden until Select", (await page.locator("#panel-history .select-bar").isHidden()) && (await page.locator(".hrow-check").count()) === 0);
  await page.locator("#panel-history .history-bar > .btn", { hasText: "Select" }).click();
  check("Select shows a checkbox per row and hides Clear History", (await page.locator(".hrow-check").count()) === total && (await page.getByText("Clear History", { exact: true }).isHidden()));
  check("Select turns into Done and copy buttons go away", (await page.locator("#panel-history .history-bar > .btn").first().textContent()) === "Done" && (await page.locator(".hrow .copy").count()) === 0);
  check("Delete is off while nothing is picked", (await count()) === "None selected" && (await page.locator("#panel-history .select-bar .btn-danger").isDisabled()));

  await page.locator(".hrow-text").first().click();
  check("clicking a row picks it", (await count()) === "1 selected" && (await page.locator(".hrow").first().getAttribute("class")).includes("is-selected"));
  await page.locator(".hrow-check").nth(2).click();
  check("a checkbox adds to the pick and shows its tick", (await count()) === "2 selected" && (await page.locator(".hrow-check").nth(2).isChecked()));
  await page.locator(".hrow-check").nth(5).click({ modifiers: ["Shift"] });
  const ticks = await page.locator(".hrow-check").evaluateAll((els) => els.map((e) => e.checked));
  check("Shift-click picks the run between, ticks included", (await count()) === "5 selected" && ticks.slice(0, 6).join() === "true,false,true,true,true,true", ticks);
  check("the day box shows a mixed state", await page.locator("#panel-history .group-check").first().evaluate((b) => b.indeterminate || b.checked));

  await page.locator("#select-all").click();
  check("Select All picks every row shown", (await count()) === total + " selected");
  await page.locator("#select-all").click();
  check("Select All again clears the pick", (await count()) === "None selected" && (await page.locator("#panel-history .select-bar .btn-danger").isDisabled()));

  await page.locator(".hrow-check").first().focus();
  await page.keyboard.press("Meta+a");
  check("Cmd+A picks every row shown", (await count()) === total + " selected");
  await page.fill("#panel-history .search-input", "domain");
  await page.waitForTimeout(100);
  check("search drops picks it hides, so Delete never removes unseen rows", (await count()) === "1 selected");
  await page.fill("#panel-history .search-input", "");
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
  check("Esc leaves select mode", (await page.locator("#panel-history .select-bar").isHidden()) && (await page.locator(".hrow-check").count()) === 0);

  await page.locator("#panel-history .history-bar > .btn", { hasText: "Select" }).click();
  const dayRows = await page.locator("#panel-history .card").first().locator(".hrow").count();
  await page.locator("#panel-history .group-check").first().click();
  check("the day box picks that whole day", (await count()) === dayRows + " selected");
  await page.locator("#panel-history .select-bar .btn-danger").click();
  await page.waitForTimeout(250);
  const title = dayRows === 1 ? "Delete this dictation?" : "Delete " + dayRows + " dictations?";
  check("delete dialog counts the pick", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === title);
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(400);
  check("confirm deletes exactly the picked rows", (await rows()) === total - dayRows);
  check("the app is asked to delete them by id", (await page.evaluate(() => window.__calls)).includes("delete_history:" + dayRows));
  check("select mode ends after a delete", await page.locator("#panel-history .select-bar").isHidden());
  const said = await page.locator(".sr-only[role=status]").textContent();
  check("the delete is announced", said === "Deleted " + (dayRows === 1 ? "1 dictation" : dayRows + " dictations") + ".", said);
  check("no page errors (history select)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Notes, voices told apart ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=notes&scheme=light&notes=split");
  const panel = page.locator("#panel-notes");
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("two voices on the call read as Speaker 1 and Speaker 2", (await panel.locator(".turn-name").allTextContents()).join(",") === "Speaker 1,Me,Speaker 2");
  check("each voice gets its own name box, and no Others box", (await panel.locator(".speaker-key").allTextContents()).join(",") === "Me,Speaker 1,Speaker 2");
  const box = panel.locator(".speaker-field", { hasText: "Speaker 2" }).locator("input");
  await box.fill("Priya");
  await box.press("Enter");
  await page.waitForTimeout(250);
  check("naming Speaker 2 renames only that voice", (await panel.locator(".turn-name").allTextContents()).join(",") === "Speaker 1,Me,Priya" && (await page.evaluate(() => window.__calls)).includes("rename_speaker:s2=Priya"));
  check("no page errors (voices)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Notes, the name from setup ----------
{
  const { page, ctx } = await open("?view=main&tab=notes&scheme=light&name=Rupesh");
  const panel = page.locator("#panel-notes");
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("your lines carry the name you gave in setup", (await panel.locator(".turn-name").allTextContents()).join(",") === "Others,Rupesh,Others" && (await panel.locator(".speaker-key").first().textContent()) === "Rupesh");
  await ctx.close();
}

// ---------- main: Settings > Notes > Detect calls ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light");
  const sw = page.getByRole("switch", { name: "Detect calls" });
  check("Settings > Notes has Detect calls, on by default", (await sw.getAttribute("aria-checked")) === "true");
  await sw.click();
  await page.waitForTimeout(250);
  check("turning Detect calls off saves it", (await page.evaluate(() => window.__cfg)).includes('set_config:{"detect_calls":false}'));
  const border = page.getByRole("switch", { name: "Read Zoom's speaking border" });
  check("the speaking border is off by default", (await border.getAttribute("aria-checked")) === "false");
  await border.click();
  await page.waitForTimeout(300);
  const traffic = await page.evaluate(() => window.__cfg);
  check("turning the speaking border on saves it and asks for Screen Recording", traffic.includes('set_config:{"speaking_border":true}') && traffic.includes("request_screen_access"));
  check("no page errors (detect calls)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- toast: call detected ----------
{
  const ctx = await browser.newContext({ viewport: { width: 360, height: 60 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("file://" + ROOT + "/ui/toast.html");
  await page.evaluate(() => {
    window.__calls = [];
    const fake = { core: { invoke: (cmd) => { window.__calls.push(cmd); return Promise.resolve(); } } };
    document.body.textContent = "";
    window.__toast = window.mountToast(document.body, fake);
    window.__toast.show({ call: "Zoom", showMs: 5000 });
  });
  check("the toast names the call", (await page.locator(".toast-title").textContent()) === "Zoom call detected");
  await page.getByRole("button", { name: "Start note" }).click();
  check("Start note starts a note and hides the toast", (await page.evaluate(() => window.__calls.join(","))) === "toast_start_note" && (await page.locator(".toast").isHidden()));
  await page.evaluate(() => window.__toast.show({ call: "Slack", showMs: 300 }));
  await page.waitForTimeout(500);
  check("an ignored toast puts itself away", (await page.evaluate(() => window.__calls.join(","))) === "toast_start_note,toast_dismiss" && (await page.locator(".toast").isHidden()));
  // the toast fills its window, so shrink it in the test to leave room outside it
  await page.setViewportSize({ width: 360, height: 120 });
  await page.evaluate(() => { document.querySelector(".toast").style.height = "60px"; });
  await page.mouse.move(5, 110);
  await page.evaluate(() => window.__toast.show({ call: "Zoom", showMs: 1500 }));
  const box = await page.locator(".toast").boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + 2);
  await page.waitForTimeout(2300);
  check("the pointer on the toast holds it", await page.locator(".toast").isVisible());
  await page.mouse.move(5, 110);
  await page.waitForTimeout(700);
  check("leaving gives it a moment before it goes", await page.locator(".toast").isVisible());
  await page.waitForTimeout(1600);
  check("then it puts itself away", await page.locator(".toast").isHidden());
  check("no page errors (toast)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Settings > Notes > Echo cancellation ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light");
  const sw = page.getByRole("switch", { name: "Echo cancellation" });
  check("Settings has a Notes section with Echo cancellation, on by default", (await page.locator("#panel-settings .group-title", { hasText: "Notes" }).count()) === 1 && (await sw.getAttribute("aria-checked")) === "true");
  await sw.click();
  await page.waitForTimeout(250);
  check("turning Echo cancellation off saves it", (await page.evaluate(() => window.__cfg)).includes('set_config:{"echo_cancellation":false}') && (await sw.getAttribute("aria-checked")) === "false");
  check("no page errors (echo cancellation)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Settings > Appearance ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light");
  const looks = page.locator("#panel-settings .looks");
  check("Appearance is the first Settings section", (await page.locator("#panel-settings .group-title").first().textContent()) === "Appearance");
  check("Appearance offers System, Light and Dark tiles", (await looks.locator(".theme-name").allTextContents()).join(",") === "System,Light,Dark");
  check("System is chosen until the user picks a look", (await looks.locator('[aria-checked="true"] .theme-name').textContent()) === "System" && (await page.evaluate(() => document.documentElement.dataset.scheme)) === undefined);
  check("the System tile names the OS it follows", (await looks.locator(".theme-note").first().textContent()) === "Matches macOS");
  const bg = () => page.evaluate(() => getComputedStyle(document.querySelector("#panel-settings .card")).backgroundColor);
  const lightBg = await bg();
  await looks.locator(".theme-card", { hasText: "Dark" }).click();
  await page.waitForTimeout(250);
  check("Dark saves appearance dark and pins the page dark", (await page.evaluate(() => window.__cfg)).includes('set_config:{"appearance":"dark"}') && (await page.evaluate(() => document.documentElement.dataset.scheme)) === "dark" && (await bg()) !== lightBg, [lightBg, await bg()]);
  await looks.locator('[aria-checked="true"]').press("ArrowLeft");
  await page.waitForTimeout(250);
  check("arrow keys move the choice like a radio group", (await looks.locator('[aria-checked="true"] .theme-name').textContent()) === "Light" && (await page.evaluate(() => document.documentElement.dataset.scheme)) === "light");
  await looks.locator(".theme-card", { hasText: "System" }).click();
  await page.waitForTimeout(250);
  check("System hands the look back to the OS", (await page.evaluate(() => document.documentElement.dataset.scheme)) === undefined);
  check("no page errors (appearance)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows");
  check("on Windows the System tile says it matches Windows", (await page.locator("#panel-settings .looks .theme-note").first().textContent()) === "Matches Windows");
  await ctx.close();
}

// ---------- main: summary models in the Model tab, and note summaries ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=model&scheme=light&ram=24");
  const panel = page.locator("#panel-model .summary-list");
  check("the Model tab has Speech model and Summary model sections", (await page.locator("#panel-model .group-title").allTextContents()).join(",") === "This computer,Speech model,Summary model");
  check("the Summary model section lists both summary models", (await panel.locator(".mrow-name").allTextContents()).join(",") === "Granite 4.0 Micro,Gemma 4 E4B");
  check("on a 24 GB Mac with a GPU, Gemma is recommended", (await panel.locator(".mrow", { hasText: "Gemma 4 E4B" }).locator(".tag-blue").count()) === 1 && (await panel.locator(".mrow", { hasText: "Granite" }).locator(".tag-blue").count()) === 0);
  check("each row says who made it and what it is good at", (await panel.locator(".mrow-about").first().textContent()).startsWith("IBM. Fastest and smallest."));
  await panel.locator(".mrow", { hasText: "Gemma 4 E4B" }).locator(".btn-action").click();
  await page.waitForTimeout(200);
  check("a download asks first and names its size and source", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Download Gemma 4 E4B?" && (await page.locator("dialog.dlg[open] .dlg-body").textContent()).includes("Google's page on Hugging Face"));
  await page.locator("dialog.dlg .btn-action").click();
  await page.waitForTimeout(150);
  check("the download shows progress", (await panel.locator(".progress-text").count()) === 1);
  await page.waitForTimeout(3200);
  check("a finished download is the model in use", (await page.evaluate(() => window.__calls)).includes("choose_summary_model:gemma4-e4b") && (await panel.locator(".mrow", { hasText: "Gemma 4 E4B" }).locator(".pill-active").count()) === 1);
  check("no page errors (summary models)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&platform=windows&ram=32");
  const panel = page.locator("#panel-model .summary-list");
  check("on Windows, Granite is recommended and Gemma warns it runs on the CPU", (await panel.locator(".mrow", { hasText: "Granite" }).locator(".tag-blue").count()) === 1 && (await panel.locator(".mrow", { hasText: "Gemma 4 E4B" }).locator(".reasons").textContent()).includes("Runs on the CPU here"));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light");
  const panel = page.locator("#panel-model .summary-list");
  check("on an 8 GB Mac, Granite is recommended and Gemma says it needs 16 GB", (await panel.locator(".mrow", { hasText: "Granite" }).locator(".tag-blue").count()) === 1 && (await panel.locator(".mrow", { hasText: "Gemma 4 E4B" }).locator(".reasons").textContent()).includes("Needs 16 GB RAM"));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=notes&scheme=light&summaries=on");
  const panel = page.locator("#panel-notes");
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("without a summary model the note offers to choose one", (await panel.locator(".note-summary .btn").textContent()) === "Choose a Summary Model");
  await panel.locator(".note-summary .btn").click();
  await page.waitForTimeout(200);
  check("that button opens the Model tab at its Summary model section", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true" && (await page.locator("#summary-models").isVisible()));
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=notes&scheme=light&summary=granite");
  const panel = page.locator("#panel-notes");
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("a note offers Summarize with the downloaded model", (await panel.locator(".note-summary .btn-action").textContent()) === "Summarize" && (await panel.locator(".note-summary .sum-empty").textContent()).includes("Granite 4.0 Micro"));
  check("the summary sits above the transcript", await page.evaluate(() => { const s = document.querySelector(".note-summary"); const t = document.querySelector(".transcript"); return !!s && !!t && (s.compareDocumentPosition(t) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0; }));
  await panel.locator(".note-summary .btn-action").click();
  await page.waitForTimeout(250);
  check("summarizing shows its steps", /^Summarizing/.test(await panel.locator(".sum-busy").textContent()));
  await page.waitForTimeout(900);
  check("the summary shows an overview", (await panel.locator(".sum-overview").textContent()).startsWith("The team reviewed"));
  check("who said what uses the note's speaker names", (await panel.locator(".sum-name").allTextContents()).join(",") === "Me,Others");
  check("action items show owner and time, or Unassigned", (await panel.locator(".sum-owner").allTextContents()).join("|") === "Others, 0:12|Unassigned, 0:30");
  check("a summarized note offers Summarize Again", (await panel.locator(".note-summary .sum-head .btn").textContent()) === "Summarize Again");
  const othersBox = panel.locator(".speaker-field", { hasText: "Others" }).locator("input");
  await othersBox.fill("Priya");
  await othersBox.press("Enter");
  await page.waitForTimeout(300);
  await panel.locator(".note-back").click();
  await page.waitForTimeout(200);
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("renaming a speaker renames them in the summary too", (await panel.locator(".sum-name").allTextContents()).join(",") === "Me,Priya");
  const copied = () => page.evaluate(() => window.__copied || "");
  await panel.getByRole("button", { name: "Copy summary" }).click();
  await page.waitForTimeout(150);
  const sum = await copied();
  check("Copy summary copies only the summary, with the names", sum.startsWith("Summary\nThe team reviewed") && sum.includes("Priya:") && !/\(\d+:\d\d\): /.test(sum), sum);
  await panel.getByRole("button", { name: "Copy transcript" }).click();
  await page.waitForTimeout(150);
  const lines = (await copied()).split("\n");
  check("Copy transcript copies only the lines, in time order", lines.length > 1 && lines.every((l) => /^(Me|Priya) \(\d+:\d\d\): /.test(l)) && !lines.some((l) => l.startsWith("Summary")), lines);
  await panel.getByRole("button", { name: "Copy note" }).click();
  await page.waitForTimeout(150);
  const all = await copied();
  check("Copy note still copies title, summary and transcript", all.includes("\nSummary\n") && all.endsWith(lines[lines.length - 1]), all.slice(0, 120));
  check("no page errors (note summary)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: the Summaries switch ----------
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light");
  check("Summaries start off", (await page.getByRole("switch", { name: "Summaries" }).getAttribute("aria-checked")) === "false");
  await page.locator("#tab-notes").click();
  await page.waitForTimeout(200);
  await page.locator("#panel-notes .nrow-open").first().click();
  await page.waitForTimeout(300);
  check("with Summaries off by default a note has no Summary card", (await page.locator("#panel-notes .note-summary").count()) === 0);
  await page.locator("#tab-model").click();
  await page.waitForTimeout(200);
  await page.locator("#panel-model .summary-list .mrow", { hasText: "Granite" }).locator(".btn-action").click();
  await page.waitForTimeout(250);
  await page.locator("dialog.dlg .btn-action").click();
  await page.waitForTimeout(3000);
  check("downloading a summary model turns Summaries on", (await page.getByRole("switch", { name: "Summaries" }).getAttribute("aria-checked")) === "true");
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=model&scheme=light&summary=granite");
  const sw = page.getByRole("switch", { name: "Summaries" });
  check("the Summary model section has a Summaries switch, on once a model is downloaded", (await sw.count()) === 1 && (await sw.getAttribute("aria-checked")) === "true");
  await sw.click();
  await page.waitForTimeout(300);
  check("turning Summaries off saves summaries false and says nothing runs", (await page.evaluate(() => window.__cfg)).includes('set_config:{"summaries":false}') && (await page.locator("#panel-model .srow", { has: page.locator(".srow-title", { hasText: "Summaries" }) }).locator(".srow-desc").textContent()).includes("no summary model runs"));
  await page.locator("#tab-notes").click();
  await page.waitForTimeout(200);
  await page.locator("#panel-notes .nrow-open").first().click();
  await page.waitForTimeout(300);
  check("with Summaries off a note has no Summary card", (await page.locator("#panel-notes .note-summary").count()) === 0);
  await page.locator("#tab-model").click();
  await page.waitForTimeout(200);
  await page.getByRole("switch", { name: "Summaries" }).click();
  await page.waitForTimeout(300);
  await page.locator("#tab-notes").click();
  await page.waitForTimeout(200);
  check("turning it back on brings Summarize back", (await page.locator("#panel-notes .note-summary .btn-action").textContent()) === "Summarize");
  check("no page errors (summaries switch)", errors.length === 0, errors);
  await ctx.close();
}

// ---------- main: Notes, names from Zoom's participant list ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=notes&scheme=light&notes=split&people=1");
  const panel = page.locator("#panel-notes");
  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("a note shows who the call app listed", (await panel.locator(".call-people .chip").allTextContents()).join(",") === "Simon Keller,Priya Shah" && (await panel.locator(".call-people-label").textContent()) === "Zoom showed:");
  check("the hint says Zoom does not tell who spoke", (await panel.locator(".call-people-help").textContent()).includes("does not say who spoke"));
  const box = panel.locator(".speaker-field", { hasText: "Speaker 1" }).locator("input");
  check("speaker boxes offer the listed names, your own box does not", (await box.getAttribute("list")) !== null && (await panel.locator(".speaker-field", { hasText: "Me" }).locator("input").getAttribute("list")) === null && (await page.locator("#panel-notes datalist option").count()) === 2);
  await box.fill("Priya Shah");
  await box.press("Enter");
  await page.waitForTimeout(250);
  check("picking a listed name renames that speaker", (await page.evaluate(() => window.__calls)).includes("rename_speaker:s1=Priya Shah"));
  await panel.locator(".speaker-field", { hasText: "Speaker 2" }).locator("input").focus();
  await panel.locator(".call-people .chip", { hasText: "Simon Keller" }).click();
  await page.waitForTimeout(250);
  check("clicking a name chip fills the box you last touched", (await page.evaluate(() => window.__calls)).includes("rename_speaker:s2=Simon Keller") && (await panel.locator(".turn-name").allTextContents()).join(",") === "Priya Shah,Me,Simon Keller");
  check("no page errors (call names)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=notes&scheme=light");
  await page.locator("#panel-notes .nrow-open").first().click();
  await page.waitForTimeout(300);
  check("without a participant list there is no hint", (await page.locator("#panel-notes .call-people").count()) === 0);
  await ctx.close();
}

// ---------- main: Notes ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=notes&scheme=light");
  const panel = page.locator("#panel-notes");
  const titles = await panel.locator(".nrow-title").allTextContents();
  check("Notes lists notes newest first with default and own titles", titles.length === 2 && /^Zoom call, /.test(titles[0]) && titles[1] === "Design review", titles);
  check("a note row shows its first line and length", (await panel.locator(".nrow-first").first().textContent()).startsWith("Let's start") && (await panel.locator(".nrow .hrow-meta").first().textContent()) === "42 min");
  const titleSize = await page.evaluate(() => { document.querySelector("#panel-notes .nrow-open").click(); return new Promise((r) => setTimeout(() => r(parseFloat(getComputedStyle(document.querySelector(".note-title")).fontSize)), 300)); });
  check("the note title is a 22 pt heading", titleSize === 22, titleSize);
  await panel.locator(".note-back").click();
  await page.waitForTimeout(200);

  await panel.locator(".nrow-open").first().click();
  await page.waitForTimeout(300);
  check("opening a note shows its transcript as Me and Others turns", (await panel.locator(".turn").count()) === 3 && (await panel.locator(".turn-name").allTextContents()).join(",") === "Others,Me,Others");
  check("turn times read as minutes and seconds", (await panel.locator(".turn-time").first().textContent()) === "0:04");
  check("the title field shows the default as its placeholder", (await panel.locator(".note-title").getAttribute("placeholder")).startsWith("Zoom call, "));
  await panel.locator(".note-title").fill("Q3 sync");
  await panel.locator(".note-title").press("Enter");
  await page.waitForTimeout(250);
  check("renaming sends the new title", (await page.evaluate(() => window.__calls)).includes("rename_note:Q3 sync"));
  check("each speaker has a name box labelled by its default name", (await panel.locator(".speaker-key").allTextContents()).join(",") === "Me,Others");
  const othersBox = panel.locator(".speaker-field", { hasText: "Others" }).locator("input");
  await othersBox.fill("Priya");
  await othersBox.press("Enter");
  await page.waitForTimeout(250);
  check("naming Others once renames every Others line", (await page.evaluate(() => window.__calls)).includes("rename_speaker:others=Priya") && (await panel.locator(".turn-name").allTextContents()).join(",") === "Priya,Me,Priya");
  await othersBox.fill("");
  await othersBox.press("Enter");
  await page.waitForTimeout(250);
  check("clearing the name brings back Others", (await panel.locator(".turn-name").allTextContents()).join(",") === "Others,Me,Others");
  await panel.locator(".note-back").click();
  await page.waitForTimeout(250);
  check("back returns to the list with the new title", (await panel.locator(".nrow-title").first().textContent()) === "Q3 sync");

  // consent sheet first, then a live note
  await panel.locator(".history-bar .btn-action", { hasText: "New Note" }).click();
  await page.waitForTimeout(250);
  check("the first New Note shows the consent sheet", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Before you take notes");
  check("the sheet shows the consent line to copy", (await page.locator("dialog.dlg[open] .consent-line").textContent()).startsWith("Heads up: I'm taking notes"));
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  check("cancel starts nothing", !(await page.evaluate(() => window.__calls)).includes("start_note"));
  await panel.locator(".history-bar .btn-action", { hasText: "New Note" }).click();
  await page.waitForTimeout(200);
  await page.locator("dialog.dlg .btn-action", { hasText: "Continue" }).click();
  await page.waitForTimeout(400);
  check("Continue saves the consent flag and starts the note", (await page.evaluate(() => window.__cfg.includes('set_config:{"notes_consent_seen":true}'))) && (await page.evaluate(() => window.__calls)).includes("start_note"));
  check("the live note shows Recording, a timer, Stop and Copy Consent Line", (await panel.locator(".note-live .note-rec-label").textContent()) === "Recording" && (await panel.locator(".note-live .btn", { hasText: "Stop" }).count()) === 1 && (await panel.locator(".note-live .btn", { hasText: "Copy Consent Line" }).count()) === 1);
  check("an empty live note says lines come after pauses", (await panel.locator(".turn-empty").textContent()).startsWith("Listening."));
  await page.evaluate(() => window.__emit("note-segment", { id: "feedfacecafe0001", segment: { start_ms: 2000, end_ms: 4000, speaker: "others", text: "<b>Hello</b> from the call." } }));
  await page.evaluate(() => window.__emit("note-segment", { id: "feedfacecafe0001", segment: { start_ms: 500, end_ms: 1500, speaker: "me", text: "Can everyone hear me?" } }));
  await page.waitForTimeout(150);
  check("live lines appear in time order", (await panel.locator(".turn-text").allTextContents()).join("|") === "Can everyone hear me?|<b>Hello</b> from the call.");
  check("transcript text is never parsed as markup", (await panel.locator(".turn-text b").count()) === 0);
  await page.evaluate(() => window.__emit("note-segment", { id: "someothernote000", segment: { start_ms: 0, end_ms: 1, speaker: "me", text: "stray" } }));
  await page.waitForTimeout(100);
  check("lines for another note are ignored", (await panel.locator(".turn").count()) === 2);
  await page.evaluate(() => window.__emit("note-segment", { id: "feedfacecafe0001", heard: "Sam", segment: { start_ms: 6000, end_ms: 7000, speaker: "others", text: "Right, so let us say" } }));
  await page.waitForTimeout(150);
  check("a live call line shows who the call app had talking", (await panel.locator(".turn-name").last().textContent()) === "Sam");
  await panel.locator(".note-back").click();
  await page.waitForTimeout(200);
  const openRec = panel.locator(".history-bar .btn-action");
  check("while recording, the list offers Open Recording", (await openRec.textContent()) === "Open Recording" && (await openRec.isEnabled()));
  await openRec.click();
  await page.waitForTimeout(300);
  check("Open Recording returns to the live note", (await panel.locator(".note-live").count()) === 1 && (await panel.locator(".note-live .btn", { hasText: "Stop" }).count()) === 1);
  await panel.locator(".note-live .btn", { hasText: "Stop" }).click();
  await page.waitForTimeout(100);
  check("Stop asks the app to stop and shows Tidying", (await page.evaluate(() => window.__calls)).includes("stop_note") && (await panel.locator(".note-meta").textContent()).includes("Tidying"));
  await page.waitForTimeout(600);
  check("when tidied the note shows its copy and delete buttons", (await panel.locator(".note-actions .copy").count()) === 1 && (await panel.locator(".note-live").count()) === 0);

  // delete from the note, then select mode on the list
  await panel.locator(".note-actions [aria-label='Delete note']").click();
  await page.waitForTimeout(200);
  check("deleting a note asks first", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Delete this note?");
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(300);
  check("the deleted note leaves the list", (await panel.locator(".nrow").count()) === 2 && (await page.evaluate(() => window.__calls)).includes("delete_notes:1"));
  await panel.locator(".history-bar > .btn", { hasText: "Select" }).click();
  check("Select hides New Note and shows note checkboxes", (await panel.locator(".nrow-check").count()) === 2 && (await panel.locator(".history-bar .btn-action", { hasText: "New Note" }).isHidden()));
  await panel.locator("#select-all-notes").click();
  await panel.locator(".select-bar .btn-danger").click();
  await page.waitForTimeout(200);
  check("deleting picked notes counts them", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Delete 2 notes?");
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(300);
  check("with every note gone, Notes shows its empty state", (await panel.locator(".empty-title").textContent()) === "No notes yet");
  await page.keyboard.press("Meta+f");
  await page.waitForTimeout(150);
  check("Cmd+F on Notes searches notes", (await page.evaluate(() => document.activeElement.getAttribute("aria-label"))) === "Search notes" && (await page.locator("#tab-notes").getAttribute("aria-selected")) === "true");
  check("no page errors (notes)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=notes&scheme=light&notes=old-macos");
  const panel = page.locator("#panel-notes");
  check("an old macOS says why and turns New Note off", (await panel.locator(".notes-gate").textContent()) === "Notes need macOS 14.6 or later." && (await panel.locator(".history-bar .btn-action").isDisabled()));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&consent=seen");
  await page.evaluate(() => window.__emit("show-tab", { tab: "notes", newNote: true }));
  await page.waitForTimeout(400);
  check("the tray's New Note opens Notes and starts at once once consent was seen", (await page.locator("#tab-notes").getAttribute("aria-selected")) === "true" && (await page.evaluate(() => window.__calls)).includes("start_note") && (await page.locator("dialog.dlg[open]").count()) === 0);
  await ctx.close();
}
// ---------- Settings: Accessibility and Reinstall ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&access=denied", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Accessibility$/ }) });
  check("Accessibility row says Not allowed and how to fix a stale switch", (await row.locator(".tag").textContent()) === "Not allowed" && (await row.locator(".srow-desc").textContent()).includes("minus button and add it again"));
  await row.locator(".btn", { hasText: "Open Settings" }).click();
  check("Open Settings asks the app to open the pane", (await calls(page)).includes("open_accessibility_settings"));
  await page.evaluate(() => window.__emit("permissions-changed", {}));
  await page.waitForTimeout(200);
  check("a granted permission is announced", (await page.locator(".sr-only[role=status]").textContent()).startsWith("Accessibility is allowed"));
  const reinstall = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Reinstall$/ }) }).locator(".btn");
  await reinstall.click();
  await page.waitForTimeout(200);
  check("Reinstall asks first and focuses Cancel", (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Reinstall local-stt?" && (await page.evaluate(() => document.activeElement.textContent)) === "Cancel");
  await page.locator("#reinstall-clean").check();
  check("choosing to delete data turns the confirm into Delete and Reinstall", (await page.locator("dialog.dlg .btn-danger-solid").textContent()) === "Delete and Reinstall");
  await page.locator("#reinstall-clean").uncheck();
  await page.locator("dialog.dlg .btn-action", { hasText: "Reinstall" }).click();
  await page.waitForTimeout(300);
  check("Reinstall keeps data by default and reports back", (await calls(page)).includes("reinstall_app:false") && (await page.locator(".toast").textContent()).startsWith("Reinstalled local-stt"));
  check("no page errors (reinstall)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows", { tall: true });
  check("Windows shows no Accessibility row", (await page.locator(".srow-title", { hasText: /^Accessibility$/ }).count()) === 0);
  await ctx.close();
}

// ---------- Fn key switches keyboard language ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&fn=conflict&update=1", { tall: true });
  const banner = page.locator(".mw-banner");
  check("an Fn shortcut that macOS swallows shows the banner first, ahead of the update", (await banner.isVisible()) && (await banner.getAttribute("aria-label")) === "Shortcut problem" && (await banner.textContent()).includes("Your Fn shortcut may not work") && (await banner.textContent()).includes("Do Nothing"));
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Fn key$/ }) });
  check("Settings has an Fn key row saying it switches language", (await row.locator(".tag").textContent()) === "Switches language" && (await row.locator(".srow-desc").textContent()).includes("Do Nothing"));
  await banner.getByRole("button", { name: "Open Keyboard Settings" }).click();
  check("the banner opens Keyboard settings", (await calls(page)).includes("open_keyboard_settings"));
  await banner.getByRole("button", { name: "Later" }).click();
  check("Later puts it off and the update banner takes its place", (await banner.getAttribute("aria-label")) === "Software update" && (await banner.textContent()).includes("0.2.0 is available"));
  await page.evaluate(() => { window.__setFnConflict(false); window.__emit("fn-key-conflict", false); });
  await page.waitForTimeout(200);
  check("once fixed the row says Ready and the fix is announced", (await row.locator(".tag").textContent()) === "Ready" && (await page.locator(".sr-only[role=status]").textContent()).startsWith("Fn is free again"));
  await page.evaluate(() => { window.__setFnConflict(true); window.__emit("fn-key-conflict", true); });
  await page.waitForTimeout(200);
  check("a relapse after Later shows the banner again", (await banner.getAttribute("aria-label")) === "Shortcut problem");
  check("no page errors with the Fn banner", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light", { tall: true });
  check("without the conflict there is no banner and the Fn key row says Ready", (await page.locator(".mw-banner").isHidden()) && (await page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Fn key$/ }) }).locator(".tag").textContent()) === "Ready");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows&fn=conflict", { tall: true });
  check("Windows shows no Fn key row or banner", (await page.locator(".srow-title", { hasText: /^Fn key$/ }).count()) === 0 && (await page.locator(".mw-banner").isHidden()));
  await ctx.close();
}

// ---------- a shortcut the system also uses ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light", { tall: true });
  const banner = page.locator(".mw-banner");
  check("the default shortcuts clash with nothing", await banner.isHidden());
  await page.locator("#tab-settings").click();
  const push = page.locator('.hk[data-slot="hold"]');
  await push.click();
  await page.waitForTimeout(2500);
  const warn = page.locator(".srow", { has: push }).locator(".field-warn");
  check("a captured system shortcut shows a warning under its field", (await warn.isVisible()) && (await warn.textContent()).startsWith("macOS also uses this for switching to the next input source") && (await warn.textContent()).includes("Keyboard Shortcuts"));
  check("and a banner naming the shortcut and what macOS uses it for", (await banner.getAttribute("aria-label")) === "Shortcut problem" && (await banner.textContent()).includes("Your push to talk shortcut is also macOS’s for switching to the next input source"));
  await page.locator("#tab-history").click();
  await banner.getByRole("button", { name: "Change Shortcut" }).click();
  await page.waitForTimeout(200);
  check("Change Shortcut opens Settings at that field", (await page.locator("#tab-settings").getAttribute("aria-selected")) === "true" && (await page.evaluate(() => document.activeElement && document.activeElement.dataset.slot)) === "hold");
  await banner.getByRole("button", { name: "Later" }).click();
  check("Later puts the clash banner off", await banner.isHidden());
  await page.getByRole("button", { name: "Reset Push to talk shortcut" }).click();
  await page.waitForTimeout(400);
  check("a free shortcut clears the warning", await warn.isHidden());
  check("no page errors with the clash warning", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&fn=conflict", { tall: true });
  await page.locator('.hk[data-slot="hold"]').click();
  await page.waitForTimeout(2500);
  const text = await page.locator(".mw-banner").textContent();
  check("the Fn banner outranks a clash banner", text.includes("Your Fn shortcut may not work") && !text.includes("Change Shortcut"));
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
  const row = (name) => page.locator("#panel-model .speech-list .mrow", { hasText: name });
  check("model rows are not radios and have no tab stops", (await page.locator("#panel-model .speech-list .mrow[role=radio], .card[role=radiogroup], .mrow[tabindex]").count()) === 0);
  check("Active shown for the loaded model", (await row("Small").locator(".pill-active").count()) === 1 && (await row("Medium").locator(".pill-active").count()) === 0);
  check("unsupported row has no action button and keeps its reasons", (await row("Large v3").locator(".mrow-head button").count()) === 0 && (await row("Large v3").locator(".reasons li").count()) === 2);
  check("every row shows its full SHA-256 as selectable text", (await page.locator("#panel-model .speech-list .mrow-hash").allTextContents()).every((t) => /^[0-9a-f]{64}$/.test(t))
    && (await page.locator("#panel-model .speech-list .mrow-hash").first().evaluate((e) => getComputedStyle(e).userSelect)) === "text");
  check("no Delete on the active model or one not on disk", (await row("Small").getByRole("button", { name: /^Delete/ }).count()) === 0 && (await row("Medium").getByRole("button", { name: /^Delete/ }).count()) === 0);
  await row("Large v3").getByRole("button", { name: "View Large v3 on Hugging Face" }).click();
  check("Hugging Face opens through the app by model id, even for an unsupported model", (await calls(page)).includes("open_model_page:large-v3"), await calls(page));
  const act = row("Medium").locator(".mrow-head button");
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
  check("Medium becomes Active and Small offers Use", (await row("Medium").locator(".pill-active").count()) === 1 && (await row("Small").locator(".mrow-head button").textContent()) === "Use");
  check("the model no longer in use can be deleted, the active one cannot", (await row("Small").getByRole("button", { name: "Delete Small" }).count()) === 1 && (await row("Medium").getByRole("button", { name: /^Delete/ }).count()) === 0);
  await row("Small").locator(".mrow-head button").click();
  await page.waitForTimeout(200);
  check("Use shows Loading, no confirmation for a downloaded model", (await row("Small").locator(".pill-busy").textContent()) === "Loading" && (await page.locator("dialog.dlg[open]").count()) === 0);
  await page.waitForTimeout(800);
  check("Small is Active again after the load event", (await row("Small").locator(".pill-active").count()) === 1);
  check("no page errors (model)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&active=none");
  const small = page.locator("#panel-model .speech-list .mrow", { hasText: "Small" });
  check("Active follows the loaded model, not config.model", (await small.locator(".pill-active").count()) === 0 && (await small.locator(".mrow-head button").textContent()) === "Use");
  check("the chosen model is not offered for deletion while it is not loaded", (await small.getByRole("button", { name: /^Delete/ }).count()) === 0);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&downloading=medium");
  const med = page.locator("#panel-model .speech-list .mrow", { hasText: "Medium" });
  check("a download already running shows at once", (await med.locator(".pill-busy").textContent()) === "Downloading" && (await med.locator("[role=progressbar]").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=model&scheme=light&fail=medium");
  const med = page.locator("#panel-model .speech-list .mrow", { hasText: "Medium" });
  await med.locator(".mrow-head button").click();
  await page.locator("dialog.dlg .btn-action").click();
  await page.waitForTimeout(3200);
  check("failed download shows the error, offers Download again, keeps Small active",
    (await med.locator(".model-error").count()) === 1 && (await med.locator(".mrow-head button").textContent()) === "Download 514 MB" && (await page.locator("#panel-model .speech-list .mrow", { hasText: "Small" }).locator(".pill-active").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=model&scheme=light&have=medium");
  const med = page.locator("#panel-model .speech-list .mrow", { hasText: "Medium" });
  const del = med.getByRole("button", { name: "Delete Medium" });
  await del.click();
  await page.waitForTimeout(250);
  const body = await page.locator("dialog.dlg[open]").textContent();
  check("delete asks first, names the space freed, and the confirm is destructive",
    (await page.locator("dialog.dlg[open] .dlg-title").textContent()) === "Delete Medium?" && body.includes("514 MB") && (await page.locator("dialog.dlg[open] .btn-danger-solid").textContent()) === "Delete", body);
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  await page.waitForTimeout(150);
  check("cancel keeps the model", !(await calls(page)).some((c) => c.startsWith("delete_model")) && (await med.locator(".tag-green").count()) === 1);
  await del.click();
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(400);
  check("confirmed delete removes it: Download again, no Delete, Small still active",
    (await calls(page)).includes("delete_model:medium") && (await med.locator(".mrow-head button").textContent()) === "Download 514 MB"
    && (await med.getByRole("button", { name: /^Delete/ }).count()) === 0 && (await page.locator("#panel-model .speech-list .mrow", { hasText: "Small" }).locator(".pill-active").count()) === 1);
  check("no page errors (model delete)", errors.length === 0, errors);
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
  check("history list is emptied and says History is off", (await page.locator("#panel-history .empty-title").textContent()) === "History is off");
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
  await page.keyboard.press("Control+4");
  await page.waitForTimeout(250);
  check("Ctrl+4 opens Model on Windows", (await page.locator("#tab-model").getAttribute("aria-selected")) === "true");
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
  const text = await page.locator("#panel-history .empty-body").textContent();
  check("empty hint with hands-free off names only push to talk", text === "Hold fnShift and speak. Your text appears here." && (await page.locator("#panel-history .empty-body .key").count()) === 2, text);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&empty=1&platform=windows");
  const hint = await page.locator("#panel-history .empty-body .key").allTextContents();
  check("Windows empty hint shows both shortcuts", hint.join("+") === "Ctrl+Alt+Ctrl+Alt+Space", hint);
  await ctx.close();
}

// ---------- main: Settings, install and notices ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Install$/ }) });
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
  const button = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Install$/ }) }).locator(".btn");
  check("installed state shows a disabled Installed button with a check", (await button.textContent()) === "Installed" && (await button.isDisabled()) && (await button.locator("svg.ic-check").count()) === 1);
  check("installed state never calls install_app", !(await calls(page)).includes("install_app"));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Install$/ }) });
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
  check("turning Start at login on toasts the app's notice and flips Install to Installed", (await page.locator(".toast").textContent()) === "Installed local-stt and set it to open at login." && (await page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Install$/ }) }).locator(".btn").textContent()) === "Installed");
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
  check("Smart formatting copy", (await descOf("Smart formatting")) === 'Turns spoken lists ("point one, point two" or "first, second") into numbered lists, removes "uh" and "um", and fixes spacing.');
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
    const surface = parse(getComputedStyle(document.querySelector("#panel-history .card")).backgroundColor);
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
    card: getComputedStyle(document.querySelector("#panel-history .card")).boxShadow,
    hairline: +getComputedStyle(document.querySelector(".mw-toolbar"), "::after").opacity,
  }));
  check("Increase Contrast: opaque secondary text, 1 px card border, toolbar hairline", info.label2 === "rgb(60, 60, 67)" && info.card.includes("1px") && info.hairline === 1, info);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=dark&h=1200", { tall: true, dark: true, media: [{ name: "prefers-contrast", value: "more" }] });
  const info = await page.evaluate(() => ({ label2: getComputedStyle(document.querySelector(".group-title")).color, card: getComputedStyle(document.querySelector("#panel-history .card")).boxShadow }));
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
  const quiet = await probe(settings, ".btn-quiet", "accent-ink", "#panel-history .card", { tall: true });
  check(mode + ": Reset gets the accent ink at 4.5:1", verdict(quiet), quiet);
  const turnOff = await probe(settings, "[aria-label='Turn off hands-free']", "accent-ink", "#panel-history .card", { tall: true });
  check(mode + ": Turn off gets the accent ink at 4.5:1", verdict(turnOff), turnOff);
  const done = await probe(settings, ".btn-done", "green-ink", "#panel-history .card", { tall: true });
  check(mode + ": Installed gets the green ink at 4.5:1", verdict(done), done);
  const action = await probe("?view=main&tab=model", ".btn-action", "accent-ink", "#panel-history .card");
  check(mode + ": Download gets the accent ink at 4.5:1", verdict(action), action);
  const danger = await probe("?view=main&tab=history", ".btn-danger", "red-ink", "#panel-history .card");
  check(mode + ": Clear history gets the red ink at 4.5:1", verdict(danger), danger);
  const link = await probe("?view=main&tab=history", ".linklike:not([hidden])", "accent-ink", "#panel-history .card");
  check(mode + ": Show more gets the accent ink at 4.5:1", verdict(link), link);
  const icon = await probe("?view=main&tab=history", ".hrow .copy", "label-2", "#panel-history .card");
  check(mode + ": copy button keeps its secondary ink at 4.5:1", verdict(icon), icon);
  const solid = await probe("?view=main&tab=history", ".btn-danger-solid", "white", ".dlg", { before: async (page) => { await page.getByText("Clear History", { exact: true }).first().click(); await page.waitForTimeout(300); } });
  check(mode + ": the red confirm button has white text at 4.5:1", verdict(solid), solid);
}

// ---------- main: Insights ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light&h=1400", { tall: true });
  const tabNames = await page.locator("[role=tab]").allTextContents();
  check("Insights is the third tab", tabNames.join(",") === "History,Notes,Insights,Model,Settings", tabNames);
  await page.keyboard.press("Meta+3");
  await page.waitForTimeout(300);
  check("Cmd+3 shows the Insights panel", (await page.locator("#panel-insights").isVisible()) && (await page.locator("#tab-insights").getAttribute("aria-selected")) === "true");
  check("streak number shown", (await page.locator(".streak-num").textContent()) === "6" && (await page.locator(".streak-unit").textContent()) === "workdays");
  check("freezes read 4 of 10", (await page.locator("#panel-insights .stat", { hasText: "Freezes" }).locator(".stat-value").textContent()) === "4 of 10");
  check("next freeze line says what earns it", (await page.locator("#panel-insights .ins-line", { hasText: "+1 freeze after 3 more active workdays" }).count()) === 1);
  const info = page.getByRole("button", { name: "How freezes work" });
  check("freeze help starts closed", (await info.getAttribute("aria-expanded")) === "false" && (await page.locator("#freeze-help").isHidden()));
  await info.click();
  const rules = await page.locator("#freeze-help li").allTextContents();
  check("info button opens the freeze rules", (await info.getAttribute("aria-expanded")) === "true" && (await page.locator("#freeze-help").isVisible()) && rules.length === 5 && rules[2] === "You start with 5 freezes. Every 10 active workdays earns one more, up to 10.", rules);
  await page.keyboard.press("Enter");
  check("Enter on the info button closes the rules again", (await info.getAttribute("aria-expanded")) === "false" && (await page.locator("#freeze-help").isHidden()));
  const cells = await page.locator(".week-cell").evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
  check("week row has 7 cells, each with an aria-label", cells.length === 7 && cells.every(Boolean), cells);
  check("week labels name the day and state", cells[0] === "Monday, active" && cells[1] === "Tuesday, covered by a freeze" && cells[6] === "Sunday, bonus", cells);
  const shapes = await page.locator(".week-cell").evaluateAll((els) => els.map((e) => e.querySelector(".week-dot svg") ? "glyph" : getComputedStyle(e.querySelector(".week-dot")).borderStyle));
  check("every day state carries a shape, never colour alone", shapes.every((s) => s === "glyph" || s === "solid" || s === "dashed"), shapes);
  check("words total uses a thousands separator", (await page.locator("#panel-insights .stat", { hasText: "All time" }).first().locator(".stat-value").textContent()) === "15,400");
  check("typing estimate label uses typingWpm", (await page.locator("#panel-insights .ins-line", { hasText: "Estimated against typing at 40 words a minute" }).count()) === 1);
  check("time saved shows minutes and hours", (await page.locator("#panel-insights .stat", { hasText: "Time saved this week" }).locator(".stat-value").textContent()) === "18 min" && (await page.locator("#panel-insights .stat", { hasText: "Time saved all time" }).locator(".stat-value").textContent()) === "6.8 hours");
  check("24 hour bars", (await page.locator(".hours .hour").count()) === 24);
  const heights = await page.locator(".hours .hour").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().height));
  check("the busiest hour is the tallest bar", heights[9] === Math.max(...heights) && heights[0] < heights[9], heights);
  check("Your words shows nouns, verbs and top words", (await page.locator(".wcol-title").allTextContents()).join() === "Top nouns,Top verbs,Top words");
  check("words are labelled approximate and English only", (await page.locator("#panel-insights .footnote", { hasText: "Approximate, English only" }).count()) === 1);
  check("mix legend lists four parts", (await page.locator(".mix-legend li").count()) === 4 && (await page.locator(".mix-legend li").first().textContent()) === "Nouns 42%");
  const ms = await page.locator("#panel-insights .ins-row").allTextContents();
  check("milestones show a date or a dash", ms.length === 6 && ms[0].includes("1,000 words") && ms[0].includes("Oct 2, 2026") && ms[2] === "50,000 words-", ms);
  const dates = await page.locator("#panel-insights .ins-date").allTextContents();
  check("a missing value shows a dash, never 0", dates[2] === "-" && dates[5] === "-", dates);
  check("footer says this Mac", (await page.locator(".ins-privacy").textContent()) === "Everything here stays on this Mac.");
  const sw = page.getByRole("switch", { name: "Word insights" });
  check("Word insights switch starts on", (await sw.getAttribute("aria-checked")) === "true");
  await sw.click();
  await page.waitForTimeout(300);
  check("Word insights switch saves word_insights", (await page.evaluate(() => window.__cfg)).includes('set_config:{"word_insights":false}') && (await page.getByRole("switch", { name: "Word insights" }).getAttribute("aria-checked")) === "false");
  // Reset asks first, Cancel has focus
  await page.locator("#ins-reset").click();
  await page.waitForTimeout(300);
  const dlg = page.locator("dialog.dlg[open]");
  check("Reset asks with the agreed words", (await dlg.locator(".dlg-title").textContent()) === "Reset Insights?" && (await dlg.locator(".dlg-body").textContent()) === "Your streak, freezes, word counts and milestones will be removed from this Mac. This can't be undone.");
  check("Cancel is focused and the confirm is Reset", (await page.evaluate(() => document.activeElement.textContent)) === "Cancel" && (await dlg.locator(".btn-danger-solid").textContent()) === "Reset");
  await dlg.locator(".btn", { hasText: "Cancel" }).click();
  check("Cancel leaves insights alone", !(await calls(page)).includes("reset_insights") && (await page.locator(".streak-num").count()) === 1);
  await page.locator("#ins-reset").click();
  await page.waitForTimeout(300);
  await dlg.locator(".btn-danger-solid").click();
  await page.waitForTimeout(400);
  check("Reset calls reset_insights and shows the empty state", (await calls(page)).includes("reset_insights") && (await page.locator("#panel-insights .empty-title").textContent()) === "No insights yet" && (await page.locator("#panel-insights .empty-body").textContent()) === "Dictate once to start your first streak.");
  check("the footer stays on the empty state", (await page.locator("#ins-reset").count()) === 1);
  // insights-changed refreshes the open tab
  await page.evaluate(() => window.__emit("insights-changed", null));
  await page.waitForTimeout(200);
  check("no page errors (insights)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=insights&scheme=light&insights=empty&h=900");
  check("empty view shows the empty state", (await page.locator("#panel-insights .empty-title").textContent()) === "No insights yet" && (await page.locator(".streak-num").count()) === 0);
  check("no page errors (insights empty)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=insights&scheme=light&insights=nowords&h=1400", { tall: true });
  const titles = await page.locator("#panel-insights .group-title").allTextContents();
  check("no words hides the word cards and keeps the rest", !titles.includes("Your words") && titles.includes("Milestones") && (await page.locator(".wcols").count()) === 0 && (await page.locator(".mix").count()) === 0, titles);
  check("no page errors (insights nowords)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx, errors } = await open("?view=main&tab=insights&scheme=light&insights=fullfreezes&h=1400", { tall: true });
  check("full freezes say so instead of a countdown", (await page.locator("#panel-insights .ins-line", { hasText: "Freezes are full: 10 is the most you can hold" }).count()) === 1 && (await page.locator("#panel-insights .ins-line", { hasText: "+1 freeze" }).count()) === 0);
  check("no page errors (insights full freezes)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=insights&scheme=light&platform=windows&h=1400", { tall: true });
  check("Windows footer says this PC", (await page.locator(".ins-privacy").textContent()) === "Everything here stays on this PC.");
  await page.keyboard.press("Control+3");
  await page.waitForTimeout(250);
  check("Ctrl+3 opens Insights on Windows", (await page.locator("#tab-insights").getAttribute("aria-selected")) === "true");
  await ctx.close();
}
for (const dark of [false, true]) {
  const mode = dark ? "dark" : "light";
  const { page, ctx } = await open("?view=main&tab=insights&scheme=" + mode + "&h=1400", { dark, tall: true });
  const rows = [
    [".stat-label", "label-2", ".ins-card"],
    [".week-name", "label-2", ".ins-card"],
    [".ins-date", "label-2", ".card"],
    [".btn-danger", "red-ink", ".card"],
  ];
  for (const [sel, token, surface] of rows) {
    const r = await page.evaluate(inkScript, [sel, token, surface]);
    check(mode + ": insights " + sel + " passes 4.5:1", r !== null && r.color === r.expected && r.ratio >= 4.5, r);
  }
  const states = await page.locator(".week-dot").evaluateAll((els) => els.map((e) => getComputedStyle(e).color));
  check(mode + ": week glyphs are drawn", states.length === 7);
  await ctx.close();
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

// ---------- main: after an update ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=history&scheme=light&updated=1");
  const banner = page.locator(".mw-banner");
  check("the first start after an update says so", (await banner.isVisible()) && (await banner.locator(".banner-text").textContent()) === "Updated to local-stt 0.1.0.");
  check("it offers What's New and Close", (await banner.locator("button").allTextContents()).join("|") === "What's New|Close");
  check("confetti falls without taking clicks", (await page.locator(".confetti i").count()) === 60 && (await page.locator(".confetti").evaluate((c) => getComputedStyle(c).pointerEvents)) === "none");
  await banner.getByRole("button", { name: "What's New", exact: true }).click();
  check("What's New opens the release notes", (await calls(page)).includes("open_release_notes"), await calls(page));
  await page.waitForTimeout(3700);
  check("the confetti is gone after its burst", (await page.locator(".confetti").count()) === 0);
  await banner.getByRole("button", { name: "Close", exact: true }).click();
  check("Close hides the note", await banner.isHidden());
  check("no page errors (after update)", errors.length === 0, errors);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light&updated=1&reduce=1");
  check("reduced motion: the note without confetti", (await page.locator(".mw-banner .banner-text").textContent()) === "Updated to local-stt 0.1.0." && (await page.locator(".confetti").count()) === 0);
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light");
  check("an ordinary start shows no update note", (await page.locator(".mw-banner").isHidden()) && (await page.locator(".confetti").count()) === 0);
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
    content: document.querySelector("#panel-history .history-bar").getBoundingClientRect().top,
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
  const voice = page.getByRole("switch", { name: "Learn my voice" });
  check("Learn my voice copy", (await page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Learn my voice" }) }).locator(".srow-desc").textContent()) === "Lets notes tell you apart from people in the room. A voice fingerprint, never audio, stays on this computer. Off deletes it.");
  check("Learn my voice starts on", (await voice.getAttribute("aria-checked")) === "true");
  await voice.click();
  await page.waitForTimeout(250);
  check("Learn my voice sends learn_my_voice false", (await page.evaluate(() => window.__cfg.includes('set_config:{"learn_my_voice":false}'))) && (await voice.getAttribute("aria-checked")) === "false");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&platform=windows&h=1200", { tall: true });
  check("Learn my voice shows on Windows too", (await page.getByRole("switch", { name: "Learn my voice" }).count()) === 1);
  await ctx.close();
}
// ---------- Settings: Troubleshooting ----------
{
  const { page, ctx, errors } = await open("?view=main&tab=settings&scheme=light", { tall: true });
  const groups = await page.locator(".group-title").allTextContents();
  check("Settings ends with a Troubleshooting group", groups[groups.length - 1] === "Troubleshooting", groups);
  const log = page.getByRole("switch", { name: "Troubleshooting log" });
  check("Troubleshooting log starts on", (await log.getAttribute("aria-checked")) === "true");
  const logDesc = await page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Troubleshooting log" }) }).locator(".srow-desc").textContent();
  check("log copy says what is never kept", logDesc.includes("Never your words, audio or key presses") && logDesc.includes("Off deletes it"), logDesc);
  await log.click();
  await page.waitForTimeout(250);
  check("turning the log off sends diagnostic_log false", (await page.evaluate(() => window.__cfg.includes('set_config:{"diagnostic_log":false}'))) && (await log.getAttribute("aria-checked")) === "false");
  const exportBtn = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Diagnostic report" }) }).locator(".btn");
  check("Export Report is an enabled 28 pt button", (await exportBtn.textContent()) === "Export Report" && (await exportBtn.isEnabled()) && (await exportBtn.boundingBox()).height >= 28);
  await exportBtn.click();
  await page.waitForTimeout(100);
  check("export shows Saving... and blocks a second click", (await exportBtn.textContent()) === "Saving..." && (await exportBtn.isDisabled()));
  await page.waitForTimeout(600);
  check("export asks the app once and toasts where it went", (await page.evaluate(() => window.__calls.filter((c) => c === "export_diagnostics").length)) === 1 && (await page.locator(".toast").textContent()).startsWith("Saved local-stt-diagnostics-"));
  check("Export Report is ready again", (await exportBtn.textContent()) === "Export Report" && (await exportBtn.isEnabled()));
  check("no page errors (troubleshooting)", errors.length === 0, errors);
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

// ---------- Onboarding ----------
// records row state changes and everything announced to screen readers, from before the first paint
const obLog = () => {
  window.__rowLog = [];
  window.__said = [];
  new MutationObserver((muts) => {
    for (const m of muts) {
      if (m.type === "attributes" && m.target.dataset && m.target.dataset.row) window.__rowLog.push({ id: m.target.dataset.row, state: m.target.dataset.state, t: performance.now() });
      const el = m.target.nodeType === 3 ? m.target.parentElement : m.target;
      const live = el && el.closest ? el.closest(".sr-only[role=status]") : null;
      if (live && live.textContent) window.__said.push(live.textContent);
    }
  }).observe(document, { subtree: true, attributes: true, attributeFilter: ["data-state"], childList: true, characterData: true });
};
const obOpen = (extra = "", opts = {}) => open("?view=main&onboarding=1&scheme=" + (opts.dark ? "dark" : "light") + extra, { init: obLog, ...opts });
const obTitle = (page) => page.locator("#ob-title").textContent();
const obStep = (page) => page.locator(".ob-step").textContent();
const obRow = (page, id) => page.locator('.ob-row[data-row="' + id + '"]');
const obPrimary = (page) => page.locator(".ob-actions .btn-action");
const obBtn = (page, name) => page.locator(".ob").getByRole("button", { name, exact: true });
const cfgCalls = (page) => page.evaluate(() => window.__cfg.filter((c) => c.startsWith("set_config")));
const focusedId = (page) => page.evaluate(() => document.activeElement && document.activeElement.id);
const waitRow = (page, id, state, timeout = 12000) => page.waitForSelector('.ob-row[data-row="' + id + '"][data-state="' + state + '"]', { timeout }).then(() => true, () => false);

{
  const { page, ctx, errors } = await obOpen("&dlms=100");
  check("first run shows setup in place of the tabs", (await page.locator(".ob").isVisible()) && !(await page.locator(".mw-toolbar").isVisible()) && !(await page.locator("#panel-history").isVisible()));
  check("step 1: title, Step 1 of 5, no Back, Skip Setup and Skip", (await obTitle(page)) === "Dictate anywhere. Your voice stays on this Mac." && (await obStep(page)) === "Step 1 of 5" && (await obBtn(page, "Back").count()) === 0 && (await obBtn(page, "Skip Setup").isVisible()) && (await obBtn(page, "Skip").isVisible()));
  check("step 1: the title takes focus and there is one h1", (await focusedId(page)) === "ob-title" && (await page.locator(".ob h1").count()) === 1);
  check("step 1: name field, hint and Continue", (await page.locator("#ob-name").getAttribute("placeholder")) === "Your name" && (await page.locator("#ob-name-hint").textContent()) === "Optional. Stays on this Mac." && (await obPrimary(page).textContent()) === "Continue");
  check("no set_config before a choice is made", (await cfgCalls(page)).length === 0);
  await page.locator("#ob-name").fill("  Ada  ");
  await page.locator("#ob-name").press("Enter");
  await page.waitForTimeout(150);
  check("step 1: Enter continues and the trimmed name goes through set_config", (await obStep(page)) === "Step 2 of 5" && (await cfgCalls(page)).includes('set_config:{"user_name":"Ada"}'), await cfgCalls(page));

  check("step 2: greets by name, Back and Skip Setup, title has focus", (await obTitle(page)) === "Setting things up for you, Ada" && (await obBtn(page, "Back").isVisible()) && (await obBtn(page, "Skip Setup").isVisible()) && (await focusedId(page)) === "ob-title");
  check("step 2: five rows, a list with text states", (await page.locator("ul.ob-list > li").count()) === 5 && (await obRow(page, "hw").locator(".ob-state").textContent()).length > 0);
  check("step 2: Continue waits for the checks", await obPrimary(page).isDisabled());
  check("step 2: computer row reaches Done with the hardware", (await waitRow(page, "hw", "done")) && (await obRow(page, "hw").locator(".ob-detail").textContent()).startsWith("Apple M2, 8 GB memory"));
  check("step 2: a missing disk size shows a dash", (await obRow(page, "hw").locator(".ob-detail").textContent()).endsWith("free disk -"));
  check("step 2: model row names the model that fits", (await waitRow(page, "model", "done")) && (await obRow(page, "model").locator(".ob-detail").textContent()) === "Small fits this Mac");
  check("step 2: microphone row counts the inputs", (await waitRow(page, "mic", "done")) && (await obRow(page, "mic").locator(".ob-detail").textContent()) === "2 microphones found");
  check("step 2: Accessibility row is Done when allowed", (await waitRow(page, "access", "done")) && (await obRow(page, "access").locator(".ob-detail").textContent()) === "Allowed");
  const early = await page.evaluate(() => ({ next: !document.querySelector(".ob-actions .btn-action").disabled, download: document.querySelector('.ob-row[data-row="download"]').dataset.state }));
  check("step 2: Continue is on while the download is still running", early.next && early.download !== "done", early);
  check("step 2: the download row shows real bytes", await page.waitForFunction(() => /^\d+ of 181 MB$/.test(document.querySelector('.ob-row[data-row="download"] .ob-detail').textContent), null, { timeout: 8000 }).then(() => true, () => false));
  check("step 2: choose_model started the recommended model once", (await calls(page)).filter((c) => c === "choose_model:small").length === 1, await calls(page));
  check("step 2: the download row ends Done", await waitRow(page, "download", "done"));
  const log = await page.evaluate(() => window.__rowLog);
  const gaps = ["hw", "model", "mic", "access"].map((id) => {
    const run = log.find((e) => e.id === id && e.state === "running");
    const done = log.find((e) => e.id === id && e.state === "done");
    return run && done ? Math.round(done.t - run.t) : null;
  });
  check("step 2: each row shows Running for at least 300 ms", gaps.every((g) => g !== null && g >= 290), gaps);
  const said = await page.evaluate(() => [...new Set(window.__said)]);
  check("step 2: row results are announced", said.includes("Checking your computer: Done. Apple M2, 8 GB memory, free disk -"), said);
  const pct = said.filter((m) => /percent/.test(m));
  check("step 2: the download is announced every 25 percent only", pct.length > 0 && pct.length <= 3 && pct.every((m) => /, (25|50|75) percent\.$/.test(m)), pct);

  await obBtn(page, "Back").click();
  check("Back from step 2 returns to step 1 with the name kept", (await obStep(page)) === "Step 1 of 5" && (await page.locator("#ob-name").inputValue()) === "Ada");
  await obPrimary(page).click();
  await page.waitForTimeout(100);
  check("coming back to step 2 keeps the finished checks", (await obStep(page)) === "Step 2 of 5" && (await obRow(page, "download").getAttribute("data-state")) === "done" && !(await obPrimary(page).isDisabled()));
  await obPrimary(page).click();

  check("step 3: title, two choices, Back", (await obTitle(page)) === "How do you want to set up?" && (await obStep(page)) === "Step 3 of 5" && (await obBtn(page, "Use Recommended Settings").isVisible()) && (await obBtn(page, "Customize").isVisible()) && (await obBtn(page, "Back").isVisible()));
  const facts = await page.locator(".ob-facts li").allTextContents();
  check("step 3: the recommended list names the model, shortcut, output and switches", facts.length === 7 && facts[0] === "Speech modelSmall" && facts[2] === "OutputEnglish" && facts[3] === "Smart formattingOn" && facts[5] === "Save historyOn" && facts[6] === "Start at loginOff", facts);
  await obBtn(page, "Customize").click();
  const titles = await page.locator(".ob .srow-title").allTextContents();
  check("step 3: Customize lists exactly the setup controls", JSON.stringify(titles) === JSON.stringify(["Push to talk", "Output language", "Spoken language", "Paste into the focused app", "Smart formatting", "Save history", "Start at login"]), titles);
  check("step 3: Customize keeps Step 3 of 5 and has Back and Continue", (await obStep(page)) === "Step 3 of 5" && (await obBtn(page, "Back").isVisible()) && (await obPrimary(page).textContent()) === "Continue");
  await page.locator('.ob button[aria-label="Smart formatting"]').click();
  await page.waitForTimeout(100);
  check("step 3: a Customize switch saves through set_config", (await cfgCalls(page)).includes('set_config:{"smart_format":false}') && (await page.locator('.ob button[aria-label="Smart formatting"]').getAttribute("aria-checked")) === "false");
  await obBtn(page, "Back").click();
  check("step 3: Back from Customize returns to the two choices", (await obTitle(page)) === "How do you want to set up?");
  await obBtn(page, "Use Recommended Settings").click();

  check("step 4: title, Step 4 of 5, both cards with Skip For Now", (await obTitle(page)) === "Allow what local-stt needs" && (await obStep(page)) === "Step 4 of 5" && (await page.locator(".ob-perm").count()) === 2 && (await obBtn(page, "Skip For Now").count()) === 1);
  await obBtn(page, "Test Microphone").click();
  await page.waitForTimeout(300);
  const level = await page.evaluate(() => { const t = document.querySelector(".ob-level"); return { shown: !t.hidden, now: +t.getAttribute("aria-valuenow"), scale: document.querySelector(".ob-level i").style.transform }; });
  check("step 4: the microphone test shows a live level", level.shown && level.now > 0 && /scaleX/.test(level.scale), level);
  check("step 4: Listening state while the test runs", (await obBtn(page, "Listening...").isDisabled()) && (await page.locator('.ob-perm[data-perm="mic"]').textContent()).includes("Say something"));
  await page.waitForSelector(".ob-perm .tag-green", { timeout: 4000 });
  check("step 4: the test ends with Heard you and Try Again", (await page.locator('.ob-perm[data-perm="mic"] .tag-green').textContent()) === "Heard you" && (await obBtn(page, "Try Again").isVisible()) && (await calls(page)).includes("check_microphone"));
  check("step 4: Accessibility card shows Allowed when it is on", (await page.locator('.ob-perm[data-perm="access"] .tag-green').textContent()) === "Allowed" && (await obBtn(page, "Open Settings").count()) === 0);
  await obPrimary(page).click();

  check("step 5: title, Step 5 of 5, Skip, Finish, Back", (await obTitle(page)) === "Try it" && (await obStep(page)) === "Step 5 of 5" && (await obBtn(page, "Skip").isVisible()) && (await obBtn(page, "Finish").isVisible()) && (await obBtn(page, "Back").isVisible()));
  check("step 5: asks to hold the shortcut and the box has focus", (await page.locator(".ob-prompt").textContent()).startsWith("Hold ") && (await page.locator(".ob-prompt kbd").count()) === 2 && (await focusedId(page)) === "ob-box" && (await page.locator("#ob-box").isEnabled()));
  check("step 5: no success before any text", await page.locator(".ob-success").isHidden());
  await page.locator("#ob-box").pressSequentially("Hello there");
  check("step 5: success when the box gets text", (await page.locator(".ob-success").isVisible()) && (await page.locator(".ob-success").textContent()).startsWith("That's it. Hold") && (await page.locator(".ob-success").textContent()).endsWith(" in any app."));
  await obBtn(page, "Finish").click();
  await page.waitForTimeout(300);
  check("Finish sends setup_done true with the name", (await cfgCalls(page)).includes('set_config:{"setup_done":true,"user_name":"Ada"}'), await cfgCalls(page));
  check("Finish shows History and the menu bar toast", (await page.locator(".mw-toolbar").isVisible()) && (await page.locator("#tab-history").getAttribute("aria-selected")) === "true" && (await page.locator("#panel-history").isVisible()) && !(await page.locator(".ob").isVisible()) && (await page.locator(".toast").textContent()) === "local-stt lives in the menu bar");
  check("no page errors (setup flow)", errors.length === 0, errors);
  await ctx.close();
}

// Skip Setup on every step, Skip on step 1 and 5
{
  const { page, ctx } = await obOpen("&downloaded=1");
  await obBtn(page, "Skip Setup").click();
  await page.waitForTimeout(250);
  check("Skip Setup on step 1 saves setup_done true and shows History", (await cfgCalls(page)).join() === 'set_config:{"setup_done":true,"user_name":""}' && (await page.locator("#panel-history").isVisible()) && !(await page.locator(".ob").isVisible()));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&downloaded=1");
  await obBtn(page, "Skip").click();
  await page.waitForTimeout(150);
  check("Skip on step 1 moves on without a name or a set_config call", (await obStep(page)) === "Step 2 of 5" && (await obTitle(page)) === "Setting things up for you" && (await cfgCalls(page)).length === 0);
  await waitRow(page, "access", "done");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  check("Recommended changes nothing", (await cfgCalls(page)).length === 0 && (await obStep(page)) === "Step 4 of 5", await cfgCalls(page));
  await obPrimary(page).click();
  await obBtn(page, "Skip").click();
  await page.waitForTimeout(250);
  check("Skip on step 5 saves setup_done true", (await cfgCalls(page)).join() === 'set_config:{"setup_done":true,"user_name":""}' && (await page.locator("#panel-history").isVisible()));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&downloaded=1");
  await obPrimary(page).click();
  await waitRow(page, "download", "done");
  check("an installed model reads Already downloaded and is not fetched again", (await obRow(page, "download").locator(".ob-detail").textContent()) === "Already downloaded" && !(await calls(page)).some((c) => c.startsWith("choose_model")), await calls(page));
  const seen = [await obBtn(page, "Skip Setup").isVisible()];
  await obPrimary(page).click();
  seen.push(await obBtn(page, "Skip Setup").isVisible());
  await obBtn(page, "Use Recommended Settings").click();
  seen.push(await obBtn(page, "Skip Setup").isVisible());
  await obPrimary(page).click();
  seen.push(await obBtn(page, "Skip Setup").isVisible());
  check("Skip Setup is in the header on steps 2 to 5", seen.length === 4 && seen.every(Boolean), seen);
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&downloading=small&dlms=150");
  await obPrimary(page).click();
  check("a download already running at launch is followed, not restarted", (await waitRow(page, "download", "done", 15000)) && !(await calls(page)).some((c) => c.startsWith("choose_model")), await calls(page));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&fail=small&dlms=100");
  await obPrimary(page).click();
  check("a failed download shows Failed with Try Again", (await waitRow(page, "download", "failed", 15000)) && (await obRow(page, "download").locator(".ob-detail").textContent()).startsWith("The download failed") && (await obRow(page, "download").getByRole("button", { name: "Try Again" }).isVisible()));
  await obRow(page, "download").getByRole("button", { name: "Try Again" }).click();
  await page.waitForTimeout(200);
  check("Try Again asks for the model again", (await calls(page)).filter((c) => c === "choose_model:small").length === 2, await calls(page));
  await ctx.close();
}

// no microphone, access denied
{
  const { page, ctx } = await obOpen("&inputs=none&access=denied&downloaded=1");
  await obPrimary(page).click();
  check("no microphone is Needs You, not a failure", (await waitRow(page, "mic", "needs")) && (await obRow(page, "mic").locator(".ob-detail").textContent()).startsWith("No microphone found") && (await obRow(page, "mic").locator(".ob-state").textContent()) === "Needs You");
  check("Accessibility not yet allowed is Needs You", (await waitRow(page, "access", "needs")) && (await obRow(page, "access").locator(".ob-detail").textContent()) === "You'll allow this in a moment");
  await waitRow(page, "download", "done");
  check("Needs You rows do not block Continue", !(await obPrimary(page).isDisabled()));
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  await obBtn(page, "Test Microphone").click();
  await page.waitForSelector('.ob-perm[data-perm="mic"] [role=alert]', { timeout: 3000 });
  check("a refused microphone test says why and offers Try Again", (await page.locator('.ob-perm[data-perm="mic"]').textContent()).includes("No microphone found.") && (await obBtn(page, "Try Again").isVisible()));
  const ax = page.locator('.ob-perm[data-perm="access"]');
  check("Accessibility card offers Open Settings and Skip For Now", (await ax.getByRole("button", { name: "Open Settings" }).isVisible()) && (await ax.getByRole("button", { name: "Skip For Now" }).isVisible()));
  await ax.getByRole("button", { name: "Open Settings" }).click();
  check("Open Settings asks the app and shows Waiting for you", (await calls(page)).includes("open_accessibility_settings") && (await ax.textContent()).includes("Waiting for you to turn on local-stt"));
  await page.evaluate(() => { window.__setAccess(true); window.__emit("permissions-changed", {}); });
  await ax.locator(".tag-green").waitFor({ timeout: 3000 });
  check("Accessibility flips to Allowed on permissions-changed", (await ax.locator(".tag-green").textContent()) === "Allowed" && (await ax.getByRole("button", { name: "Open Settings" }).count()) === 0);
  await obBtn(page, "Skip For Now").first().click();
  check("Skip For Now marks the microphone card skipped", (await page.locator('.ob-perm[data-perm="mic"]').textContent()).includes("Skipped for now"));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&mic=silent&downloaded=1");
  await obPrimary(page).click();
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  await obBtn(page, "Test Microphone").click();
  await page.waitForSelector('.ob-perm[data-perm="mic"] .tag-red', { timeout: 4000 });
  const text = await page.locator('.ob-perm[data-perm="mic"]').textContent();
  check("a silent test says Didn't hear anything with the macOS path and Try Again", text.includes("Didn't hear anything") && text.includes("Open System Settings > Privacy & Security > Microphone") && (await obBtn(page, "Try Again").isVisible()), text);
  await ctx.close();
}

// step 5 without Accessibility, and with the model still downloading
{
  const { page, ctx } = await obOpen("&access=denied&dlms=1000");
  await obPrimary(page).click();
  await waitRow(page, "access", "needs");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  await obPrimary(page).click();
  check("step 5 on macOS without Accessibility says Allow Accessibility first", (await page.locator(".ob-try").textContent()).includes("Allow Accessibility first") && (await page.locator("#ob-box").count()) === 0 && (await obBtn(page, "Open Settings").isVisible()));
  await page.evaluate(() => { window.__setAccess(true); window.__emit("permissions-changed", {}); });
  await page.waitForSelector("#ob-box", { timeout: 3000 });
  check("step 5 switches to the box on permissions-changed", (await page.locator("#ob-box").count()) === 1);
  check("step 5 disables the box while the model downloads and shows the bytes", (await page.locator("#ob-box").isDisabled()) && /^\d+ of 181 MB$/.test(await page.locator(".ob-dl .progress-text").textContent()) && (await page.locator(".ob-dl [role=progressbar]").count()) === 1);
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&dlms=120");
  await obPrimary(page).click();
  await waitRow(page, "access", "done");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  await obPrimary(page).click();
  const before = await page.locator("#ob-box").isDisabled();
  await page.waitForFunction(() => !document.querySelector("#ob-box").disabled, null, { timeout: 10000 });
  check("step 5 enables and focuses the box when the download ends", before && (await focusedId(page)) === "ob-box");
  await ctx.close();
}

// Windows
{
  const { page, ctx } = await obOpen("&platform=windows&downloaded=1");
  check("Windows: title says this PC", (await obTitle(page)) === "Dictate anywhere. Your voice stays on this PC." && (await page.locator("#ob-name-hint").textContent()) === "Optional. Stays on this PC.");
  await obPrimary(page).click();
  await waitRow(page, "mic", "done");
  check("Windows: no Accessibility row", (await page.locator("ul.ob-list > li").count()) === 4 && (await obRow(page, "access").count()) === 0 && (await obRow(page, "model").locator(".ob-detail").textContent()).endsWith("fits this PC"));
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  check("Windows: step 4 has only the microphone card, with a Windows hint", (await obTitle(page)) === "Check your microphone" && (await page.locator(".ob-perm").count()) === 1 && (await page.locator(".ob").textContent()).includes("Audio stays on this PC") && (await page.locator(".ob").textContent()).indexOf("Accessibility") === -1);
  await obPrimary(page).click();
  check("Windows: step 5 goes straight to the box", (await page.locator("#ob-box").isEnabled()) && (await page.locator(".ob-prompt").textContent()).includes("Hold Ctrl"));
  await obBtn(page, "Finish").click();
  await page.waitForTimeout(250);
  check("Windows: the toast says system tray", (await page.locator(".toast").textContent()) === "local-stt lives in the system tray");
  await ctx.close();
}

// when it shows
{
  const { page, ctx } = await open("?view=main&tab=history&scheme=light");
  check("a finished setup never shows on start", !(await page.locator(".ob").isVisible()) && (await page.locator("#panel-history").isVisible()));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&demo=1");
  check("the demo never shows setup", !(await page.locator(".ob").isVisible()) && (await page.locator(".mw-toolbar").isVisible()));
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const run = page.locator(".srow", { has: page.locator(".srow-title", { hasText: "Run Setup Again" }) });
  check("Settings has a Run Setup Again row", (await run.locator(".btn").textContent()) === "Run Setup Again");
  await run.locator(".btn").click();
  await page.waitForTimeout(150);
  check("Run Setup Again shows step 1 without resetting settings", (await obStep(page)) === "Step 1 of 5" && !(await page.locator(".mw-toolbar").isVisible()) && (await cfgCalls(page)).length === 0);
  await obBtn(page, "Skip Setup").click();
  await page.waitForTimeout(250);
  check("Skip Setup after a rerun returns to History", (await page.locator("#panel-history").isVisible()) && (await page.locator("#tab-history").getAttribute("aria-selected")) === "true");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  const row = page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Your name$/ }) });
  const input = row.locator("input");
  await input.fill("Ada");
  await input.blur();
  await page.waitForTimeout(150);
  check("Your name saves on blur", (await cfgCalls(page)).includes('set_config:{"user_name":"Ada"}') && (await row.locator(".field-error").isHidden()));
  await input.fill("bad\u0001name");
  await input.blur();
  await page.waitForTimeout(150);
  check("the app's refusal shows under the name field", (await row.locator(".field-error").textContent()) === "Name can't contain control characters." && (await input.getAttribute("maxlength")) === "60");
  await ctx.close();
}
{
  const { page, ctx } = await open("?view=main&tab=settings&scheme=light&h=1200", { tall: true });
  await page.locator(".srow", { has: page.locator(".srow-title", { hasText: /^Reinstall$/ }) }).locator(".btn").click();
  await page.waitForTimeout(150);
  const body = () => page.locator("dialog.dlg .dlg-body").textContent();
  check("the Reinstall dialog stays as before without the delete box", !(await body()).includes("Setup runs again"));
  await page.locator("#reinstall-clean").check();
  check("ticking delete adds that Setup runs again afterwards", (await body()).endsWith(" Setup runs again afterwards."));
  await page.locator("#reinstall-clean").uncheck();
  check("unticking removes the sentence", !(await body()).includes("Setup runs again"));
  await ctx.close();
}

// motion and look
{
  const { page, ctx } = await obOpen("&dlms=1500", { reduced: true });
  await obPrimary(page).click();
  await waitRow(page, "download", "running");
  const info = await page.evaluate(() => {
    const row = document.querySelector('.ob-row[data-row="download"]');
    return { text: row.querySelector(".ob-state").textContent, anim: getComputedStyle(row.querySelector(".spin")).animationName };
  });
  check("reduced motion: the spinner stops and Running stays as text", info.anim === "none" && info.text === "Running", info);
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&dlms=1500");
  await obPrimary(page).click();
  await waitRow(page, "download", "running");
  const anim = await page.evaluate(() => getComputedStyle(document.querySelector('.ob-row[data-row="download"] .spin')).animationName);
  check("normal motion: the spinner turns", anim === "spin", anim);
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&downloaded=1", { reduced: true });
  await obPrimary(page).click();
  await waitRow(page, "access", "done");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  await obBtn(page, "Test Microphone").click();
  await page.waitForTimeout(200);
  check("reduced motion: the level bar is off and Listening stays as text", (await page.locator(".ob-level").evaluate((n) => getComputedStyle(n).display)) === "none" && (await page.locator('.ob-perm[data-perm="mic"]').textContent()).includes("Listening"));
  await ctx.close();
}
for (const dark of [false, true]) {
  const mode = dark ? "dark" : "light";
  const probe = async (extra, sel, token, surface, before) => {
    const { page, ctx } = await obOpen(extra, { dark });
    if (before) await before(page);
    const r = await page.evaluate(inkScript, [sel, token, surface]);
    await ctx.close();
    return r;
  };
  const verdict = (r) => r !== null && r.color === r.expected && r.ratio >= 4.5;
  const skip = await probe("&downloaded=1", ".ob-skip", "accent-ink", ".mw");
  check(mode + ": Skip Setup passes 4.5:1", verdict(skip), skip);
  const primary = await probe("&downloaded=1", ".ob-actions .btn-action", "accent-ink", ".mw");
  check(mode + ": the primary button passes 4.5:1", verdict(primary), primary);
  const done = await probe("&downloaded=1", '.ob-row[data-state="done"] .ob-state', "green-ink", ".ob-list", async (page) => { await obPrimary(page).click(); await waitRow(page, "hw", "done"); });
  check(mode + ": Done passes 4.5:1", verdict(done), done);
  const needs = await probe("&inputs=none&downloaded=1", '.ob-row[data-state="needs"] .ob-state', "orange-ink", ".ob-list", async (page) => { await obPrimary(page).click(); await waitRow(page, "mic", "needs"); });
  check(mode + ": Needs You passes 4.5:1", verdict(needs), needs);
  const failed = await probe("&fail=small&dlms=100", '.ob-row[data-state="failed"] .ob-state', "red-ink", ".ob-list", async (page) => { await obPrimary(page).click(); await waitRow(page, "download", "failed", 15000); });
  check(mode + ": Failed passes 4.5:1", verdict(failed), failed);
  const hint = await probe("&downloaded=1", ".ob-hint", "label-2", ".ob-card");
  check(mode + ": the hint text passes 4.5:1", verdict(hint), hint);
  const step = await probe("&downloaded=1", ".ob-step", "label-2", ".mw");
  check(mode + ": Step N of 5 passes 4.5:1", verdict(step), step);
  const waiting = await probe("&access=denied&downloaded=1", ".ob-waiting", "accent-ink", ".ob-perm", async (page) => {
    await obPrimary(page).click();
    await waitRow(page, "access", "needs");
    await obPrimary(page).click();
    await obBtn(page, "Use Recommended Settings").click();
    await page.locator('.ob-perm[data-perm="access"]').getByRole("button", { name: "Open Settings" }).click();
  });
  check(mode + ": Waiting for you passes 4.5:1", verdict(waiting), waiting);
}
for (const dark of [false, true]) {
  const { page, ctx } = await obOpen("&downloaded=1", { dark });
  await page.evaluate(() => document.querySelector(".ob-page").scrollIntoView());
  const info = await page.evaluate(() => {
    const cs = getComputedStyle(document.querySelector(".ob-input"));
    return { bg: cs.backgroundColor, page: getComputedStyle(document.querySelector(".mw")).backgroundColor };
  });
  check((dark ? "dark" : "light") + ": setup draws on the theme tokens", info.bg !== info.page && /^rgba?\(/.test(info.bg), info);
  await ctx.close();
}

// ---------- Onboarding: Fn key switches keyboard language ----------
{
  const { page, ctx } = await obOpen("&fn=conflict&downloaded=1");
  await obPrimary(page).click();
  await waitRow(page, "download", "done");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  const fk = page.locator('.ob-perm[data-perm="fnkey"]');
  check("setup shows an Fn key card with the fix", (await fk.isVisible()) && (await fk.textContent()).includes("Do Nothing") && (await fk.locator(".tag-red").textContent()) === "Switches language");
  await fk.getByRole("button", { name: "Open Keyboard Settings" }).click();
  check("the setup card opens Keyboard settings", (await calls(page)).includes("open_keyboard_settings"));
  await page.evaluate(() => { window.__setFnConflict(false); window.__emit("fn-key-conflict", false); });
  await fk.locator(".tag-green").waitFor({ timeout: 3000 });
  check("the card flips to Fixed and hides its button", (await fk.locator(".tag-green").textContent()) === "Fixed" && (await fk.getByRole("button", { name: "Open Keyboard Settings" }).isHidden()));
  await ctx.close();
}
{
  const { page, ctx } = await obOpen("&downloaded=1");
  await obPrimary(page).click();
  await waitRow(page, "download", "done");
  await obPrimary(page).click();
  await obBtn(page, "Use Recommended Settings").click();
  check("setup has no Fn key card when Fn is free", (await page.locator('.ob-perm[data-perm="fnkey"]').count()) === 0);
  await ctx.close();
}

// ---------- static hygiene ----------
{
  const files = ["main.js", "overlay.js", "live.js", "toast.js", "main.css", "overlay.css", "live.css", "toast.css", "tokens.css", "main.html", "overlay.html", "live.html", "toast.html"].map((f) => [f, fs.readFileSync(ROOT + "/ui/" + f, "utf8")]);
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
