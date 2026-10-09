// Recording capsule. Listens to overlay-* events and renders; it never decides when it is shown.
(function () {
  "use strict";

  var SVG_NS = "http://www.w3.org/2000/svg";
  var THEMES = ["pill", "waveform", "minimal"];
  var STATES = ["recording", "transcribing", "done", "warning", "error", "positioning"];
  var DEFAULT_LABELS = {
    recording: "Recording",
    transcribing: "Transcribing",
    done: "Done",
    warning: "Check settings",
    error: "Something went wrong",
    positioning: "Drag me anywhere"
  };

  var PILL_BARS = 7;
  var WAVE_BARS = 20;
  var WAVE_STEP_MS = 100; // 20 bars x 100 ms = the last 2 s
  var BAR_MIN = 4;
  var BAR_MAX = 18;
  var ATTACK = 0.6; // per 60 Hz frame
  var RELEASE = 0.12;
  var RIPPLE_FRAMES = 4; // each bar away from the centre shows the level 4 frames older
  var HOLD_STEP_MS = 100; // reduced motion: level is sampled, not animated
  var SHOW_DONE_MS = 600;
  var SHOW_ERROR_MS = 3000; // error and warning share it, the backend hides at the same moment
  var FADE_OUT_MS = 150;
  var LABEL_SWAP_MS = 90;

  function el(tag, className) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    return node;
  }

  function clamp01(n) {
    return n < 0 ? 0 : n > 1 ? 1 : n;
  }

  function formatElapsed(ms) {
    var total = Math.floor(Math.max(0, ms) / 1000);
    var secs = total % 60;
    return Math.floor(total / 60) + ":" + (secs < 10 ? "0" : "") + secs;
  }

  function buildCheck() {
    var svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("class", "ov-check");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("aria-hidden", "true");
    var path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", "M3.5 8.4 6.8 11.6 12.5 4.9");
    path.setAttribute("pathLength", "1");
    svg.appendChild(path);
    return svg;
  }

  function buildWarn() {
    var svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("class", "ov-warn");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("aria-hidden", "true");
    ["M8 2.3 14.3 13.2H1.7Z", "M8 6.6v2.9", "M8 11.5h.01"].forEach(function (d) {
      var path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", d);
      svg.appendChild(path);
    });
    return svg;
  }

  function buildBars(count) {
    var wrap = el("div", "ov-bars");
    wrap.setAttribute("aria-hidden", "true");
    var bars = [];
    for (var i = 0; i < count; i++) {
      var bar = el("i");
      wrap.appendChild(bar);
      bars.push(bar);
    }
    return { wrap: wrap, bars: bars };
  }

  function mountOverlay(root, tauri, options) {
    var autoHide = !(options && options.autoHide === false); // the browser preview keeps done and error on screen
    root.classList.add("ov", "theme-pill");
    root.dataset.state = "recording";
    root.dataset.visible = "false";

    // ---- DOM ----
    var capsule = el("div", "ov-capsule");
    capsule.setAttribute("data-tauri-drag-region", "");

    var lead = el("div", "ov-lead");
    var ring = el("span", "ov-ring");
    var dot = el("span", "ov-dot");
    lead.appendChild(ring);
    lead.appendChild(dot);
    lead.appendChild(buildCheck());
    lead.appendChild(buildWarn());

    var text = el("div", "ov-text");
    var label = el("span", "ov-label");
    label.setAttribute("role", "status");
    label.setAttribute("aria-live", "polite");
    var time = el("span", "ov-time");
    time.setAttribute("aria-hidden", "true");
    text.appendChild(label);
    text.appendChild(time);

    var meter = el("div", "ov-meter");
    var pill = buildBars(PILL_BARS);
    var wave = buildBars(WAVE_BARS);
    pill.wrap.classList.add("ov-bars-pill");
    wave.wrap.classList.add("ov-bars-wave");
    var level = el("div", "ov-level");
    level.setAttribute("aria-hidden", "true");
    var levelFill = el("i");
    level.appendChild(levelFill);
    meter.appendChild(pill.wrap);
    meter.appendChild(wave.wrap);
    meter.appendChild(level);

    capsule.appendChild(lead);
    capsule.appendChild(text);
    capsule.appendChild(meter);
    root.appendChild(capsule);

    // ---- state ----
    var theme = "pill";
    var status = "recording";
    var startedAtMs = null;
    var targetLevel = 0;
    var shownLevel = 0;
    var history = []; // recent smoothed levels, newest last
    var waveValues = [];
    var waveClock = 0;
    var holdClock = 0;
    var reduceFromEvent = false;
    var solidFromEvent = false; // Windows has no native glass, so the app asks for the opaque capsule
    var reduce = false;
    var rafId = 0;
    var lastFrame = 0;
    var tickTimer = 0;
    var hideTimer = 0;
    var swapTimer = 0;
    var visible = false;
    var unlisten = [];

    var reduceQuery = window.matchMedia ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;
    var solidQuery = window.matchMedia
      ? window.matchMedia("(prefers-reduced-transparency: reduce), (prefers-contrast: more)")
      : null;

    for (var w = 0; w < WAVE_BARS; w++) waveValues.push(0);

    // ---- accessibility flags ----
    function applyAccessibility() {
      reduce = reduceFromEvent || !!(reduceQuery && reduceQuery.matches) || root.dataset.forceReduce === "1";
      root.classList.toggle("reduce-motion", reduce);
      root.classList.toggle("solid", solidFromEvent || !!(solidQuery && solidQuery.matches));
    }

    function listenQuery(query) {
      if (!query) return;
      if (query.addEventListener) query.addEventListener("change", applyAccessibility);
      else if (query.addListener) query.addListener(applyAccessibility);
    }

    // ---- drawing ----
    function barHeight(value) {
      return BAR_MIN + (BAR_MAX - BAR_MIN) * clamp01(value);
    }

    function drawPill() {
      var centre = (PILL_BARS - 1) / 2;
      for (var i = 0; i < PILL_BARS; i++) {
        var distance = Math.abs(i - centre);
        var index = history.length - 1 - Math.round(distance * RIPPLE_FRAMES);
        var value = index >= 0 ? history[index] : 0;
        var falloff = 1 - distance * 0.1;
        pill.bars[i].style.height = barHeight(value * falloff) + "px";
      }
    }

    function drawWave() {
      for (var i = 0; i < WAVE_BARS; i++) {
        wave.bars[i].style.height = barHeight(waveValues[i]) + "px";
      }
    }

    function drawRing(value) {
      ring.style.transform = "scale(" + (1 + value * 1.45).toFixed(3) + ")";
      ring.style.opacity = (0.16 + value * 0.5).toFixed(3);
    }

    function drawHeld(value) {
      levelFill.style.transform = "scaleX(" + clamp01(value).toFixed(3) + ")";
      ring.style.opacity = (0.16 + value * 0.5).toFixed(3);
    }

    function resetMeters() {
      shownLevel = 0;
      history = [];
      for (var i = 0; i < WAVE_BARS; i++) waveValues[i] = 0;
      drawPill();
      drawWave();
      drawRing(0);
      drawHeld(0);
    }

    // ---- level loop (recording only) ----
    function frame(now) {
      rafId = 0;
      if (status !== "recording" || reduce) return;
      var dt = lastFrame ? Math.min(now - lastFrame, 100) : 16.667;
      lastFrame = now;
      var base = targetLevel > shownLevel ? ATTACK : RELEASE;
      var k = 1 - Math.pow(1 - base, dt / 16.667); // same feel on 60 and 120 Hz screens
      shownLevel += (targetLevel - shownLevel) * k;
      history.push(shownLevel);
      if (history.length > 40) history.shift();

      waveClock += dt;
      while (waveClock >= WAVE_STEP_MS) {
        waveClock -= WAVE_STEP_MS;
        waveValues.shift();
        waveValues.push(shownLevel);
      }

      if (theme === "pill") drawPill();
      else if (theme === "waveform") drawWave();
      else drawRing(shownLevel);
      rafId = requestAnimationFrame(frame);
    }

    function startLoop() {
      if (reduce || rafId) return;
      lastFrame = 0;
      rafId = requestAnimationFrame(frame);
    }

    function stopLoop() {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = 0;
    }

    // ---- timer ----
    function paintTime() {
      time.textContent = startedAtMs === null ? "-" : formatElapsed(Date.now() - startedAtMs);
    }

    function startTick() {
      stopTick();
      paintTime();
      tickTimer = setInterval(paintTime, 250);
    }

    function stopTick() {
      if (tickTimer) clearInterval(tickTimer);
      tickTimer = 0;
    }

    // ---- label ----
    function setLabel(next) {
      clearTimeout(swapTimer);
      if (!visible || label.textContent === "") {
        label.textContent = next;
        text.classList.remove("swap");
        return;
      }
      if (label.textContent === next) {
        text.classList.remove("swap");
        return;
      }
      text.classList.add("swap");
      swapTimer = setTimeout(function () {
        label.textContent = next;
        text.classList.remove("swap");
      }, reduce ? 0 : LABEL_SWAP_MS);
    }

    // ---- visibility ----
    function show() {
      clearTimeout(hideTimer);
      if (visible) return;
      visible = true;
      // snap to hidden first: after a native hide the capsule can still read as shown
      capsule.classList.add("snap");
      root.dataset.visible = "false";
      void capsule.offsetWidth;
      capsule.classList.remove("snap");
      void capsule.offsetWidth; // commit the hidden style so the entrance transitions
      root.dataset.visible = "true";
    }

    function hide() {
      visible = false;
      root.dataset.visible = "false";
      stopTick();
      stopLoop();
    }

    function hideAfter(ms) {
      clearTimeout(hideTimer);
      if (!autoHide) return;
      hideTimer = setTimeout(hide, Math.max(0, ms - FADE_OUT_MS));
    }

    // ---- events ----
    function onState(payload) {
      if (!payload || STATES.indexOf(payload.state) === -1) return;
      var next = payload.state;
      var previous = status;
      // the app hides the window itself on cancel and on leaving positioning, so JS never saw a hide
      var reopened = (next === "recording" || next === "positioning") && (previous === "recording" || previous === "positioning");
      if (reopened) visible = false;
      status = next;
      root.dataset.state = next;
      var text_ = typeof payload.label === "string" && payload.label !== "" ? payload.label : DEFAULT_LABELS[next];

      if (next === "recording") {
        if (previous !== "recording") resetMeters();
        var started = Number(payload.startedAtMs);
        startedAtMs = payload.startedAtMs !== undefined && payload.startedAtMs !== null && isFinite(started) ? started : null;
        time.hidden = false;
        startTick();
        startLoop();
      } else {
        stopTick();
        stopLoop();
        time.hidden = true;
      }

      setLabel(text_);
      show();
      if (next === "done") hideAfter(SHOW_DONE_MS);
      else if (next === "error" || next === "warning") hideAfter(SHOW_ERROR_MS);
      else clearTimeout(hideTimer);
    }

    function onLevel(payload) {
      var value = payload ? Number(payload.level) : NaN;
      if (!isFinite(value)) return;
      targetLevel = clamp01(value);
      if (status !== "recording" || !reduce) return;
      var now = Date.now();
      if (now - holdClock < HOLD_STEP_MS) return;
      holdClock = now;
      drawHeld(targetLevel);
    }

    function applyTheme(next) {
      if (THEMES.indexOf(next) === -1) return;
      theme = next;
      THEMES.forEach(function (name) {
        root.classList.toggle("theme-" + name, name === next);
      });
    }

    function onTheme(payload) {
      if (!payload) return;
      applyTheme(payload.theme);
      if (typeof payload.reducedMotion === "boolean") reduceFromEvent = payload.reducedMotion;
      if (typeof payload.solid === "boolean") solidFromEvent = payload.solid;
      applyAccessibility();
      if (reduce) stopLoop();
      else if (status === "recording" && visible) startLoop();
    }

    function subscribe(name, handler) {
      var listen = tauri && tauri.event && tauri.event.listen;
      if (!listen) return;
      Promise.resolve(listen(name, function (event) { handler(event.payload); })).then(function (fn) {
        if (typeof fn === "function") unlisten.push(fn);
      }).catch(function () {});
    }

    applyAccessibility();
    listenQuery(reduceQuery);
    listenQuery(solidQuery);
    time.hidden = false;
    paintTime();

    subscribe("overlay-state", onState);
    subscribe("overlay-level", onLevel);
    subscribe("overlay-theme", onTheme);

    // the first theme event can fire before this page is listening, so ask once
    if (tauri && tauri.core && tauri.core.invoke) {
      Promise.resolve(tauri.core.invoke("get_state")).then(function (state) {
        if (state && state.config) applyTheme(state.config.theme);
      }).catch(function () {});
    }

    return {
      destroy: function () {
        stopLoop();
        stopTick();
        clearTimeout(hideTimer);
        clearTimeout(swapTimer);
        unlisten.forEach(function (fn) { fn(); });
        unlisten = [];
      }
    };
  }

  window.mountOverlay = mountOverlay;
  if (!window.__LSTT_PREVIEW__) mountOverlay(document.body, window.__TAURI__);
})();
