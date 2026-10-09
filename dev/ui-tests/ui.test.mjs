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
  await btn("Move pill").click();
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

  await page.getByText("Clear history", { exact: true }).first().click();
  await page.waitForTimeout(250);
  check("clear history asks first", (await page.locator("dialog.dlg[open]").count()) === 1);
  await page.locator("dialog.dlg .btn", { hasText: "Cancel" }).click();
  check("cancel keeps history", (await page.locator(".hrow").count()) > 0);
  await page.getByText("Clear history", { exact: true }).first().click();
  await page.locator("dialog.dlg .btn-danger-solid").click();
  await page.waitForTimeout(300);
  check("confirm clears history", (await page.locator(".empty-title").textContent()) === "No dictations yet");
  const hint = await page.locator(".empty-body .key").allTextContents();
  check("macOS empty-state keycaps use Mac names", hint.join("+") === "fn+Shift", hint);

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
  const move = page.getByRole("button", { name: "Move pill" });
  check("Pill position row has Move pill and Reset position", (await page.locator(".srow-title", { hasText: "Pill position" }).count()) === 1 && (await move.count()) === 1 && (await page.getByRole("button", { name: "Reset position" }).count()) === 1);
  await move.click();
  await page.waitForTimeout(200);
  check("Move pill calls move_overlay on:true and becomes Done", (await calls(page)).includes("move_overlay:true") && (await page.getByRole("button", { name: "Done", exact: true }).count()) === 1);
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.waitForTimeout(200);
  check("Done calls move_overlay on:false", (await calls(page)).join() === "move_overlay:true,move_overlay:false", await calls(page));
  await page.getByRole("button", { name: "Move pill" }).click();
  await page.locator("#tab-history").click();
  await page.waitForTimeout(200);
  check("leaving the Settings tab stops positioning", (await calls(page)).slice(-1)[0] === "move_overlay:false", await calls(page));
  await page.locator("#tab-settings").click();
  check("button is back to Move pill after returning", (await page.getByRole("button", { name: "Move pill" }).count()) === 1);
  await page.getByRole("button", { name: "Reset position" }).click();
  await page.waitForTimeout(250);
  check("Reset position resets and says so", (await calls(page)).includes("reset_overlay_position") && (await page.locator(".toast").textContent()) === "Position reset.");

  // Reset while the pill is up re-places it: hide, then show again
  await page.getByRole("button", { name: "Move pill" }).click();
  await page.getByRole("button", { name: "Reset position" }).click();
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

  check("hotkey shortcut copy", (await page.locator(".srow-desc", { hasText: "Click, then press the new shortcut." }).count()) === 1);
  check("start at login copy has no shell command", (await page.locator(".srow-desc", { hasText: "uv tool" }).count()) === 0);
  const hk = page.locator(".hk");
  check("Mac keycaps in settings", (await hk.locator(".key").allTextContents()).join("+") === "fn+Shift");
  await hk.click();
  await page.waitForTimeout(150);
  check("capture start is announced", (await page.locator(".sr-only[role=status]").textContent()) === "Press your shortcut now. Escape cancels.");
  await page.waitForTimeout(2400);
  check("captured combo uses Mac names", (await hk.locator(".key").allTextContents()).join("+") === "Control+Option+Space", await hk.locator(".key").allTextContents());
  await page.getByRole("button", { name: "Reset", exact: true }).click();
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
  const hk = page.locator(".hk");
  check("Windows keycaps keep Ctrl and Alt", (await hk.locator(".key").allTextContents()).join("+") === "Ctrl+Alt");
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
  await page.getByRole("button", { name: "Move pill" }).click();
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

// ---------- static hygiene ----------
{
  const files = ["main.js", "overlay.js", "main.css", "overlay.css", "tokens.css", "main.html", "overlay.html"].map((f) => [f, fs.readFileSync(ROOT + "/ui/" + f, "utf8")]);
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
  check("preview loads ../ui/ files that exist", refs.length === 5 && refs.every((r) => fs.existsSync(ROOT + "/dev/" + r)), refs);
}

await browser.close();
console.log(failed ? `${failed} FAILED` : "ALL PASSED");
process.exit(failed ? 1 : 0);
