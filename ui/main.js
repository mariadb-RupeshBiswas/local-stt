// Main window: History, Model, Settings. Talks to the app only through the commands and events in the UI contract.
(function () {
  "use strict";

  var SVG_NS = "http://www.w3.org/2000/svg";
  var MODIFIER_ORDER = ["Fn", "Ctrl", "Alt", "Shift", "Meta"]; // local fallback only, the app display string wins
  var MAC_KEYS = { Fn: "fn", Ctrl: "Control", Alt: "Option", Shift: "Shift", Meta: "Command" };
  var WINDOWS_KEYS = { Fn: "Fn", Ctrl: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Win" };
  var KEY_ALIASES = { fn: "Fn", ctrl: "Ctrl", control: "Ctrl", alt: "Alt", option: "Alt", shift: "Shift", cmd: "Meta", command: "Meta", win: "Meta", meta: "Meta" };
  var ARCH_LABELS = { macos: { aarch64: "Apple silicon", arm64: "Apple silicon" }, windows: { x86_64: "64-bit", x64: "64-bit", amd64: "64-bit" } };
  var DEFAULT_COMBO = {
    macos: { modifiers: ["Fn", "Shift"], key: null },
    windows: { modifiers: ["Ctrl", "Alt"], key: null }
  };
  var DEFAULT_TOGGLE = {
    macos: { modifiers: ["Fn", "Shift"], key: "Space" },
    windows: { modifiers: ["Ctrl", "Alt"], key: "Space" }
  };
  // config field and spoken name per shortcut slot
  var SLOTS = {
    hold: { field: "hotkey", name: "Push to talk" },
    toggle: { field: "toggle_hotkey", name: "Hands-free" }
  };
  var TABS = [
    { value: "history", label: "History" },
    { value: "model", label: "Model" },
    { value: "settings", label: "Settings" }
  ];
  var LANGUAGES = ("en:English|zh:Chinese|de:German|es:Spanish|ru:Russian|ko:Korean|fr:French|ja:Japanese|pt:Portuguese|tr:Turkish|" +
    "pl:Polish|ca:Catalan|nl:Dutch|ar:Arabic|sv:Swedish|it:Italian|id:Indonesian|hi:Hindi|fi:Finnish|vi:Vietnamese|he:Hebrew|" +
    "uk:Ukrainian|el:Greek|ms:Malay|cs:Czech|ro:Romanian|da:Danish|hu:Hungarian|ta:Tamil|no:Norwegian|th:Thai|ur:Urdu|hr:Croatian|" +
    "bg:Bulgarian|lt:Lithuanian|la:Latin|mi:Maori|ml:Malayalam|cy:Welsh|sk:Slovak|te:Telugu|fa:Persian|lv:Latvian|bn:Bengali|" +
    "sr:Serbian|az:Azerbaijani|sl:Slovenian|kn:Kannada|et:Estonian|mk:Macedonian|br:Breton|eu:Basque|is:Icelandic|hy:Armenian|" +
    "ne:Nepali|mn:Mongolian|bs:Bosnian|kk:Kazakh|sq:Albanian|sw:Swahili|gl:Galician|mr:Marathi|pa:Punjabi|si:Sinhala|km:Khmer|" +
    "sn:Shona|yo:Yoruba|so:Somali|af:Afrikaans|oc:Occitan|ka:Georgian|be:Belarusian|tg:Tajik|sd:Sindhi|gu:Gujarati|am:Amharic|" +
    "yi:Yiddish|lo:Lao|uz:Uzbek|fo:Faroese|ht:Haitian Creole|ps:Pashto|tk:Turkmen|nn:Nynorsk|mt:Maltese|sa:Sanskrit|" +
    "lb:Luxembourgish|my:Myanmar|bo:Tibetan|tl:Tagalog|mg:Malagasy|as:Assamese|tt:Tatar|haw:Hawaiian|ln:Lingala|ha:Hausa|" +
    "ba:Bashkir|jw:Javanese|su:Sundanese").split("|").map(function (pair) {
    var cut = pair.indexOf(":");
    return { value: pair.slice(0, cut), label: pair.slice(cut + 1) };
  }).sort(function (a, b) {
    return a.label < b.label ? -1 : a.label > b.label ? 1 : 0;
  });

  var ICONS = {
    copy: ["M11.5 9h6A2.5 2.5 0 0 1 20 11.5v6a2.5 2.5 0 0 1-2.5 2.5h-6A2.5 2.5 0 0 1 9 17.5v-6A2.5 2.5 0 0 1 11.5 9Z", "M5 15V6.5A2.5 2.5 0 0 1 7.5 4H15"],
    check: ["M5 12.5l4.5 4.5L19 7.5"],
    search: ["M11 4.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13Z", "M16 16l4 4"],
    close: ["M7 7l10 10M17 7 7 17"],
    info: ["M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17Z", "M12 11v5", "M12 8h.01"],
    trash: ["M5 7h14", "M10 7V5h4v2", "M7 7l1 12h8l1-12", "M10.5 10.5v5M13.5 10.5v5"],
    chevron: ["M9 6l6 6-6 6"]
  };

  // ---------- small DOM helpers (text always goes through textContent) ----------

  function h(tag, attrs) {
    var node = document.createElement(tag);
    var props = attrs || {};
    Object.keys(props).forEach(function (key) {
      var value = props[key];
      if (value === null || value === undefined || value === false) return;
      if (key === "class") node.className = value;
      else if (key === "text") node.textContent = value;
      else if (key.indexOf("on") === 0) node.addEventListener(key.slice(2), value);
      else if (key === "hidden" || key === "disabled") node[key] = true;
      else node.setAttribute(key, value === true ? "" : String(value));
    });
    for (var i = 2; i < arguments.length; i++) append(node, arguments[i]);
    return node;
  }

  function append(parent, child) {
    if (child === null || child === undefined || child === false) return;
    if (Array.isArray(child)) child.forEach(function (c) { append(parent, c); });
    else if (typeof child === "string") parent.appendChild(document.createTextNode(child));
    else parent.appendChild(child);
  }

  function icon(name) {
    var svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("class", "ic ic-" + name);
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    ICONS[name].forEach(function (d) {
      var path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", d);
      svg.appendChild(path);
    });
    return svg;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  // ---------- formatting ----------

  function dash(value) {
    if (value === null || value === undefined || value === "") return "-";
    if (typeof value === "number" && !isFinite(value)) return "-";
    return String(value);
  }

  function errText(err) {
    if (typeof err === "string" && err) return err;
    if (err && typeof err.message === "string" && err.message) return err.message;
    return "Something went wrong.";
  }

  function mb(bytes) {
    return typeof bytes === "number" && isFinite(bytes) ? Math.round(bytes / 1048576) + " MB" : "-";
  }

  function keyName(modifier, platform) {
    return (platform === "windows" ? WINDOWS_KEYS : MAC_KEYS)[modifier];
  }

  function comboParts(combo, platform) {
    if (!combo || !Array.isArray(combo.modifiers)) return [];
    var parts = MODIFIER_ORDER.filter(function (m) { return combo.modifiers.indexOf(m) !== -1; }).map(function (m) { return keyName(m, platform); });
    if (combo.key) parts.push(combo.key);
    return parts;
  }

  // the app sends text like "Ctrl + Alt"; rename its modifiers to this platform's keycap names
  function displayParts(display, platform) {
    return display.split(" + ").map(function (token) {
      var modifier = KEY_ALIASES[token.toLowerCase()];
      return modifier ? keyName(modifier, platform) : token;
    });
  }

  function sameCombo(a, b) {
    return JSON.stringify(comboParts(a, "macos")) === JSON.stringify(comboParts(b, "macos"));
  }

  function systemLabel(hw, platform) {
    if (!hw.os && !hw.arch) return "-";
    var arch = hw.arch ? (ARCH_LABELS[platform] || {})[String(hw.arch).toLowerCase()] || hw.arch : null;
    return [hw.os, arch].filter(Boolean).join(", ");
  }

  // Home and End jump to the ends, arrows wrap; -1 means the key is not navigation
  function stepTarget(event, current, count) {
    if (event.key === "Home") return 0;
    if (event.key === "End") return count - 1;
    var step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    return step ? (current + step + count) % count : -1;
  }

  function dayKey(date) {
    return date.getFullYear() + "-" + date.getMonth() + "-" + date.getDate();
  }

  function dayLabel(date) {
    var today = new Date();
    var yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
    if (dayKey(date) === dayKey(today)) return "Today";
    if (dayKey(date) === dayKey(yesterday)) return "Yesterday";
    return date.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: date.getFullYear() === today.getFullYear() ? undefined : "numeric" });
  }

  // ---------- reusable controls ----------

  function keycaps(parts) {
    var wrap = h("span", { class: "keys" });
    parts.forEach(function (part) { wrap.appendChild(h("kbd", { class: "key", text: part })); });
    return wrap;
  }

  function segmented(opts) {
    var isTabs = opts.role === "tablist";
    var root = h("div", { class: "seg init", role: opts.role, "aria-label": opts.label });
    root.style.setProperty("--n", String(opts.items.length));
    root.appendChild(h("span", { class: "seg-thumb", "aria-hidden": "true" }));

    var buttons = opts.items.map(function (item, index) {
      var button = h("button", {
        class: "seg-item",
        type: "button",
        role: isTabs ? "tab" : "radio",
        id: item.id,
        "aria-controls": item.controls,
        text: item.label,
        onclick: function () { choose(index, false); }
      });
      root.appendChild(button);
      return button;
    });

    function currentIndex() {
      var value = opts.value();
      for (var i = 0; i < opts.items.length; i++) if (opts.items[i].value === value) return i;
      return 0;
    }

    function choose(index, focus) {
      if (focus) buttons[index].focus();
      if (opts.items[index].value !== opts.value()) opts.onChange(opts.items[index].value);
    }

    function sync() {
      var current = currentIndex();
      root.style.setProperty("--i", String(current));
      buttons.forEach(function (button, i) {
        button.setAttribute(isTabs ? "aria-selected" : "aria-checked", i === current ? "true" : "false");
        button.tabIndex = i === current ? 0 : -1;
        if (opts.disabled) button.disabled = opts.disabled();
      });
      root.classList.toggle("is-disabled", !!(opts.disabled && opts.disabled()));
    }

    root.addEventListener("keydown", function (event) {
      var next = stepTarget(event, currentIndex(), buttons.length);
      if (next === -1) return;
      event.preventDefault();
      choose(next, true);
    });

    sync();
    requestAnimationFrame(function () { root.classList.remove("init"); });
    return { el: root, sync: sync };
  }

  function toggle(opts) {
    var button = h("button", { class: "sw", type: "button", role: "switch", "aria-label": opts.label }, h("span", { class: "sw-thumb" }));
    button.addEventListener("click", function () {
      if (opts.disabled && opts.disabled()) return;
      opts.onChange(!opts.value());
    });
    function sync() {
      button.setAttribute("aria-checked", opts.value() ? "true" : "false");
      var off = !!(opts.disabled && opts.disabled());
      button.disabled = off;
    }
    sync();
    return { el: button, sync: sync };
  }

  function select(opts) {
    var node = h("select", { class: "select", "aria-label": opts.label });
    node.addEventListener("change", function () { opts.onChange(node.value); });
    function sync() {
      clear(node);
      var current = opts.value();
      var options = opts.options();
      var known = options.some(function (o) { return o.value === current; });
      if (!known) options = options.concat([{ value: current, label: current + " (not connected)" }]);
      options.forEach(function (o) {
        var option = h("option", { value: o.value, text: o.label });
        node.appendChild(option);
      });
      node.value = current;
    }
    sync();
    return { el: node, sync: sync };
  }

  // ---------- the window ----------

  function mountMain(root, tauri) {
    var invoke = tauri && tauri.core && tauri.core.invoke ? tauri.core.invoke : function () { return Promise.reject("The app is not connected."); };
    var listen = tauri && tauri.event && tauri.event.listen ? tauri.event.listen : null;

    var S = {
      loaded: false,
      loadError: "",
      config: null,
      models: [],
      hardware: null,
      inputs: [],
      version: "",
      platform: "macos",
      display: { hold: "", toggle: "" }, // the app's text for each shortcut
      displayFor: { hold: "", toggle: "" }, // JSON of the combo each display describes
      installed: false,
      tab: "history",
      history: [],
      historyLoaded: false,
      historyStale: false,
      search: "",
      progress: {},
      modelErrors: {},
      activeModel: null, // the model actually loaded, which config.model may not be
      downloading: null,
      loadingModel: "",
      autostartEnabled: null,
      moving: false,
      capturingSlot: null, // "hold", "toggle" or null: one capture at a time
      hotkeyErrors: { hold: "", toggle: "" },
      installing: false
    };
    var syncers = []; // settings controls, refreshed after every config change
    var progressRefs = {}; // model id -> { bar, text, track }
    var unlisten = [];
    var toastTimer = 0;

    root.classList.add("mw");
    root.dataset.platform = "macos";

    var reduceQuery = window.matchMedia ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;
    function applyReduce() {
      root.classList.toggle("reduce-motion", !!(reduceQuery && reduceQuery.matches) || root.dataset.forceReduce === "1");
    }
    applyReduce();
    if (reduceQuery && reduceQuery.addEventListener) reduceQuery.addEventListener("change", applyReduce);

    // ---------- skeleton ----------

    var panels = {};
    var tabs = segmented({
      role: "tablist",
      label: "Sections",
      items: TABS.map(function (t) { return { value: t.value, label: t.label, id: "tab-" + t.value, controls: "panel-" + t.value }; }),
      value: function () { return S.tab; },
      onChange: function (value) { showTab(value); }
    });

    tabs.el.classList.add("toolbar-seg");
    var toolbar = h("header", { class: "mw-toolbar" }, tabs.el);
    var scroller = h("main", { class: "mw-scroll" });
    TABS.forEach(function (t) {
      panels[t.value] = h("section", { class: "panel", id: "panel-" + t.value, role: "tabpanel", "aria-labelledby": "tab-" + t.value, hidden: t.value !== S.tab });
      scroller.appendChild(panels[t.value]);
    });
    var toastHost = h("div", { class: "toast-host", role: "status", "aria-live": "polite" });
    var announcer = h("div", { class: "sr-only", role: "status", "aria-live": "polite" });
    var dialog = h("dialog", { class: "dlg", "aria-labelledby": "dlg-title" });

    root.appendChild(toolbar); // first in the DOM so the tabs lead the Tab order, CSS keeps them on top
    root.appendChild(scroller);
    root.appendChild(toastHost);
    root.appendChild(announcer);
    root.appendChild(dialog);

    scroller.addEventListener("scroll", function () {
      toolbar.classList.toggle("scrolled", scroller.scrollTop > 2);
    });

    function showTab(name) {
      if (name === S.tab) return;
      if (name !== "settings") stopMoving();
      S.tab = name;
      TABS.forEach(function (t) { panels[t.value].hidden = t.value !== name; });
      tabs.sync();
      scroller.scrollTop = 0;
      if (name === "history" && S.historyStale) loadHistory();
    }

    // ---------- keyboard shortcuts ----------

    // Cmd on macOS, Ctrl on Windows: 1, 2, 3 pick a tab, F searches the history, W closes the window
    function onShortcut(event) {
      var mod = S.platform === "windows" ? event.ctrlKey : event.metaKey;
      if (!mod || event.altKey || event.shiftKey || S.capturingSlot || dialog.open) return;
      var tab = { "1": "history", "2": "model", "3": "settings" }[event.key];
      var win = tauri && tauri.window && tauri.window.getCurrentWindow ? tauri.window.getCurrentWindow() : null;
      if ((event.key === "w" || event.key === "W") && win) {
        event.preventDefault();
        stopMoving();
        win.close();
      } else if (tab) {
        event.preventDefault();
        showTab(tab);
      } else if (event.key === "f" || event.key === "F") {
        event.preventDefault();
        showTab("history");
        searchInput.focus();
        searchInput.select();
      }
    }

    function onHidden() {
      if (document.visibilityState === "hidden") stopMoving();
    }

    document.addEventListener("keydown", onShortcut);
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", stopMoving);

    // ---------- toast and screen reader status ----------

    function announce(message) {
      announcer.textContent = "";
      setTimeout(function () { announcer.textContent = message; }, 60);
    }

    function toast(message) {
      clearTimeout(toastTimer);
      clear(toastHost);
      var node = h("div", { class: "toast", text: message });
      toastHost.appendChild(node);
      void node.offsetWidth;
      node.classList.add("in");
      toastTimer = setTimeout(function () {
        node.classList.remove("in");
        setTimeout(function () { if (node.parentNode) node.parentNode.removeChild(node); }, 200);
      }, Math.min(9000, Math.max(4200, message.length * 55))); // install notes run long, give them reading time
    }

    // ---------- data ----------

    function applyState(state) {
      S.config = state.config;
      S.models = Array.isArray(state.models) ? state.models : [];
      S.hardware = state.hardware || null;
      S.inputs = Array.isArray(state.inputs) ? state.inputs : [];
      S.version = state.version || "";
      S.platform = state.platform === "windows" ? "windows" : "macos";
      S.display.hold = state.hotkeyDisplay || "";
      S.display.toggle = state.toggleDisplay || "";
      S.displayFor.hold = JSON.stringify(slotCombo("hold"));
      S.displayFor.toggle = JSON.stringify(slotCombo("toggle"));
      S.installed = state.installed === true;
      S.activeModel = typeof state.activeModel === "string" ? state.activeModel : null;
      S.downloading = typeof state.downloading === "string" ? state.downloading : null;
      S.autostartEnabled = typeof state.autostartEnabled === "boolean" ? state.autostartEnabled : null;
      if (S.downloading && !S.progress[S.downloading]) S.progress[S.downloading] = { downloaded: null, total: null };
      if (S.loadingModel && S.loadingModel === S.activeModel) S.loadingModel = "";
      S.loaded = true;
      S.loadError = "";
      root.dataset.platform = S.platform;
    }

    function refreshState() {
      return Promise.resolve(invoke("get_state")).then(function (state) {
        applyState(state);
        renderAll();
      }).catch(function (err) {
        if (!S.loaded) {
          S.loadError = errText(err);
          renderAll();
        } else {
          toast(errText(err));
        }
      });
    }

    function loadHistory() {
      S.historyStale = false;
      return Promise.resolve(invoke("get_history")).then(function (entries) {
        S.history = Array.isArray(entries) ? entries : [];
        S.historyLoaded = true;
        renderHistoryList();
      }).catch(function (err) {
        toast(errText(err));
      });
    }

    // re-read the app's state and repaint in place, so a focused control keeps focus
    function reloadState() {
      return Promise.resolve(invoke("get_state")).then(function (state) {
        applyState(state);
        syncSettings();
        renderHistoryEmptyHint();
      }).catch(function () {});
    }

    function displaysStale() {
      return Object.keys(SLOTS).some(function (slot) {
        return S.displayFor[slot] !== JSON.stringify(slotCombo(slot));
      });
    }

    function patchConfig(change) {
      var before = S.config;
      S.config = Object.assign({}, S.config, change);
      syncSettings();
      renderHistoryEmptyHint();
      return Promise.resolve(invoke("set_config", { patch: change })).then(function (saved) {
        if (saved && typeof saved === "object") S.config = saved;
        if (change.autostart !== undefined) S.autostartEnabled = !!S.config.autostart;
        syncSettings();
        renderHistoryEmptyHint();
        // turning Start at login on installs the app first, and a new shortcut needs the app's own text
        if (change.autostart !== undefined || displaysStale()) return reloadState();
      }).catch(function (err) {
        S.config = before;
        syncSettings();
        renderHistoryEmptyHint();
        toast("Couldn't save: " + errText(err));
      });
    }

    function slotCombo(slot) {
      return S.config ? S.config[SLOTS[slot].field] || null : null;
    }

    // keycap names for a slot: the app's own text while it matches the config, else built locally
    function slotParts(slot) {
      var combo = slotCombo(slot);
      var fresh = S.display[slot] && S.displayFor[slot] === JSON.stringify(combo);
      return fresh ? displayParts(S.display[slot], S.platform) : comboParts(combo, S.platform);
    }

    // ---------- History ----------

    var historyPanel = panels.history;
    var searchInput = h("input", {
      class: "search-input",
      type: "text",
      placeholder: "Search dictations",
      "aria-label": "Search dictations",
      autocomplete: "off",
      spellcheck: "false"
    });
    var searchClear = h("button", { class: "icon-btn search-clear", type: "button", "aria-label": "Clear search", hidden: true, onclick: function () {
      searchInput.value = "";
      onSearch();
      searchInput.focus();
    } }, icon("close"));
    var clearButton = h("button", { class: "btn btn-danger", type: "button", text: "Clear history", onclick: askClearHistory });
    var historyList = h("div", { class: "history-list" });
    var historyBar = h("div", { class: "history-bar" },
      h("div", { class: "search" }, icon("search"), searchInput, searchClear),
      clearButton
    );
    historyPanel.appendChild(historyBar);
    historyPanel.appendChild(historyList);

    function onSearch() {
      S.search = searchInput.value;
      searchClear.hidden = S.search === "";
      renderHistoryList();
    }
    searchInput.addEventListener("input", onSearch);

    function renderHistoryEmptyHint() {
      if (S.historyLoaded && S.history.length === 0) renderHistoryList();
    }

    function emptyState(title, body, extra) {
      return h("div", { class: "empty" }, h("div", { class: "empty-title", text: title }), h("div", { class: "empty-body" }, body), extra);
    }

    function historyRow(entry) {
      var text = typeof entry.text === "string" ? entry.text : "";
      var date = typeof entry.ts_ms === "number" ? new Date(entry.ts_ms) : null;
      var time = date ? date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }) : "-";
      var seconds = typeof entry.duration_ms === "number" ? (entry.duration_ms / 1000).toFixed(1) + " s" : "-";
      var failed = entry.ok === false;

      var body = h("div", { class: "hrow-text" + (text ? "" : " is-empty"), text: text || "Nothing was transcribed." });
      var moreDot = h("span", { class: "dot-sep", "aria-hidden": "true", hidden: true });
      var more = h("button", { class: "linklike", type: "button", text: "Show more", hidden: true, onclick: function () {
        var open = body.classList.toggle("open");
        more.textContent = open ? "Show less" : "Show more";
      } });
      var snippet = text.length > 40 ? text.slice(0, 40).trim() + "..." : text;
      var copyLabel = text === "" ? "Copy dictation (empty)" : "Copy dictation: " + snippet;
      var copy = h("button", { class: "icon-btn copy", type: "button", "aria-label": copyLabel, disabled: text === "" }, icon("copy"));
      var copyTimer = 0;
      copy.addEventListener("click", function () {
        Promise.resolve(invoke("copy_text", { text: text })).then(function () {
          clearTimeout(copyTimer);
          clear(copy);
          copy.appendChild(icon("check"));
          copy.classList.add("done");
          copy.setAttribute("aria-label", "Copied");
          announce("Copied to the clipboard.");
          copyTimer = setTimeout(function () {
            clear(copy);
            copy.appendChild(icon("copy"));
            copy.classList.remove("done");
            copy.setAttribute("aria-label", copyLabel);
          }, 1000);
        }).catch(function (err) { toast("Couldn't copy: " + errText(err)); });
      });

      var row = h("div", { class: "hrow" + (failed ? " failed" : "") },
        h("div", { class: "hrow-main" },
          body,
          h("div", { class: "hrow-meta" },
            h("span", { text: time }),
            h("span", { class: "dot-sep", "aria-hidden": "true" }),
            h("span", { text: seconds }),
            h("span", { class: "dot-sep", "aria-hidden": "true" }),
            h("span", { text: dash(entry.model) }),
            failed ? h("span", { class: "tag tag-red", text: "Failed" }) : null,
            moreDot,
            more
          )
        ),
        h("div", { class: "hrow-actions" }, copy)
      );
      row._body = body;
      row._more = more;
      row._moreDot = moreDot;
      return row;
    }

    function renderHistoryList() {
      clear(historyList);
      var needle = S.search.trim().toLowerCase();
      var entries = S.history.filter(function (e) {
        return needle === "" || String(e.text || "").toLowerCase().indexOf(needle) !== -1;
      });
      clearButton.disabled = S.history.length === 0;

      if (!S.historyLoaded) return;

      if (S.history.length === 0) {
        if (S.config && S.config.save_history === false) {
          historyList.appendChild(emptyState("History is off", "Turn on Save history in Settings to keep your dictations here.",
            h("button", { class: "btn", type: "button", text: "Open Settings", onclick: function () { showTab("settings"); } })));
        } else {
          var pushKeys = slotParts("hold");
          var freeKeys = slotParts("toggle");
          historyList.appendChild(emptyState("No dictations yet", [
            "Hold ", pushKeys.length ? keycaps(pushKeys) : "your shortcut", " and speak",
            freeKeys.length ? [", or press ", keycaps(freeKeys), " for hands-free"] : null, // hands-free can be off
            ". Your text appears here."
          ]));
        }
        return;
      }

      if (entries.length === 0) {
        historyList.appendChild(emptyState("No results", "Nothing matches your search."));
        return;
      }

      var rows = [];
      var group = null;
      var card = null;
      entries.forEach(function (entry) {
        var date = typeof entry.ts_ms === "number" ? new Date(entry.ts_ms) : null;
        var key = date ? dayKey(date) : "unknown";
        if (key !== group) {
          group = key;
          historyList.appendChild(h("h3", { class: "group-title", text: date ? dayLabel(date) : "-" }));
          card = h("div", { class: "card" });
          historyList.appendChild(card);
        }
        var row = historyRow(entry);
        rows.push(row);
        card.appendChild(row);
      });

      // reveal "Show more" only where the clamp actually hides text
      requestAnimationFrame(function () {
        rows.forEach(function (row) {
          if (row._body.scrollHeight > row._body.clientHeight + 1) {
            row._more.hidden = false;
            row._moreDot.hidden = false;
          }
        });
      });
    }

    // one confirmation dialog for every action that deletes or downloads something
    function askConfirm(opts) {
      clear(dialog);
      dialog.appendChild(h("h2", { class: "dlg-title", id: "dlg-title", text: opts.title }));
      dialog.appendChild(h("p", { class: "dlg-body", text: opts.body }));
      var cancel = h("button", { class: "btn", type: "button", text: "Cancel", onclick: function () { closeDialog(); } });
      var confirm = h("button", { class: "btn " + (opts.danger ? "btn-danger-solid" : "btn-action"), type: "button", text: opts.confirm, onclick: function () {
        closeDialog();
        opts.onConfirm();
      } });
      dialog.appendChild(h("div", { class: "dlg-actions" }, cancel, confirm));
      if (typeof dialog.showModal === "function") dialog.showModal();
      else dialog.setAttribute("open", "");
      (opts.danger ? cancel : confirm).focus();
    }

    function thisDevice() {
      return S.platform === "windows" ? "this PC" : "this Mac";
    }

    function askClearHistory() {
      askConfirm({
        title: "Clear all history?",
        body: "This deletes every saved dictation from " + thisDevice() + ". It can't be undone.",
        confirm: "Clear history",
        danger: true,
        onConfirm: function () {
          Promise.resolve(invoke("clear_history")).then(function () {
            S.history = [];
            renderHistoryList();
          }).catch(function (err) { toast("Couldn't clear history: " + errText(err)); });
        }
      });
    }

    function askTurnOffHistory() {
      askConfirm({
        title: "Turn off history?",
        body: "Saved dictations will be deleted from " + thisDevice() + ".",
        confirm: "Turn off",
        danger: true,
        onConfirm: function () {
          patchConfig({ save_history: false }).then(function () {
            if (S.config && S.config.save_history === false) {
              S.history = [];
              renderHistoryList();
            }
          });
        }
      });
    }

    function closeDialog() {
      if (typeof dialog.close === "function") dialog.close();
      else dialog.removeAttribute("open");
    }

    // ---------- Model ----------

    var modelPanel = panels.model;

    function hardwareCard() {
      var hw = S.hardware || {};
      var system = systemLabel(hw, S.platform);
      var accel = hw.gpu_accel === true ? "Used for dictation" : hw.gpu_accel === false ? "Not used for dictation" : null;
      var items = [
        ["System", system, null],
        ["Processor", dash(hw.cpu), null],
        ["CPU threads", dash(hw.threads), null],
        ["Memory", hw.ram_gb === null || hw.ram_gb === undefined ? "-" : hw.ram_gb + " GB", null],
        ["Graphics", dash(hw.gpu), accel],
        ["Free disk", hw.free_disk_gb === null || hw.free_disk_gb === undefined ? "-" : hw.free_disk_gb + " GB", null]
      ];
      var grid = h("dl", { class: "hw-grid" });
      items.forEach(function (item) {
        grid.appendChild(h("div", { class: "hw-item" },
          h("dt", { text: item[0] }),
          h("dd", {}, item[1], item[2] ? h("span", { class: "hw-sub", text: item[2] }) : null)
        ));
      });
      return h("div", { class: "card hw" }, grid);
    }

    function sizeText(fit) {
      return typeof fit.size_mb === "number" ? fit.size_mb + " MB" : null;
    }

    function modelAction(fit, active, downloading, loading) {
      if (downloading) return h("span", { class: "pill pill-busy", text: "Downloading" });
      if (loading) return h("span", { class: "pill pill-busy", text: "Loading" });
      if (active) return h("span", { class: "pill pill-active" }, icon("check"), "Active");
      if (!fit.supported) return null;
      var size = sizeText(fit);
      var label = fit.downloaded ? "Use" : size ? "Download " + size : "Download";
      return h("button", {
        class: "btn btn-action",
        type: "button",
        text: label,
        "aria-label": (fit.downloaded ? "Use " : "Download ") + dash(fit.label) + (fit.downloaded || !size ? "" : ", " + size),
        onclick: function () { requestModel(fit); }
      });
    }

    function modelRow(fit) {
      var active = S.activeModel === fit.id;
      var loading = S.loadingModel === fit.id;
      var progress = S.progress[fit.id];
      var disabled = !fit.supported;
      var reasons = Array.isArray(fit.reasons) ? fit.reasons : [];

      var badges = h("span", { class: "badges" },
        fit.recommended ? h("span", { class: "tag tag-blue", text: "Recommended" }) : null,
        fit.downloaded ? h("span", { class: "tag tag-green", text: "Downloaded" }) : null,
        disabled ? h("span", { class: "tag tag-grey", text: "Not supported" }) : null
      );

      var detail = null;
      if (progress) {
        var track = h("div", { class: "track", role: "progressbar", "aria-label": "Download progress", "aria-valuemin": "0", "aria-valuemax": "100" }, h("i"));
        var line = h("div", { class: "progress-text" });
        progressRefs[fit.id] = { track: track, bar: track.firstChild, text: line };
        paintProgress(fit.id);
        detail = h("div", { class: "progress" }, track, line);
      } else {
        delete progressRefs[fit.id];
      }

      var error = S.modelErrors[fit.id] ? h("div", { class: "model-error", role: "alert", text: S.modelErrors[fit.id] }) : null;
      var why = disabled && reasons.length
        ? h("ul", { class: "reasons" }, reasons.map(function (r) { return h("li", {}, icon("info"), h("span", { text: r })); }))
        : null;

      return h("div", {
        class: "mrow" + (disabled ? " is-disabled" : "") + (active ? " is-active" : ""),
        role: "listitem"
      },
        h("div", { class: "mrow-head" },
          h("div", { class: "mrow-title" },
            h("span", { class: "mrow-name", text: dash(fit.label) }),
            h("span", { class: "mrow-size", text: sizeText(fit) || "-" }),
            badges
          ),
          modelAction(fit, active, !!progress, loading)
        ),
        why, detail, error
      );
    }

    function renderModels() {
      clear(modelPanel);
      progressRefs = {};
      if (!S.loaded) return;
      modelPanel.appendChild(h("h3", { class: "group-title", text: "This computer" }));
      modelPanel.appendChild(hardwareCard());
      modelPanel.appendChild(h("h3", { class: "group-title", text: "Speech model" }));
      var list = h("div", { class: "card", role: "list", "aria-label": "Speech models" });
      S.models.forEach(function (fit) { list.appendChild(modelRow(fit)); });
      modelPanel.appendChild(list);
      modelPanel.appendChild(h("p", { class: "footnote", text: "Models run on this computer. A download is checked against a fixed checksum before it is used." }));
    }

    function paintProgress(id) {
      var ref = progressRefs[id];
      var info = S.progress[id];
      if (!ref || !info) return;
      var known = typeof info.total === "number" && info.total > 0 && typeof info.downloaded === "number";
      var fraction = known ? Math.min(1, info.downloaded / info.total) : 0;
      ref.bar.style.transform = "scaleX(" + fraction.toFixed(4) + ")";
      ref.track.classList.toggle("indeterminate", !known);
      if (known) ref.track.setAttribute("aria-valuenow", String(Math.round(fraction * 100)));
      else ref.track.removeAttribute("aria-valuenow");
      ref.text.textContent = known
        ? mb(info.downloaded) + " of " + mb(info.total) + "  (" + Math.round(fraction * 100) + "%)"
        : "Starting download...";
    }

    // a model that is not on disk costs a big download, so say how big before starting it
    function requestModel(fit) {
      if (fit.downloaded) {
        chooseModel(fit);
        return;
      }
      var size = sizeText(fit);
      askConfirm({
        title: "Download " + dash(fit.label) + "?",
        body: (size ? size + " will be downloaded" : "It will be downloaded") + ", checked against a fixed checksum, and kept on " + thisDevice() + " for quick switching.",
        confirm: size ? "Download " + size : "Download",
        onConfirm: function () { chooseModel(fit); }
      });
    }

    function chooseModel(fit) {
      if (!fit.supported || S.progress[fit.id] || S.loadingModel === fit.id || S.activeModel === fit.id) return;
      delete S.modelErrors[fit.id];
      if (fit.downloaded) S.loadingModel = fit.id;
      else S.progress[fit.id] = { downloaded: null, total: null };
      renderModels();
      Promise.resolve(invoke("choose_model", { id: fit.id })).catch(function (err) {
        delete S.progress[fit.id];
        if (S.loadingModel === fit.id) S.loadingModel = "";
        S.modelErrors[fit.id] = errText(err);
        renderModels();
      });
    }

    // ---------- Settings ----------

    var settingsPanel = panels.settings;

    function syncSettings() {
      syncers.forEach(function (fn) { fn(); });
    }

    function settingRow(title, description, control, extra) {
      return h("div", { class: "srow" },
        h("div", { class: "srow-text" },
          h("div", { class: "srow-title", text: title }),
          description ? h("div", { class: "srow-desc", text: description }) : null
        ),
        h("div", { class: "srow-control" }, control),
        extra || null
      );
    }

    function card(rows) {
      return h("div", { class: "card" }, rows);
    }

    function addSync(control) {
      syncers.push(control.sync);
      return control.el;
    }

    function startCapture(slot) {
      S.capturingSlot = slot;
      S.hotkeyErrors = { hold: "", toggle: "" };
      syncSettings();
      announce("Press your shortcut now. Escape cancels.");
      Promise.resolve(invoke("start_hotkey_capture", { slot: slot })).catch(function (err) {
        if (S.capturingSlot === slot) S.capturingSlot = null;
        S.hotkeyErrors[slot] = errText(err);
        syncSettings();
      });
    }

    function stopCapture() {
      if (!S.capturingSlot) return;
      S.capturingSlot = null;
      syncSettings();
      Promise.resolve(invoke("cancel_hotkey_capture")).catch(function () {});
    }

    // one shortcut field with its own Reset; hands-free also gets Turn off
    function hotkeyControl(slot) {
      var info = SLOTS[slot];
      var defaults = slot === "hold" ? DEFAULT_COMBO : DEFAULT_TOGGLE;
      var field = h("button", { class: "hk", type: "button", "data-slot": slot });
      var reset = h("button", { class: "btn btn-quiet", type: "button", text: "Reset", "aria-label": "Reset " + info.name + " shortcut", onclick: function () {
        var change = {};
        change[info.field] = defaults[S.platform];
        patchConfig(change);
      } });
      var turnOff = slot === "toggle" ? h("button", { class: "btn btn-quiet", type: "button", text: "Turn off", "aria-label": "Turn off hands-free", onclick: function () {
        patchConfig({ toggle_hotkey: null });
      } }) : null;
      var error = h("div", { class: "field-error", role: "alert", hidden: true });

      field.addEventListener("click", function () {
        if (S.capturingSlot === slot) {
          stopCapture();
        } else {
          field.focus(); // WebKit does not focus a button on click, and blur is how capture ends
          startCapture(slot);
        }
      });
      field.addEventListener("blur", function () {
        if (S.capturingSlot === slot) stopCapture(); // another slot's capture is not ours to end
      });
      field.addEventListener("keydown", function (event) {
        if (S.capturingSlot !== slot) return;
        event.preventDefault(); // keys belong to the OS hook while capturing
        if (event.key === "Escape") stopCapture();
      });

      var drawn = null; // what the field shows now; redrawing an unchanged field would eat a click already under way
      function sync() {
        var capturing = S.capturingSlot === slot;
        var parts = slotParts(slot);
        var off = !slotCombo(slot);
        var shown = capturing ? "capturing" : parts.join("+") + (off ? "off" : "");
        field.classList.toggle("capturing", capturing);
        if (capturing) field.setAttribute("aria-label", info.name + " shortcut: press your shortcut now.");
        else if (off) field.setAttribute("aria-label", info.name + " shortcut: off. Click to turn on.");
        else field.setAttribute("aria-label", info.name + " shortcut: " + parts.join(" plus ") + ". Click to change.");
        if (shown !== drawn) {
          drawn = shown;
          clear(field);
          if (capturing) field.appendChild(h("span", { class: "hk-prompt" }, h("i", { class: "hk-pulse" }), "Press your shortcut..."));
          else if (parts.length) field.appendChild(keycaps(parts));
          else field.appendChild(h("span", { class: "hk-prompt", text: slot === "toggle" ? "Off" : "-" }));
        }
        error.hidden = S.hotkeyErrors[slot] === "";
        error.textContent = S.hotkeyErrors[slot];
        reset.hidden = capturing || !S.config || sameCombo(slotCombo(slot), defaults[S.platform]);
        if (turnOff) turnOff.hidden = capturing || off;
      }
      sync();
      return { el: h("div", { class: "hk-wrap" }, h("div", { class: "hk-line" }, field, reset, turnOff), error), sync: sync };
    }

    // wording and button follow the app's installed flag and the platform's launcher names
    function installRow() {
      var desc = h("div", { class: "srow-desc" });
      var button = h("button", { class: "btn", type: "button", onclick: function () {
        if (S.installed || S.installing) return;
        S.installing = true;
        syncSettings();
        Promise.resolve(invoke("install_app")).then(function (sentence) {
          toast(typeof sentence === "string" && sentence ? sentence : "Installed.");
          return reloadState();
        }).catch(function (err) {
          toast("Couldn't install: " + errText(err));
        }).then(function () {
          S.installing = false;
          syncSettings();
        });
      } });
      function sync() {
        var windows = S.platform === "windows";
        var where = windows ? "your Start menu" : "your Applications folder";
        var finder = windows ? "Windows search" : "Spotlight";
        desc.textContent = S.installed
          ? "local-stt is in " + where + " (" + finder + " finds it) and the local-stt terminal command is set up."
          : "Add local-stt to " + (windows ? "the Start menu so Windows search finds it" : "Applications so Spotlight finds it") + ", and add the local-stt terminal command.";
        clear(button);
        button.classList.toggle("btn-done", S.installed);
        button.disabled = S.installed || S.installing;
        append(button, S.installed ? [icon("check"), "Installed"] : S.installing ? "Installing..." : "Install");
      }
      syncers.push(sync);
      sync();
      return h("div", { class: "srow" },
        h("div", { class: "srow-text" }, h("div", { class: "srow-title", text: "Install" }), desc),
        h("div", { class: "srow-control" }, button)
      );
    }

    function themeControl() {
      var cards = [
        { value: "pill", name: "Pill", note: "Level bars" },
        { value: "waveform", name: "Waveform", note: "Last 2 seconds" },
        { value: "minimal", name: "Minimal", note: "Dot and ring" }
      ];
      var group = h("div", { class: "themes", role: "radiogroup", "aria-label": "Overlay theme" });
      var buttons = cards.map(function (c) {
        var stage = h("div", { class: "theme-stage" }, miniOverlay(c.value));
        var button = h("button", { class: "theme-card", type: "button", role: "radio" },
          stage,
          h("span", { class: "theme-name", text: c.name }),
          h("span", { class: "theme-note", text: c.note })
        );
        button.addEventListener("click", function () {
          if (S.config.theme !== c.value) patchConfig({ theme: c.value });
        });
        group.appendChild(button);
        return button;
      });
      function sync() {
        buttons.forEach(function (button, i) {
          var on = S.config && S.config.theme === cards[i].value;
          button.setAttribute("aria-checked", on ? "true" : "false");
          button.tabIndex = on ? 0 : -1;
          button.disabled = !!(S.config && S.config.show_overlay === false);
        });
      }
      group.addEventListener("keydown", function (event) {
        var current = cards.findIndex(function (c) { return c.value === S.config.theme; });
        var next = stepTarget(event, current, cards.length);
        if (next === -1) return;
        event.preventDefault();
        buttons[next].focus();
        patchConfig({ theme: cards[next].value });
      });
      sync();
      return { el: group, sync: sync };
    }

    function setMoving(on) {
      if (S.moving === on) return;
      S.moving = on;
      syncSettings();
      announce(on ? "The pill is on screen. Drag it where you like." : "Pill position saved.");
      Promise.resolve(invoke("move_overlay", { on: on })).catch(function (err) {
        S.moving = !on;
        syncSettings();
        toast(errText(err));
      });
    }

    function stopMoving() {
      setMoving(false);
    }

    function pillPositionControl() {
      var move = h("button", { class: "btn", type: "button", onclick: function () { setMoving(!S.moving); } });
      var reset = h("button", { class: "btn", type: "button", text: "Reset position", onclick: function () {
        Promise.resolve(invoke("reset_overlay_position")).then(function () {
          toast("Position reset.");
          if (!S.moving) return null;
          // the app places only a hidden pill, so hide it and show it again
          return Promise.resolve(invoke("move_overlay", { on: false })).then(function () {
            return new Promise(function (done) { setTimeout(done, 200); });
          }).then(function () {
            if (S.moving) return invoke("move_overlay", { on: true });
          });
        }).catch(function (err) { toast(errText(err)); });
      } });
      function sync() { move.textContent = S.moving ? "Done" : "Move pill"; }
      sync();
      return { el: h("div", { class: "btn-row" }, move, reset), sync: sync };
    }

    // tiny static drawing of each overlay theme, built from plain elements
    function miniOverlay(theme) {
      var capsule = h("div", { class: "mini mini-" + theme }, h("span", { class: "mini-dot" }));
      if (theme === "minimal") {
        capsule.insertBefore(h("span", { class: "mini-ring" }), capsule.firstChild);
        return capsule;
      }
      var heights = theme === "pill" ? [5, 8, 12, 15, 12, 8, 5] : [4, 6, 9, 5, 12, 15, 8, 5, 10, 13, 7, 5, 9, 14, 11, 6];
      var bars = h("span", { class: "mini-bars" });
      heights.forEach(function (px) {
        var bar = h("i");
        bar.style.height = px + "px";
        bars.appendChild(bar);
      });
      capsule.appendChild(h("span", { class: "mini-label" }));
      capsule.appendChild(bars);
      return capsule;
    }

    function buildSettings() {
      clear(settingsPanel);
      syncers = [];
      if (!S.loaded) return;
      var cfg = function () { return S.config; };
      var langOptions = function () { return [{ value: "auto", label: "Auto-detect" }].concat(LANGUAGES); };
      var micOptions = function () {
        return [{ value: "", label: "System default" }].concat(S.inputs.map(function (name) { return { value: name, label: name }; }));
      };

      var pushKey = hotkeyControl("hold");
      var freeKey = hotkeyControl("toggle");
      syncers.push(pushKey.sync, freeKey.sync);

      var output = segmented({
        role: "radiogroup", label: "Output language",
        items: [{ value: "english", label: "English" }, { value: "spoken", label: "Spoken language" }],
        value: function () { return cfg().translate ? "english" : "spoken"; },
        onChange: function (v) { patchConfig({ translate: v === "english" }); }
      });

      var dictation = card([
        settingRow("Push to talk", "Hold to talk, release to paste.", pushKey.el),
        settingRow("Hands-free", "Press once to start listening, again to stop. Adding the extra key while holding push to talk switches to hands-free.", freeKey.el),
        settingRow("Output language", "English translates what you say. Spoken language keeps it as spoken.", addSync(output)),
        settingRow("Spoken language", "Auto-detect works for most speech.", addSync(select({
          label: "Spoken language", options: langOptions,
          value: function () { return cfg().language; },
          onChange: function (v) { patchConfig({ language: v }); }
        })))
      ]);

      var outputCard = card([
        settingRow("Paste into the focused app", "Off copies to the clipboard only.", addSync(toggle({
          label: "Paste into the focused app",
          value: function () { return cfg().paste; },
          onChange: function (v) { patchConfig({ paste: v }); }
        }))),
        settingRow("Restore clipboard", "Put back what you had copied before, after pasting.", addSync(toggle({
          label: "Restore clipboard",
          value: function () { return cfg().restore_clipboard; },
          disabled: function () { return !cfg().paste; },
          onChange: function (v) { patchConfig({ restore_clipboard: v }); }
        }))),
        settingRow("Microphone", null, addSync(select({
          label: "Microphone", options: micOptions,
          value: function () { return cfg().microphone || ""; },
          onChange: function (v) { patchConfig({ microphone: v === "" ? null : v }); }
        }))),
        settingRow("Sounds", "A short tone when recording starts and stops.", addSync(toggle({
          label: "Sounds",
          value: function () { return cfg().sounds; },
          onChange: function (v) { patchConfig({ sounds: v }); }
        })))
      ]);

      var themes = themeControl();
      syncers.push(themes.sync);
      var overlayCard = card([
        settingRow("Show overlay", "A small indicator while you dictate.", addSync(toggle({
          label: "Show overlay",
          value: function () { return cfg().show_overlay; },
          onChange: function (v) { patchConfig({ show_overlay: v }); }
        }))),
        h("div", { class: "srow srow-stack" }, h("div", { class: "srow-title", text: "Theme" }), themes.el),
        settingRow("Pill position", "Show the pill, then drag it where you like. It snaps to the corners.", addSync(pillPositionControl()))
      ]);

      var general = card([
        settingRow("Save history", "Off keeps nothing on disk and deletes what is saved.", addSync(toggle({
          label: "Save history",
          value: function () { return cfg().save_history; },
          onChange: function (v) {
            if (v) patchConfig({ save_history: true });
            else askTurnOffHistory();
          }
        }))),
        installRow(),
        settingRow("Start at login", "Opens local-stt when you log in. Turning it on installs local-stt first if needed.", addSync(toggle({
          label: "Start at login",
          value: function () { return typeof S.autostartEnabled === "boolean" ? S.autostartEnabled : cfg().autostart; },
          onChange: function (v) { patchConfig({ autostart: v }); }
        })))
      ]);

      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Dictation" }));
      settingsPanel.appendChild(dictation);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Output" }));
      settingsPanel.appendChild(outputCard);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Overlay" }));
      settingsPanel.appendChild(overlayCard);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "General" }));
      settingsPanel.appendChild(general);
      settingsPanel.appendChild(h("p", { class: "footnote", text: "local-stt " + dash(S.version) + ". Audio is processed on this computer and is never uploaded." }));
      syncSettings();
    }

    // ---------- render ----------

    function renderAll() {
      if (!S.loaded) {
        [panels.history, panels.model, panels.settings].forEach(function (p) { clear(p); });
        if (S.loadError) {
          panels[S.tab].appendChild(emptyState("Couldn't load", S.loadError,
            h("button", { class: "btn", type: "button", text: "Try again", onclick: refreshState })));
        }
        return;
      }
      if (!panels.history.contains(historyBar)) {
        clear(panels.history);
        panels.history.appendChild(historyBar);
        panels.history.appendChild(historyList);
      }
      renderModels();
      buildSettings();
      renderHistoryList();
    }

    // ---------- events ----------

    function subscribe(name, handler) {
      if (!listen) return;
      Promise.resolve(listen(name, function (event) { handler(event.payload); })).then(function (fn) {
        if (typeof fn === "function") unlisten.push(fn);
      }).catch(function () {});
    }

    subscribe("model-progress", function (p) {
      if (!p || !p.id) return;
      S.progress[p.id] = { downloaded: p.downloaded, total: p.total };
      if (progressRefs[p.id]) paintProgress(p.id);
      else if (S.loaded) renderModels();
    });

    subscribe("model-done", function (p) {
      if (!p || !p.id) return;
      delete S.progress[p.id];
      if (S.loadingModel === p.id) S.loadingModel = "";
      if (p.ok === false) {
        S.modelErrors[p.id] = p.error || "The download failed. The previous model is still active.";
        toast(S.modelErrors[p.id]);
      }
      refreshState();
    });

    // the app saves a captured shortcut itself, so this only repaints; "Cancelled" is Escape, not a failure
    subscribe("hotkey-captured", function (p) {
      S.capturingSlot = null;
      var slot = p && p.slot === "toggle" ? "toggle" : "hold";
      S.hotkeyErrors = { hold: "", toggle: "" };
      if (p && p.error && p.error !== "Cancelled") S.hotkeyErrors[slot] = p.error;
      syncSettings();
      if (p && !p.error) reloadState();
    });

    subscribe("notice", function (p) {
      if (p && typeof p.message === "string" && p.message !== "") toast(p.message);
    });

    subscribe("history-changed", function () {
      if (S.tab === "history") loadHistory();
      else S.historyStale = true;
    });

    // ---------- start ----------

    refreshState().then(function () {
      if (S.loaded) loadHistory();
    });

    return {
      destroy: function () {
        unlisten.forEach(function (fn) { fn(); });
        unlisten = [];
        clearTimeout(toastTimer);
        document.removeEventListener("keydown", onShortcut);
        document.removeEventListener("visibilitychange", onHidden);
        window.removeEventListener("pagehide", stopMoving);
      },
      showTab: showTab
    };
  }

  window.mountMain = mountMain;
  if (!window.__LSTT_PREVIEW__) mountMain(document.body, window.__TAURI__);
})();
