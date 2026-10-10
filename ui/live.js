// Live transcript popover. Listens to live-* events and renders; it never decides when it is shown.
(function () {
  "use strict";

  var PLACEHOLDER = "Listening...";
  var TAG_TEXT = "Formats on paste";

  function el(tag, className) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  // the app resends only the newest seconds, so the front drops off and whisper revises words
  // unchanged length = the longest start of next that already sits somewhere in prev
  function unchangedLength(prev, next) {
    var best = 0;
    for (var skip = 0; skip < prev.length; skip++) {
      var i = 0;
      while (skip + i < prev.length && i < next.length && prev.charCodeAt(skip + i) === next.charCodeAt(i)) i++;
      if (i > best) best = i;
    }
    return best;
  }

  function mountLive(root, tauri) {
    root.classList.add("lv");

    var card = el("div", "lv-card");
    var body = el("p", "lv-text is-empty");
    body.setAttribute("role", "status");
    body.setAttribute("aria-live", "polite");
    body.setAttribute("aria-atomic", "false");
    body.textContent = PLACEHOLDER;
    var tag = el("span", "lv-tag");
    tag.textContent = TAG_TEXT;
    tag.hidden = true;
    card.appendChild(body);
    card.appendChild(tag);
    root.appendChild(card);

    var shown = "";
    var formattedSeen = false; // an event's flag is newer than the one read at load
    var reduceFromEvent = false;
    var solidFromEvent = false; // Windows has no native glass, so the app asks for the opaque card
    var unlisten = [];

    var reduceQuery = window.matchMedia ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;
    var solidQuery = window.matchMedia
      ? window.matchMedia("(prefers-reduced-transparency: reduce), (prefers-contrast: more)")
      : null;

    function applyAccessibility() {
      var reduce = reduceFromEvent || !!(reduceQuery && reduceQuery.matches) || root.dataset.forceReduce === "1";
      root.classList.toggle("reduce-motion", reduce);
      root.classList.toggle("solid", solidFromEvent || !!(solidQuery && solidQuery.matches));
    }

    function listenQuery(query) {
      if (!query) return;
      if (query.addEventListener) query.addEventListener("change", applyAccessibility);
      else if (query.addListener) query.addListener(applyAccessibility);
    }

    // the text is a replacement, never a delta: redraw it whole and fade in only what changed
    function setText(next) {
      var text = typeof next === "string" ? next.replace(/^\s+/, "") : "";
      if (text === "") {
        shown = "";
        clear(body);
        body.textContent = PLACEHOLDER;
        body.classList.add("is-empty");
        body.classList.remove("clipped");
        return;
      }
      var keep = unchangedLength(shown, text);
      clear(body);
      body.classList.remove("is-empty");
      if (keep > 0) body.appendChild(document.createTextNode(text.slice(0, keep)));
      if (keep < text.length) {
        var fresh = el("span", "lv-new");
        fresh.textContent = text.slice(keep);
        body.appendChild(fresh);
      }
      shown = text;
      settle();
      requestAnimationFrame(settle); // layout may not exist yet when the first text arrives
    }

    // newest line stays in view and older ones fade at the top
    function settle() {
      body.scrollTop = body.scrollHeight;
      body.classList.toggle("clipped", !body.classList.contains("is-empty") && body.scrollHeight > body.clientHeight + 1);
    }

    function setFormatted(on) {
      tag.hidden = !on;
      root.dataset.formatted = on ? "true" : "false";
    }

    function subscribe(name, handler) {
      var listen = tauri && tauri.event && tauri.event.listen;
      if (!listen) return;
      Promise.resolve(listen(name, function (event) { handler(event.payload); })).then(function (fn) {
        if (typeof fn === "function") unlisten.push(fn);
      }).catch(function () {});
    }

    subscribe("live-reset", function () {
      setText("");
    });

    subscribe("live-text", function (p) {
      if (!p) return;
      if (typeof p.formatted === "boolean") {
        formattedSeen = true;
        setFormatted(p.formatted);
      }
      setText(p.text);
    });

    subscribe("live-place", function (p) {
      root.classList.toggle("below", !!(p && p.below));
    });

    subscribe("overlay-theme", function (p) {
      if (!p) return;
      if (typeof p.reducedMotion === "boolean") reduceFromEvent = p.reducedMotion;
      if (typeof p.solid === "boolean") solidFromEvent = p.solid;
      applyAccessibility();
    });

    applyAccessibility();
    listenQuery(reduceQuery);
    listenQuery(solidQuery);
    setFormatted(false);

    // the first events can fire before this page listens, so read the setting once
    if (tauri && tauri.core && tauri.core.invoke) {
      Promise.resolve(tauri.core.invoke("get_state")).then(function (state) {
        if (!formattedSeen && state && state.config && typeof state.config.smart_format === "boolean") setFormatted(state.config.smart_format);
      }).catch(function () {});
    }

    return {
      destroy: function () {
        unlisten.forEach(function (fn) { fn(); });
        unlisten = [];
      }
    };
  }

  window.mountLive = mountLive;
  if (!window.__LSTT_PREVIEW__) mountLive(document.body, window.__TAURI__);
})();
