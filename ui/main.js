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
    { value: "notes", label: "Notes" },
    { value: "insights", label: "Insights" },
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
    chevron: ["M9 6l6 6-6 6"],
    freeze: ["M12 4v16", "M5.1 8l13.8 8", "M18.9 8 5.1 16"],
    minus: ["M8 12h8"],
    plus: ["M12 8v8M8 12h8"],
    up: ["M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17Z", "M12 16.5v-8", "M8.6 11.6 12 8.2l3.4 3.4"]
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
      demo: false,
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
      notes: { supported: true, reason: null, active: null }, // from the app: can notes run here, and the one recording
      noteList: [],
      noteListLoaded: false,
      noteListStale: false,
      noteSearch: "",
      insights: null, // the get_insights view, or null before the first load
      insightsError: "",
      insightsSeq: 0, // drops an older answer that arrives after a newer one
      openNote: null, // the whole note shown in the Notes tab, or null for the list
      openStatus: "done", // recording, tidying or done
      progress: {},
      modelErrors: {},
      summaryModels: [], // summary model fits from the app
      summaryDownloading: null,
      summarizing: null, // the note id being summarized
      summaryStep: null, // { done, total } for that note
      activeModel: null, // the model actually loaded, which config.model may not be
      downloading: null,
      loadingModel: "",
      autostartEnabled: null,
      accessibility: null, // macOS: whether the shortcut and paste have their rights; null elsewhere
      moving: false,
      capturingSlot: null, // "hold", "toggle" or null: one capture at a time
      hotkeyErrors: { hold: "", toggle: "" },
      installing: false,
      update: null, // { current, latest, available, notesUrl } from the app, or null
      hiddenUpdate: "", // the version whose banner was dismissed (Later) or just skipped
      offeredUpdate: "", // a version the app just offered on request, which beats an earlier Skip
      installingUpdate: false,
      updateError: "",
      checkingUpdates: false
    };
    var syncers = []; // settings controls, refreshed after every config change
    var OB = { on: false, step: 1, name: "", nameError: "", custom: false, rows: null, run: 0, dl: null, mic: { state: "idle", level: 0, error: "" }, access: "idle", syncers: [], refs: {}, primary: null }; // setup state
    var sink = syncers; // where the control builders register their sync, so setup can build its own copies
    var progressRefs = {}; // model id -> { bar, text, track }
    var unlisten = [];
    var toastTimer = 0;
    var checkTimer = 0;
    var bannerKey = "";

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
    var banner = h("section", { class: "mw-banner", role: "region", "aria-label": "Software update", hidden: true });

    root.appendChild(toolbar); // first in the DOM so the tabs lead the Tab order, CSS keeps them on top
    root.appendChild(banner);
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
      if (name !== "history" && historyPick.on) historyPick.set(false);
      if (name !== "notes" && notesPick.on) notesPick.set(false);
      if (name === "notes" && S.noteListStale) loadNotes();
      if (name === "insights") loadInsights(); // every visit, so a new day or a missed event never shows old numbers
      if (name === "history" && S.historyStale) loadHistory();
    }

    // ---------- keyboard shortcuts ----------

    // Cmd on macOS, Ctrl on Windows: 1 to 5 pick a tab, F searches the history, W closes the window
    function onShortcut(event) {
      var mod = S.platform === "windows" ? event.ctrlKey : event.metaKey;
      if (!mod || event.altKey || event.shiftKey || S.capturingSlot || dialog.open) return;
      var tab = { "1": "history", "2": "notes", "3": "insights", "4": "model", "5": "settings" }[event.key];
      var win = tauri && tauri.window && tauri.window.getCurrentWindow ? tauri.window.getCurrentWindow() : null;
      if ((event.key === "w" || event.key === "W") && win) {
        event.preventDefault();
        stopMoving();
        win.close();
      } else if (OB.on) {
        return; // setup replaces the tabs
      } else if (tab) {
        event.preventDefault();
        showTab(tab);
      } else if (event.key === "f" || event.key === "F") {
        event.preventDefault();
        // searches the list in front: notes on the Notes tab, dictations everywhere else
        var box = S.tab === "notes" ? notesSearch.input : searchInput;
        if (S.tab === "notes" && S.openNote) showNotesList();
        else if (S.tab !== "notes") showTab("history");
        box.focus();
        box.select();
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
      S.demo = state.demo === true;
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
      S.summaryModels = Array.isArray(state.summaryModels) ? state.summaryModels : [];
      S.summaryDownloading = typeof state.summaryDownloading === "string" ? state.summaryDownloading : null;
      S.summarizing = typeof state.summarizing === "string" ? state.summarizing : null;
      S.autostartEnabled = typeof state.autostartEnabled === "boolean" ? state.autostartEnabled : null;
      S.accessibility = typeof state.accessibility === "boolean" ? state.accessibility : null;
      S.update = state.update && typeof state.update === "object" ? state.update : null;
      if (state.notes && typeof state.notes === "object") {
        S.notes = {
          supported: state.notes.supported !== false,
          reason: typeof state.notes.reason === "string" ? state.notes.reason : null,
          active: state.notes.active && typeof state.notes.active.id === "string" ? state.notes.active : null
        };
      }
      if (S.downloading && !S.progress[S.downloading]) S.progress[S.downloading] = { downloaded: null, total: null };
      if (S.summaryDownloading && !S.progress[S.summaryDownloading]) S.progress[S.summaryDownloading] = { downloaded: null, total: null };
      if (S.loadingModel && S.loadingModel === S.activeModel) S.loadingModel = "";
      S.loaded = true;
      S.loadError = "";
      root.dataset.platform = S.platform;
      applyAppearance();
      renderBanner();
    }

    function refreshState() {
      return Promise.resolve(invoke("get_state")).then(function (state) {
        applyState(state);
        renderAll();
        obSync();
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
        obSync();
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

    // ---------- Update banner ----------

    function updateVisible() {
      var u = S.update;
      if (!(u && u.available === true && typeof u.latest === "string" && u.latest !== "" && u.latest !== S.hiddenUpdate)) return false;
      return S.offeredUpdate === u.latest || !(S.config && S.config.skip_update === u.latest);
    }

    function renderUpdatedNotice() {
      if (bannerKey === "updated") return;
      bannerKey = "updated";
      var whatsNew = h("button", { class: "btn btn-quiet", type: "button", text: "What's New", onclick: openReleaseNotes });
      var close = h("button", { class: "btn btn-quiet", type: "button", text: "Close", onclick: function () {
        S.updatedNotice = false;
        renderBanner();
      } });
      clear(banner);
      banner.appendChild(h("div", { class: "banner-main banner-done" },
        icon("check"),
        h("div", { class: "banner-text", text: "Updated to local-stt " + dash(S.version) + "." })
      ));
      banner.appendChild(h("div", { class: "banner-actions" }, whatsNew, close));
    }

    // a short burst over the window on the first start of a new version; none when motion is reduced
    function confetti() {
      if (root.classList.contains("reduce-motion")) return;
      var colors = ["var(--accent)", "var(--green)", "var(--orange)", "var(--red)"];
      var layer = h("div", { class: "confetti", "aria-hidden": "true" });
      for (var i = 0; i < 60; i++) {
        var piece = h("i");
        piece.style.left = (Math.random() * 100).toFixed(1) + "%";
        piece.style.background = colors[i % colors.length];
        piece.style.animationDelay = (Math.random() * 0.4).toFixed(2) + "s";
        piece.style.animationDuration = (1.6 + Math.random() * 1.2).toFixed(2) + "s";
        piece.style.setProperty("--drift", Math.round((Math.random() - 0.5) * 160) + "px");
        piece.style.setProperty("--spin", Math.round(360 + Math.random() * 720) + "deg");
        layer.appendChild(piece);
      }
      document.body.appendChild(layer);
      setTimeout(function () { layer.remove(); }, 3600);
    }

    function celebrateUpdate() {
      S.updatedNotice = true;
      renderBanner();
      announce("Updated to local-stt " + dash(S.version) + ".");
      confetti();
    }

    function installButton() {
      return banner.querySelector(".btn-install");
    }

    function renderBanner() {
      var show = updateVisible();
      // a newer version on offer outranks the note that the last update finished
      var notice = !show && S.updatedNotice === true;
      banner.hidden = !(show || notice);
      root.classList.toggle("has-banner", show || notice);
      if (notice) {
        renderUpdatedNotice();
        return;
      }
      if (!show) {
        var lostFocus = banner.contains(document.activeElement);
        bannerKey = "";
        clear(banner);
        if (lostFocus) document.getElementById("tab-" + S.tab).focus(); // a button that vanished must not drop focus to the page
        return;
      }
      var u = S.update;
      var key = [u.latest, u.current, S.installingUpdate, S.updateError].join("|");
      if (key === bannerKey) return; // an unchanged banner keeps its focus
      bannerKey = key;
      var hadFocus = banner.contains(document.activeElement);
      var busy = S.installingUpdate;
      var install = h("button", { class: "btn btn-action btn-install" + (busy ? " btn-busy" : ""), type: "button", disabled: busy, onclick: installUpdate },
        busy ? [h("i", { class: "spin", "aria-hidden": "true" }), "Installing..."] : S.updateError ? "Try Again" : "Install and Restart");
      var notes = h("button", { class: "btn btn-quiet", type: "button", text: "Release Notes", disabled: busy, onclick: openReleaseNotes });
      var later = h("button", { class: "btn btn-quiet", type: "button", text: "Later", disabled: busy, onclick: dismissUpdate });
      var skip = h("button", { class: "btn btn-quiet", type: "button", text: "Skip This Version", disabled: busy, onclick: skipUpdate });
      clear(banner);
      banner.appendChild(h("div", { class: "banner-main" },
        icon("up"),
        h("div", { class: "banner-text", text: "local-stt " + u.latest + " is available. You have " + dash(u.current) + "." })
      ));
      banner.appendChild(h("div", { class: "banner-actions" }, install, notes, later, skip));
      if (S.updateError) banner.appendChild(h("div", { class: "banner-error", role: "alert", text: S.updateError }));
      if (hadFocus && !busy) install.focus();
    }

    function installUpdate() {
      if (S.installingUpdate || !updateVisible()) return;
      var latest = S.update.latest;
      S.installingUpdate = true;
      S.updateError = "";
      renderBanner();
      Promise.resolve(invoke("install_update")).then(function (sentence) {
        S.installingUpdate = false;
        S.hiddenUpdate = latest; // the app restarts itself, so the banner steps aside
        renderBanner();
        toast(typeof sentence === "string" && sentence ? sentence : "Update installed. local-stt will restart.");
      }).catch(function (err) {
        S.installingUpdate = false;
        S.updateError = errText(err);
        renderBanner();
        if (installButton()) installButton().focus();
      });
    }

    function openReleaseNotes() {
      Promise.resolve(invoke("open_release_notes")).catch(function (err) { toast(errText(err)); });
    }

    // Later hides this version for the session only; the app offers it again next launch
    function dismissUpdate() {
      if (S.update) S.hiddenUpdate = S.update.latest;
      renderBanner();
    }

    function skipUpdate() {
      if (!S.update) return;
      S.hiddenUpdate = S.update.latest;
      S.offeredUpdate = "";
      renderBanner();
      Promise.resolve(invoke("skip_update")).then(reloadState).catch(function (err) {
        S.hiddenUpdate = "";
        renderBanner();
        toast("Couldn't skip: " + errText(err));
      });
    }

    // the tray's Update item: bring the banner back even after Later, and put focus on Install
    function showUpdate() {
      S.hiddenUpdate = "";
      renderBanner();
      function focusInstall() {
        banner.scrollIntoView({ block: "nearest" });
        if (installButton() && !installButton().disabled) installButton().focus();
      }
      if (updateVisible()) focusInstall();
      else reloadState().then(function () { if (updateVisible()) focusInstall(); });
    }

    function finishCheck() {
      clearTimeout(checkTimer);
      if (!S.checkingUpdates) return;
      S.checkingUpdates = false;
      syncSettings();
    }

    // ---------- select mode, shared by History and Notes ----------

    // Select mode for one list: a sticky bar with Select All, a count and Delete, and a checkbox per row and per day.
    function picker(cfg) {
      var p = { on: false, chosen: new Set(), anchor: null, shown: [] };
      var allBox = h("input", { type: "checkbox", class: "check", id: cfg.allId });
      var count = h("span", { class: "select-count", "aria-live": "polite" });
      var del = h("button", { class: "btn btn-danger btn-icon", type: "button", onclick: askDelete }, icon("trash"), h("span", { text: "Delete" }));
      p.bar = h("div", { class: "select-bar", role: "toolbar", "aria-label": "Selected " + cfg.many, hidden: true },
        h("label", { class: "select-all", for: cfg.allId }, allBox, h("span", { text: "Select All" })),
        count,
        del
      );
      allBox.addEventListener("change", function () {
        var all = allShown();
        p.shown.forEach(function (id) {
          if (all) p.chosen.delete(id);
          else p.chosen.add(id);
        });
        p.anchor = null;
        sync();
      });

      function allShown() {
        return p.shown.length > 0 && p.shown.every(function (id) { return p.chosen.has(id); });
      }

      // a tri-state box: checked when all of ids are chosen, mixed when some are
      function syncBox(box, ids) {
        var n = ids.filter(function (id) { return p.chosen.has(id); }).length;
        box.checked = n > 0 && n === ids.length;
        box.indeterminate = n > 0 && n < ids.length;
      }

      // repaint the checks in place, so the focused checkbox keeps focus
      function sync() {
        cfg.list.querySelectorAll(".pickable").forEach(function (row) {
          var on = p.chosen.has(row._id);
          row.classList.toggle("is-selected", on);
          if (row._box) row._box.checked = on;
        });
        cfg.list.querySelectorAll(".group-check").forEach(function (box) { syncBox(box, box._ids); });
        syncBox(allBox, p.shown);
        count.textContent = p.chosen.size === 0 ? "None selected" : p.chosen.size + " selected";
        del.disabled = p.chosen.size === 0;
      }

      // click picks one; Shift-click picks the run from the last one, like Finder
      function toggle(id, shift) {
        var to = !p.chosen.has(id);
        var from = p.anchor === null ? -1 : p.shown.indexOf(p.anchor);
        var at = p.shown.indexOf(id);
        if (shift && from !== -1 && at !== -1) {
          p.shown.slice(Math.min(from, at), Math.max(from, at) + 1).forEach(function (other) {
            if (to) p.chosen.add(other);
            else p.chosen.delete(other);
          });
        } else if (to) {
          p.chosen.add(id);
        } else {
          p.chosen.delete(id);
        }
        p.anchor = id;
        sync();
      }

      function askDelete() {
        var ids = Array.from(p.chosen);
        if (ids.length === 0) return;
        askConfirm({
          title: ids.length === 1 ? "Delete this " + cfg.one + "?" : "Delete " + ids.length + " " + cfg.many + "?",
          body: (ids.length === 1 ? "It" : "They") + " will be removed from " + thisDevice() + ". This can't be undone.",
          confirm: "Delete",
          danger: true,
          onConfirm: function () {
            Promise.resolve(cfg.remove(ids)).then(function () {
              p.set(false);
              announce("Deleted " + (ids.length === 1 ? "1 " + cfg.one : ids.length + " " + cfg.many) + ".");
            }).catch(function (err) { toast("Couldn't delete: " + errText(err)); });
          }
        });
      }

      p.set = function (on) {
        p.on = on;
        p.chosen.clear();
        p.anchor = null;
        cfg.button.textContent = on ? "Done" : "Select";
        cfg.button.classList.toggle("btn-action", on);
        cfg.hideWhenOn.forEach(function (el) { el.hidden = on; });
        p.bar.hidden = !on;
        cfg.render();
        if (on) allBox.focus();
        else cfg.button.focus();
      };

      // what the list shows now; picks the search hides are dropped, so Delete never removes unseen rows
      p.show = function (ids) {
        p.shown = ids;
        var shown = new Set(ids);
        p.chosen.forEach(function (id) { if (!shown.has(id)) p.chosen.delete(id); });
        if (p.on) sync();
      };

      p.rowBox = function (id, label) {
        if (!p.on) return null;
        var box = h("input", { type: "checkbox", class: "check " + cfg.rowClass, "aria-label": label });
        box.addEventListener("click", function (event) { toggle(id, event.shiftKey); });
        return box;
      };

      p.attach = function (row, id, box) {
        row._id = id;
        row._box = box;
        row.classList.add("pickable");
        if (!p.on) return;
        row.classList.add("selecting");
        row.addEventListener("click", function (event) {
          if (event.target === box || event.target.closest("button, input, a")) return;
          if (window.getSelection && String(window.getSelection()).length > 0) return; // a text drag is not a pick
          toggle(id, event.shiftKey);
        });
      };

      // a day heading, with a box that picks the whole day while selecting
      p.dayTitle = function (label, ids) {
        var title = h("h3", { class: "group-title" });
        if (!p.on) {
          title.textContent = label;
          return title;
        }
        var box = h("input", { type: "checkbox", class: "check group-check", "aria-label": "Select all from " + label });
        box._ids = ids;
        box.addEventListener("change", function () {
          var all = ids.every(function (id) { return p.chosen.has(id); });
          ids.forEach(function (id) {
            if (all) p.chosen.delete(id);
            else p.chosen.add(id);
          });
          sync();
        });
        title.appendChild(h("label", { class: "group-pick" }, box, h("span", { text: label })));
        return title;
      };

      p.sync = sync;

      // Esc leaves select mode, Delete asks to delete, Cmd/Ctrl+A picks everything shown
      cfg.panel.addEventListener("keydown", function (event) {
        if (!p.on || dialog.open) return;
        var typing = event.target.tagName === "INPUT" && event.target.type === "text";
        var mod = S.platform === "windows" ? event.ctrlKey : event.metaKey;
        if (event.key === "Escape") {
          event.preventDefault();
          p.set(false);
        } else if ((event.key === "Delete" || event.key === "Backspace") && !typing) {
          event.preventDefault();
          askDelete();
        } else if (mod && (event.key === "a" || event.key === "A") && !typing) {
          event.preventDefault();
          p.shown.forEach(function (id) { p.chosen.add(id); });
          sync();
        }
      });
      return p;
    }

    // rows grouped under day headings, so History and Notes read the same way
    function dayGroups(list, items, timeOf, idOf, pick, rowOf) {
      var rows = [];
      var group = null;
      var card = null;
      items.forEach(function (item) {
        var ms = timeOf(item);
        var date = typeof ms === "number" ? new Date(ms) : null;
        var key = date ? dayKey(date) : "unknown";
        if (key !== group) {
          group = key;
          var ids = items.filter(function (other) {
            var t = timeOf(other);
            return (typeof t === "number" ? dayKey(new Date(t)) : "unknown") === key;
          }).map(idOf);
          list.appendChild(pick.dayTitle(date ? dayLabel(date) : "-", ids));
          card = h("div", { class: "card" });
          list.appendChild(card);
        }
        var row = rowOf(item);
        rows.push(row);
        card.appendChild(row);
      });
      if (pick.on) pick.sync();
      return rows;
    }

    function searchBox(placeholder, onChange) {
      var input = h("input", { class: "search-input", type: "text", placeholder: placeholder, "aria-label": placeholder, autocomplete: "off", spellcheck: "false" });
      var clearIt = h("button", { class: "icon-btn search-clear", type: "button", "aria-label": "Clear search", hidden: true, onclick: function () {
        input.value = "";
        changed();
        input.focus();
      } }, icon("close"));
      function changed() {
        clearIt.hidden = input.value === "";
        onChange(input.value);
      }
      input.addEventListener("input", changed);
      return { input: input, el: h("div", { class: "search" }, icon("search"), input, clearIt) };
    }

    // ---------- History ----------

    var historyPanel = panels.history;
    var historySearch = searchBox("Search dictations", function (value) {
      S.search = value;
      renderHistoryList();
    });
    var searchInput = historySearch.input;
    var selectButton = h("button", { class: "btn", type: "button", text: "Select", onclick: function () { historyPick.set(!historyPick.on); } });
    var clearButton = h("button", { class: "btn btn-danger", type: "button", text: "Clear History", onclick: askClearHistory });
    var historyList = h("div", { class: "history-list" });
    var historyBar = h("div", { class: "history-bar" }, historySearch.el, selectButton, clearButton);
    var historyPick = picker({
      allId: "select-all",
      one: "dictation",
      many: "dictations",
      rowClass: "hrow-check",
      list: historyList,
      panel: historyPanel,
      button: selectButton,
      hideWhenOn: [clearButton],
      render: function () { renderHistoryList(); },
      remove: function (ids) {
        return Promise.resolve(invoke("delete_history", { ids: ids })).then(function () {
          var gone = new Set(ids);
          S.history = S.history.filter(function (e) { return !gone.has(entryId(e)); });
        });
      }
    });
    historyPanel.appendChild(historyBar);
    historyPanel.appendChild(historyPick.bar);
    historyPanel.appendChild(historyList);

    // the app's id; entries saved before ids existed go by their time, as the app does
    function entryId(e) {
      return typeof e.id === "number" && e.id > 0 ? e.id : e.ts_ms;
    }

    function renderHistoryEmptyHint() {
      if (S.historyLoaded && S.history.length === 0) renderHistoryList();
    }

    function emptyState(title, body, extra) {
      return h("div", { class: "empty" }, h("div", { class: "empty-title", text: title }), h("div", { class: "empty-body" }, body), extra);
    }

    // a copy button that shows Copied for a second
    function copyButton(label, textOf) {
      var copy = h("button", { class: "icon-btn copy", type: "button", "aria-label": label }, icon("copy"));
      var timer = 0;
      copy.addEventListener("click", function () {
        Promise.resolve(invoke("copy_text", { text: textOf() })).then(function () {
          clearTimeout(timer);
          clear(copy);
          copy.appendChild(icon("check"));
          copy.classList.add("done");
          copy.setAttribute("aria-label", "Copied");
          announce("Copied to the clipboard.");
          timer = setTimeout(function () {
            clear(copy);
            copy.appendChild(icon("copy"));
            copy.classList.remove("done");
            copy.setAttribute("aria-label", label);
          }, 1000);
        }).catch(function (err) { toast("Couldn't copy: " + errText(err)); });
      });
      return copy;
    }

    function historyRow(entry) {
      var text = typeof entry.text === "string" ? entry.text : "";
      var date = typeof entry.ts_ms === "number" ? new Date(entry.ts_ms) : null;
      var time = date ? date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }) : "-";
      var seconds = typeof entry.duration_ms === "number" ? (entry.duration_ms / 1000).toFixed(1) + " s" : "-";
      var failed = entry.ok === false;

      var body = h("div", { class: "hrow-text" + (text ? "" : " is-empty"), text: text || "Nothing was transcribed." });
      var moreDot = h("span", { class: "dot-sep", "aria-hidden": "true", hidden: true });
      var more = h("button", { class: "linklike", type: "button", text: "Show More", hidden: true, onclick: function () {
        var open = body.classList.toggle("open");
        more.textContent = open ? "Show Less" : "Show More";
      } });
      var snippet = text.length > 40 ? text.slice(0, 40).trim() + "..." : text;
      var copy = copyButton(text === "" ? "Copy dictation (empty)" : "Copy dictation: " + snippet, function () { return text; });
      copy.disabled = text === "";

      var id = entryId(entry);
      var box = historyPick.rowBox(id, "Select dictation: " + (snippet || "empty"));
      var row = h("div", { class: "hrow" + (failed ? " failed" : "") },
        box,
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
        historyPick.on ? null : h("div", { class: "hrow-actions" }, copy)
      );
      historyPick.attach(row, id, box);
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
      selectButton.disabled = S.history.length === 0 && !historyPick.on;
      historyPick.show(entries.map(entryId));

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

      var rows = dayGroups(historyList, entries, function (e) { return e.ts_ms; }, entryId, historyPick, historyRow);

      // reveal "Show More" only where the clamp actually hides text
      requestAnimationFrame(function () {
        rows.forEach(function (row) {
          if (row._body.scrollHeight > row._body.clientHeight + 1) {
            row._more.hidden = false;
            row._moreDot.hidden = false;
          }
        });
      });
    }

    // ---------- Notes ----------

    var CONSENT_LINE = "Heads up: I'm taking notes of this call with a transcription app that runs only on my computer. Tell me if you'd rather I didn't.";
    var notesPanel = panels.notes;
    var notesSearch = searchBox("Search notes", function (value) {
      S.noteSearch = value;
      renderNotesList();
    });
    var newNoteButton = h("button", { class: "btn btn-action", type: "button", text: "New Note", onclick: function () { newNote(); } });
    var notesSelectButton = h("button", { class: "btn", type: "button", text: "Select", onclick: function () { notesPick.set(!notesPick.on); } });
    var notesList = h("div", { class: "history-list notes-list" });
    var notesGate = h("p", { class: "footnote notes-gate", hidden: true });
    var notesBar = h("div", { class: "history-bar" }, notesSearch.el, notesSelectButton, newNoteButton);
    var notesPick = picker({
      allId: "select-all-notes",
      one: "note",
      many: "notes",
      rowClass: "nrow-check",
      list: notesList,
      panel: notesPanel,
      button: notesSelectButton,
      hideWhenOn: [newNoteButton],
      render: function () { renderNotesList(); },
      remove: function (ids) {
        return Promise.resolve(invoke("delete_notes", { ids: ids })).then(function () {
          var gone = new Set(ids);
          S.noteList = S.noteList.filter(function (n) { return !gone.has(n.id); });
        });
      }
    });
    var notesListView = h("div", { class: "notes-list-view" }, notesBar, notesGate, notesPick.bar, notesList);
    var noteView = h("div", { class: "note-view", hidden: true });
    notesPanel.appendChild(notesListView);
    notesPanel.appendChild(noteView);
    var noteTimer = 0;

    function noteTime(ms) {
      return typeof ms === "number" ? new Date(ms).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }) : "-";
    }

    // an untitled note reads "Zoom call, 10:30" when the app is known, else "Note, 10:30"
    function noteTitle(n) {
      if (n.title) return n.title;
      return (n.source ? n.source + " call" : "Note") + ", " + noteTime(n.started_ms !== undefined ? n.started_ms : n.startedMs);
    }

    function clock(ms) {
      var s = Math.max(0, Math.floor(ms / 1000));
      var hh = Math.floor(s / 3600);
      var mm = Math.floor((s % 3600) / 60);
      var ss = s % 60;
      var two = function (n) { return (n < 10 ? "0" : "") + n; };
      return (hh > 0 ? hh + ":" + two(mm) : mm) + ":" + two(ss);
    }

    function noteLength(start, end) {
      if (typeof start !== "number" || typeof end !== "number") return "-";
      var min = Math.round((end - start) / 60000);
      if (min < 1) return "Under a minute";
      if (min < 60) return min + " min";
      return Math.floor(min / 60) + " h " + (min % 60) + " min";
    }

    function isRecording(id) {
      return !!(S.notes.active && S.notes.active.id === id);
    }

    function loadNotes() {
      S.noteListStale = false;
      return Promise.resolve(invoke("list_notes")).then(function (rows) {
        S.noteList = Array.isArray(rows) ? rows : [];
        S.noteListLoaded = true;
        renderNotesList();
      }).catch(function (err) { toast(errText(err)); });
    }

    function noteRow(n) {
      var recording = isRecording(n.id) || n.status === "recording";
      var title = noteTitle(n);
      var box = notesPick.rowBox(n.id, "Select note: " + title);
      var open = h("button", { class: "nrow-open", type: "button", "aria-label": "Open " + title, onclick: function () { openNote(n.id); } },
        h("div", { class: "nrow-title", text: title }),
        h("div", { class: "hrow-meta" },
          n.title ? h("span", { text: noteTime(n.startedMs) }) : null, // an untitled note already has the time in its title
          n.title ? h("span", { class: "dot-sep", "aria-hidden": "true" }) : null,
          recording ? h("span", { class: "tag tag-red", text: "Recording" }) : h("span", { text: noteLength(n.startedMs, n.endedMs) })
        ),
        n.firstLine ? h("div", { class: "nrow-first", text: n.firstLine }) : null
      );
      var row = h("div", { class: "nrow" }, box, open, notesPick.on ? null : h("span", { class: "nrow-chevron", "aria-hidden": "true" }, icon("chevron")));
      notesPick.attach(row, n.id, box);
      if (notesPick.on) open.disabled = true;
      return row;
    }

    function renderNotesList() {
      clear(notesList);
      var gate = S.notes.supported === false ? (S.notes.reason || "Notes are not available on this computer.") : "";
      notesGate.textContent = gate;
      notesGate.hidden = gate === "";
      newNoteButton.disabled = gate !== "";
      newNoteButton.textContent = S.notes.active ? "Open Recording" : "New Note";
      var needle = S.noteSearch.trim().toLowerCase();
      var rows = S.noteList.filter(function (n) {
        return needle === "" || (noteTitle(n) + " " + (n.firstLine || "")).toLowerCase().indexOf(needle) !== -1;
      });
      notesSelectButton.disabled = S.noteList.length === 0 && !notesPick.on;
      notesPick.show(rows.map(function (n) { return n.id; }));
      if (!S.noteListLoaded) return;
      if (S.noteList.length === 0) {
        notesList.appendChild(emptyState("No notes yet", "Click New Note when a call starts. local-stt records your microphone as Me and the call as Others, and writes it all down on this computer."));
        return;
      }
      if (rows.length === 0) {
        notesList.appendChild(emptyState("No results", "Nothing matches your search."));
        return;
      }
      dayGroups(notesList, rows, function (n) { return n.startedMs; }, function (n) { return n.id; }, notesPick, noteRow);
    }

    function showNotesList() {
      S.openNote = null;
      clearInterval(noteTimer);
      noteView.hidden = true;
      notesListView.hidden = false;
      renderNotesList();
    }

    function openNote(id) {
      return Promise.resolve(invoke("get_note", { id: id })).then(function (note) {
        S.openNote = note;
        S.openStatus = isRecording(id) ? "recording" : note.status === "recording" ? "tidying" : "done";
        notesListView.hidden = true;
        noteView.hidden = false;
        renderNoteView();
        var back = noteView.querySelector(".note-back");
        if (back) back.focus();
      }).catch(function (err) { toast(errText(err)); });
    }

    function noteText(note) {
      var lines = [noteTitle(note), new Date(note.started_ms).toLocaleString(), ""];
      if (note.summary) lines = lines.concat(summaryText(note), [""]);
      (note.segments || []).forEach(function (seg) {
        lines.push(speakerName(seg.speaker, note) + " (" + clock(seg.start_ms) + "): " + seg.text);
      });
      return lines.join("\n");
    }

    // "s2" is the second voice heard on the other side of the call
    function defaultSpeaker(id) {
      if (id === "me") return savedName() || "Me"; // the name given in setup
      if (id === "others") return "Others";
      var m = /^s([0-9]+)$/.exec(String(id || ""));
      return m ? "Speaker " + m[1] : String(id || "-");
    }

    // the name the user gave this speaker in this note, else the default
    function speakerName(id, note) {
      var names = (note || S.openNote || {}).speakers || {};
      return typeof names[id] === "string" && names[id] !== "" ? names[id] : defaultSpeaker(id);
    }

    // one name box per speaker; a name replaces the label on every line and in copied text
    function speakersRow(note) {
      var keys = ["me"];
      (note.segments || []).forEach(function (seg) { if (keys.indexOf(seg.speaker) === -1) keys.push(seg.speaker); });
      if (keys.length === 1) keys.push("others"); // nothing heard from the call yet
      var people = Array.isArray(note.participants) ? note.participants.filter(function (p) { return typeof p === "string" && p; }) : [];
      var listId = "call-people-" + note.id;
      var row = h("div", { class: "note-speakers", role: "group", "aria-label": "Speaker names" });
      var boxes = []; // the other speakers' name boxes, for the name chips
      var lastBox = null;
      if (people.length) row.appendChild(h("datalist", { id: listId }, people.map(function (p) { return h("option", { value: p }); })));
      keys.forEach(function (key) {
        var input = h("input", { class: "speaker-input", type: "text", maxlength: "60", placeholder: people.length && key !== "me" ? "Pick or type a name" : "Add a name", list: people.length && key !== "me" ? listId : null, value: (note.speakers || {})[key] || "" });
        input.addEventListener("change", function () { saveSpeaker(note.id, key, input.value.trim()); });
        if (key !== "me") {
          boxes.push(input);
          input.addEventListener("focus", function () { lastBox = input; });
        }
        input.addEventListener("keydown", function (event) { if (event.key === "Enter") input.blur(); });
        row.appendChild(h("label", { class: "speaker-field" }, h("span", { class: "speaker-key", text: defaultSpeaker(key) }), input));
      });
      if (!people.length) return row;
      // names Zoom never showed talking during a voice's lines: the user matches them, one click each
      var hint = h("div", { class: "call-people" },
        h("span", { class: "call-people-label", text: (note.source || "The call") + " showed:" }),
        people.map(function (p) {
          return h("button", { class: "chip", type: "button", text: p, "aria-label": "Use " + p + " as a speaker name", onclick: function () {
            var target = lastBox || boxes.filter(function (b) { return !b.value; })[0];
            if (!target) return;
            target.value = p;
            target.dispatchEvent(new Event("change"));
            lastBox = null;
          } });
        }),
        h("span", { class: "call-people-help", text: "Click a speaker's box, then a name, or pick from the box's list. A call app does not say who spoke, so local-stt only names a speaker on its own in a one-to-one call." })
      );
      return h("div", {}, row, hint);
    }

    function saveSpeaker(id, key, name) {
      Promise.resolve(invoke("rename_speaker", { id: id, speaker: key, name: name })).then(function (saved) {
        if (!S.openNote || S.openNote.id !== id) return;
        S.openNote.speakers = saved && saved.speakers && typeof saved.speakers === "object" ? saved.speakers : {};
        if (noteView._card) renderTranscript(noteView._card);
        announce(name ? defaultSpeaker(key) + " is now " + name + "." : defaultSpeaker(key) + " has its default name again.");
      }).catch(function (err) { toast("Couldn't rename: " + errText(err)); });
    }

    function turnRow(seg) {
      var row = h("div", { class: "turn turn-" + (seg.speaker === "me" ? "me" : "others"), "data-speaker": seg.speaker },
        h("div", { class: "turn-who" },
          h("span", { class: "turn-name", text: speakerName(seg.speaker) }),
          h("span", { class: "turn-time", text: clock(seg.start_ms) })
        ),
        h("div", { class: "turn-text", text: seg.text })
      );
      row._start = seg.start_ms;
      return row;
    }

    function renderTranscript(card) {
      clear(card);
      var segs = (S.openNote.segments || []).slice().sort(function (a, b) { return a.start_ms - b.start_ms; });
      if (segs.length === 0) {
        card.appendChild(h("div", { class: "turn-empty", text: S.openStatus === "recording" ? "Listening. Lines appear a few seconds after each pause." : "Nothing was transcribed." }));
        return;
      }
      segs.forEach(function (seg) { card.appendChild(turnRow(seg)); });
    }

    function renderNoteView() {
      var note = S.openNote;
      if (!note) return;
      clearInterval(noteTimer);
      clear(noteView);
      var recording = S.openStatus === "recording";
      var back = h("button", { class: "btn btn-quiet note-back", type: "button", onclick: showNotesList }, h("span", { text: "Notes" }));
      back.insertBefore(icon("chevron"), back.firstChild);
      var actions = h("div", { class: "note-actions" });
      if (!recording) {
        actions.appendChild(copyButton("Copy note", function () { return noteText(S.openNote); }));
        actions.appendChild(h("button", { class: "icon-btn", type: "button", "aria-label": "Delete note", onclick: function () { askDeleteNote(note.id); } }, icon("trash")));
      }
      var title = h("input", { class: "note-title", type: "text", maxlength: "200", "aria-label": "Note title", placeholder: noteTitle(Object.assign({}, note, { title: "" })), value: note.title || "" });
      function saveTitle() {
        var next = title.value.trim();
        if (next === (S.openNote.title || "")) return;
        Promise.resolve(invoke("rename_note", { id: note.id, title: next })).then(function (saved) {
          var title = saved && typeof saved.title === "string" ? saved.title : next;
          if (S.openNote && S.openNote.id === note.id) S.openNote.title = title;
          S.noteList.forEach(function (n) { if (n.id === note.id) n.title = title; });
        }).catch(function (err) { toast("Couldn't rename: " + errText(err)); });
      }
      title.addEventListener("change", saveTitle);
      title.addEventListener("keydown", function (event) { if (event.key === "Enter") title.blur(); });
      var meta = h("div", { class: "note-meta" },
        h("span", { text: new Date(note.started_ms).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" }) + ", " + noteTime(note.started_ms) }),
        recording ? null : h("span", { class: "dot-sep", "aria-hidden": "true" }),
        recording ? null : h("span", { text: S.openStatus === "tidying" ? "Tidying..." : noteLength(note.started_ms, note.ended_ms) })
      );
      noteView.appendChild(h("div", { class: "note-top" }, back, actions));
      noteView.appendChild(title);
      noteView.appendChild(meta);
      noteView.appendChild(speakersRow(note));
      if (recording) {
        var timer = h("span", { class: "note-timer", text: clock(Date.now() - S.notes.active.startedMs) });
        noteView.appendChild(h("div", { class: "note-live", role: "group", "aria-label": "Recording" },
          h("span", { class: "rec-dot", "aria-hidden": "true" }),
          h("span", { class: "note-rec-label", text: "Recording" }),
          timer,
          h("span", { class: "note-live-gap" }),
          h("button", { class: "btn", type: "button", text: "Copy Consent Line", onclick: copyConsent }),
          h("button", { class: "btn btn-danger-solid", type: "button", text: "Stop", onclick: stopNote })
        ));
        noteView.appendChild(h("p", { class: "footnote", text: "Use headphones, so your own words are not picked up twice." }));
        noteTimer = setInterval(function () {
          if (S.notes.active) timer.textContent = clock(Date.now() - S.notes.active.startedMs);
        }, 1000);
      }
      if (S.openStatus === "done" && summariesOn()) noteView.appendChild(summaryCard(note));
      var card = h("div", { class: "card transcript", role: "log", "aria-live": recording ? "polite" : "off" });
      noteView.appendChild(card);
      renderTranscript(card);
      noteView._card = card;
    }

    function copyConsent() {
      Promise.resolve(invoke("copy_text", { text: CONSENT_LINE })).then(function () {
        announce("Consent line copied.");
        toast("Consent line copied. Paste it in the meeting chat.");
      }).catch(function (err) { toast("Couldn't copy: " + errText(err)); });
    }

    // shown once: recording other people needs their consent in many places
    function askConsent(onContinue) {
      clear(dialog);
      dialog.appendChild(h("h2", { class: "dlg-title", id: "dlg-title", text: "Before you take notes" }));
      dialog.appendChild(h("p", { class: "dlg-body", text: "Taking notes records the other people on the call. In many places you need their consent. Tell them you are taking notes." }));
      dialog.appendChild(h("p", { class: "dlg-body consent-line", text: CONSENT_LINE }));
      var cancel = h("button", { class: "btn", type: "button", text: "Cancel", onclick: function () { closeDialog(); } });
      var copy = h("button", { class: "btn", type: "button", text: "Copy Consent Line", onclick: copyConsent });
      var go = h("button", { class: "btn btn-action", type: "button", text: "Continue", onclick: function () {
        closeDialog();
        patchConfig({ notes_consent_seen: true });
        onContinue();
      } });
      dialog.appendChild(h("div", { class: "dlg-actions" }, cancel, copy, go));
      if (typeof dialog.showModal === "function") dialog.showModal();
      else dialog.setAttribute("open", "");
      go.focus();
    }

    function newNote() {
      if (S.notes.supported === false) {
        toast(S.notes.reason || "Notes are not available on this computer.");
        return;
      }
      if (S.notes.active) {
        openNote(S.notes.active.id);
        return;
      }
      if (!(S.config && S.config.notes_consent_seen)) askConsent(startNote);
      else startNote();
    }

    function startNote() {
      Promise.resolve(invoke("start_note")).then(function (id) {
        var now = Date.now();
        S.notes.active = { id: id, startedMs: now };
        S.openNote = { id: id, title: "", started_ms: now, status: "recording", segments: [] };
        S.openStatus = "recording";
        notesListView.hidden = true;
        noteView.hidden = false;
        renderNoteView();
        announce("Taking notes.");
        loadNotes();
      }).catch(function (err) { toast("Couldn't start the note: " + errText(err)); });
    }

    function stopNote() {
      Promise.resolve(invoke("stop_note")).catch(function (err) { toast(errText(err)); });
    }

    function askDeleteNote(id) {
      askConfirm({
        title: "Delete this note?",
        body: "It will be removed from " + thisDevice() + ". This can't be undone.",
        confirm: "Delete",
        danger: true,
        onConfirm: function () {
          Promise.resolve(invoke("delete_notes", { ids: [id] })).then(function () {
            S.noteList = S.noteList.filter(function (n) { return n.id !== id; });
            showNotesList();
            announce("Deleted 1 note.");
          }).catch(function (err) { toast("Couldn't delete: " + errText(err)); });
        }
      });
    }

    function onNoteState(p) {
      if (!p || typeof p.id !== "string") return;
      if (p.status === "recording") {
        S.notes.active = { id: p.id, startedMs: typeof p.startedMs === "number" ? p.startedMs : Date.now() };
      } else if (S.notes.active && S.notes.active.id === p.id) {
        S.notes.active = null;
      }
      var open = S.openNote && S.openNote.id === p.id;
      if (open && p.status === "done") {
        openNote(p.id);
      } else if (open) {
        S.openStatus = p.status === "recording" ? "recording" : "tidying";
        renderNoteView();
      }
      if (S.tab === "notes") loadNotes();
      else S.noteListStale = true;
    }

    function onNoteSegment(p) {
      if (!p || !S.openNote || p.id !== S.openNote.id || !p.segment) return;
      S.openNote.segments = (S.openNote.segments || []).concat([p.segment]);
      var card = noteView._card;
      if (!card) return;
      var atEnd = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 40;
      // one new row in time order, so a screen reader hears only the new line and a long call stays cheap
      var empty = card.querySelector(".turn-empty");
      if (empty) card.removeChild(empty);
      var row = turnRow(p.segment);
      var after = Array.prototype.find.call(card.children, function (el) { return el._start > p.segment.start_ms; });
      card.insertBefore(row, after || null);
      if (atEnd) scroller.scrollTop = scroller.scrollHeight; // follow the call unless the reader scrolled up
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
        confirm: "Clear History",
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
        confirm: "Turn Off",
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

    // the checksum and a link to the same file on Hugging Face, where its page shows that checksum too
    function modelMeta(fit, inUse, ops) {
      ops = ops || { page: openModelPage, del: askDeleteModel };
      var hash = typeof fit.sha256 === "string" && fit.sha256 ? fit.sha256 : "-";
      var page = h("button", {
        class: "btn btn-quiet",
        type: "button",
        text: "View on Hugging Face",
        "aria-label": "View " + dash(fit.label) + " on Hugging Face",
        onclick: function () { ops.page(fit); }
      });
      var del = fit.downloaded && !inUse
        ? h("button", { class: "btn btn-danger btn-icon", type: "button", "aria-label": "Delete " + dash(fit.label), onclick: function () { ops.del(fit); } }, icon("trash"), h("span", { text: "Delete" }))
        : null;
      return h("div", { class: "mrow-meta" },
        h("span", { class: "mrow-sum" }, "SHA-256 ", h("span", { class: "mrow-hash", text: hash })),
        h("span", { class: "mrow-links" }, page, del)
      );
    }

    function modelRow(fit) {
      var active = S.activeModel === fit.id;
      var loading = S.loadingModel === fit.id;
      var progress = S.progress[fit.id];
      var inUse = active || loading || !!progress || (S.config && S.config.model === fit.id);
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
        modelMeta(fit, inUse), why, detail, error
      );
    }

    function renderModels() {
      clear(modelPanel);
      progressRefs = {};
      if (!S.loaded) return;
      modelPanel.appendChild(h("h3", { class: "group-title", text: "This computer" }));
      modelPanel.appendChild(hardwareCard());
      modelPanel.appendChild(h("h3", { class: "group-title", text: "Speech model" }));
      var list = h("div", { class: "card speech-list", role: "list", "aria-label": "Speech models" });
      S.models.forEach(function (fit) { list.appendChild(modelRow(fit)); });
      modelPanel.appendChild(list);
      modelPanel.appendChild(h("p", { class: "footnote", text: "Models run on this computer. A download is checked against a fixed checksum before it is used." }));
      renderSummaryModels();
    }

    function paintProgress(id) {
      var ref = progressRefs[id] || summaryRefs[id];
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

    function openModelPage(fit) {
      Promise.resolve(invoke("open_model_page", { id: fit.id })).catch(function (err) { toast(errText(err)); });
    }

    function askDeleteModel(fit) {
      var size = sizeText(fit);
      askConfirm({
        title: "Delete " + dash(fit.label) + "?",
        body: (size ? "This frees " + size + " on " : "This removes it from ") + thisDevice() + ". You can download it again here.",
        confirm: "Delete",
        danger: true,
        onConfirm: function () {
          delete S.modelErrors[fit.id];
          Promise.resolve(invoke("delete_model", { id: fit.id })).then(function () {
            toast(dash(fit.label) + " deleted.");
            return refreshState();
          }).catch(function (err) {
            S.modelErrors[fit.id] = errText(err);
            renderModels();
          });
        }
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


    // ---------- Summary models (a section of the Model tab) ----------

    var summaryRefs = {};

    // the model a Summarize click uses, as the app picks it: chosen and downloaded, else the recommended one
    function summaryModel() {
      var list = S.summaryModels || [];
      var chosen = S.config && S.config.summary_model;
      var usable = list.filter(function (f) { return f.downloaded && f.supported; });
      return usable.filter(function (f) { return f.id === chosen; })[0] ||
        usable.filter(function (f) { return f.recommended; })[0] ||
        usable[usable.length - 1] || null;
    }

    function summaryRow(fit) {
      var used = summaryModel();
      var inUse = !!used && used.id === fit.id;
      var progress = S.progress[fit.id];
      var busy = !!progress || (inUse && !!S.summarizing);
      var disabled = !fit.supported;
      var reasons = Array.isArray(fit.reasons) ? fit.reasons : [];
      var badges = h("span", { class: "badges" },
        fit.recommended ? h("span", { class: "tag tag-blue", text: "Recommended" }) : null,
        fit.downloaded ? h("span", { class: "tag tag-green", text: "Downloaded" }) : null,
        disabled ? h("span", { class: "tag tag-grey", text: "Not supported" }) : null
      );
      var action = null;
      if (progress) action = h("span", { class: "pill pill-busy", text: "Downloading" });
      else if (inUse) action = h("span", { class: "pill pill-active" }, icon("check"), "In use");
      else if (!disabled) {
        var size = sizeText(fit);
        action = h("button", {
          class: "btn btn-action", type: "button",
          text: fit.downloaded ? "Use" : size ? "Download " + size : "Download",
          "aria-label": (fit.downloaded ? "Use " : "Download ") + dash(fit.label) + (fit.downloaded || !size ? "" : ", " + size),
          onclick: function () { requestSummaryModel(fit); }
        });
      }
      var detail = null;
      if (progress) {
        var track = h("div", { class: "track", role: "progressbar", "aria-label": "Download progress", "aria-valuemin": "0", "aria-valuemax": "100" }, h("i"));
        var line = h("div", { class: "progress-text" });
        summaryRefs[fit.id] = { track: track, bar: track.firstChild, text: line };
        paintProgress(fit.id);
        detail = h("div", { class: "progress" }, track, line);
      }
      var notes = reasons.slice();
      if (fit.hint) notes.push(fit.hint);
      var why = notes.length
        ? h("ul", { class: "reasons" }, notes.map(function (r) { return h("li", {}, icon("info"), h("span", { text: r })); }))
        : null;
      var error = S.modelErrors[fit.id] ? h("div", { class: "model-error", role: "alert", text: S.modelErrors[fit.id] }) : null;
      return h("div", { class: "mrow" + (disabled ? " is-disabled" : "") + (inUse ? " is-active" : ""), role: "listitem" },
        h("div", { class: "mrow-head" },
          h("div", { class: "mrow-title" },
            h("span", { class: "mrow-name", text: dash(fit.label) }),
            h("span", { class: "mrow-size", text: sizeText(fit) || "-" }),
            badges
          ),
          action
        ),
        h("p", { class: "mrow-about", text: dash(fit.maker) + ". " + dash(fit.about) }),
        modelMeta(fit, busy, { page: openSummaryModelPage, del: askDeleteSummaryModel }), why, detail, error
      );
    }

    // appended by renderModels, below the speech models
    // off unless the user turned it on or picked a summary model
    function summariesOn() {
      return !!(S.config && S.config.summaries === true);
    }

    function renderSummaryModels() {
      summaryRefs = {};
      modelPanel.appendChild(h("h3", { class: "group-title", id: "summary-models", text: "Summary model" }));
      modelPanel.appendChild(h("p", { class: "footnote", text: "Optional. A summary model writes an overview, what each person said and the action items for a note. Notes work without one." }));
      var sw = toggle({
        label: "Summaries",
        value: summariesOn,
        onChange: function (v) {
          Promise.resolve(patchConfig({ summaries: v })).then(function () {
            renderModels();
            if (S.openNote && S.openStatus === "done") renderNoteView();
          });
        }
      });
      modelPanel.appendChild(card([settingRow("Summaries", function () {
        return summariesOn()
          ? "Shows Summarize on finished notes. A summary runs only when you click it."
          : "Off: notes show no Summarize button and no summary model runs. Downloading a model below turns it on.";
      }, sw.el)]));
      var list = h("div", { class: "card summary-list" + (summariesOn() ? "" : " is-off"), role: "list", "aria-label": "Summary models" });
      S.summaryModels.forEach(function (fit) { list.appendChild(summaryRow(fit)); });
      modelPanel.appendChild(list);
      modelPanel.appendChild(h("p", { class: "footnote", text: "Recommended fits this computer's memory and graphics. Summaries are written on this computer; the note never leaves it." }));
    }

    function showSummaryModels() {
      showTab("model");
      var head = document.getElementById("summary-models");
      if (head) head.scrollIntoView({ block: "start" });
    }

    function requestSummaryModel(fit) {
      if (fit.downloaded) {
        chooseSummaryModel(fit);
        return;
      }
      var size = sizeText(fit);
      askConfirm({
        title: "Download " + dash(fit.label) + "?",
        body: (size ? size + " will be downloaded from " : "It will be downloaded from ") + dash(fit.maker) + "'s page on Hugging Face, checked against a fixed checksum, and kept on " + thisDevice() + ". After that, summaries need no network.",
        confirm: size ? "Download " + size : "Download",
        onConfirm: function () { chooseSummaryModel(fit); }
      });
    }

    function chooseSummaryModel(fit) {
      if (!fit.supported || S.progress[fit.id]) return;
      delete S.modelErrors[fit.id];
      if (!fit.downloaded) S.progress[fit.id] = { downloaded: null, total: null };
      renderModels();
      Promise.resolve(invoke("choose_summary_model", { id: fit.id })).then(function () {
        if (fit.downloaded) return refreshState();
      }).catch(function (err) {
        delete S.progress[fit.id];
        S.modelErrors[fit.id] = errText(err);
        renderModels();
      });
    }

    function openSummaryModelPage(fit) {
      Promise.resolve(invoke("open_summary_model_page", { id: fit.id })).catch(function (err) { toast(errText(err)); });
    }

    function askDeleteSummaryModel(fit) {
      var size = sizeText(fit);
      askConfirm({
        title: "Delete " + dash(fit.label) + "?",
        body: (size ? "This frees " + size + " on " : "This removes it from ") + thisDevice() + ". Summaries you already have stay.",
        confirm: "Delete",
        danger: true,
        onConfirm: function () {
          delete S.modelErrors[fit.id];
          Promise.resolve(invoke("delete_summary_model", { id: fit.id })).then(function () {
            toast(dash(fit.label) + " deleted.");
            return refreshState();
          }).catch(function (err) {
            S.modelErrors[fit.id] = errText(err);
            renderModels();
          });
        }
      });
    }

    // the summary as plain lines, for Copy note
    function summaryText(note) {
      var sum = note.summary;
      var lines = ["Summary", sum.overview || ""];
      (sum.by_speaker || []).forEach(function (p) {
        lines.push("", speakerName(p.speaker, note) + ":");
        (p.points || []).forEach(function (point) { lines.push("- " + point); });
      });
      if ((sum.action_items || []).length) {
        lines.push("", "Action items:");
        sum.action_items.forEach(function (a) {
          lines.push("- " + a.task + " (" + (a.owner ? speakerName(a.owner, note) : "Unassigned") + ", " + clock(a.at_ms) + ")");
        });
      }
      return lines;
    }

    function summaryCard(note) {
      var box = h("section", { class: "card note-summary", "aria-label": "Summary" });
      var head = h("div", { class: "sum-head" }, h("h3", { class: "sum-title", text: "Summary" }));
      box.appendChild(head);
      var mine = S.summarizing === note.id;
      if (mine) {
        var step = S.summaryStep;
        var part = step && typeof step.total === "number" && step.total > 0
          ? "Summarizing: step " + Math.min(step.done + 1, step.total) + " of " + step.total + "..."
          : "Summarizing...";
        box.appendChild(h("p", { class: "sum-busy", role: "status", text: part }));
        box.appendChild(h("p", { class: "footnote", text: "This runs on this computer and can take a few minutes for a long call." }));
        return box;
      }
      var model = summaryModel();
      var label = note.summary ? "Summarize Again" : "Summarize";
      if (model) {
        head.appendChild(h("button", { class: note.summary ? "btn" : "btn btn-action", type: "button", text: label,
          disabled: !!S.summarizing, onclick: function () { summarize(note.id); } }));
      } else {
        head.appendChild(h("button", { class: "btn", type: "button", text: "Choose a Summary Model", onclick: showSummaryModels }));
      }
      if (!note.summary) {
        box.appendChild(h("p", { class: "sum-empty", text: model
          ? "An overview, what each person said and the action items, written on this computer with " + dash(model.label) + "."
          : "Download a summary model in the Model tab to summarize notes on this computer." }));
        return box;
      }
      var sum = note.summary;
      box.appendChild(h("p", { class: "sum-overview", text: sum.overview || "-" }));
      if ((sum.by_speaker || []).length) {
        box.appendChild(h("h4", { class: "sum-sub", text: "Who said what" }));
        var people = h("div", { class: "sum-people" });
        sum.by_speaker.forEach(function (p) {
          people.appendChild(h("div", { class: "sum-person" },
            h("div", { class: "sum-name", text: speakerName(p.speaker, note) }),
            h("ul", { class: "sum-list" }, (p.points || []).map(function (point) { return h("li", { text: point }); }))
          ));
        });
        box.appendChild(people);
      }
      box.appendChild(h("h4", { class: "sum-sub", text: "Action items" }));
      var items = sum.action_items || [];
      box.appendChild(items.length
        ? h("ul", { class: "sum-list sum-actions" }, items.map(function (a) {
          return h("li", {},
            h("span", { text: a.task }),
            h("span", { class: "sum-owner", text: (a.owner ? speakerName(a.owner, note) : "Unassigned") + ", " + clock(a.at_ms) })
          );
        }))
        : h("p", { class: "sum-empty", text: "None found." }));
      var made = (S.summaryModels || []).filter(function (f) { return f.id === sum.model; })[0];
      box.appendChild(h("p", { class: "footnote", text: "Written on this computer by " + (made ? made.label : "a summary model") + ". Check it against the transcript below." }));
      return box;
    }

    function summarize(id) {
      if (S.summarizing) return;
      S.summarizing = id;
      S.summaryStep = null;
      renderNoteView();
      Promise.resolve(invoke("summarize_note", { id: id })).catch(function (err) {
        S.summarizing = null;
        toast("Couldn't summarize: " + errText(err));
        renderNoteView();
      });
    }

    // ---------- Settings ----------

    var settingsPanel = panels.settings;

    function syncSettings() {
      applyAppearance();
      syncers.forEach(function (fn) { fn(); });
      OB.syncers.forEach(function (fn) { fn(); });
      if (S.insightsSync) S.insightsSync(); // the Insights switch lives outside the settings syncers
    }

    // a function description is re-read on every sync, for text that depends on another setting
    function settingRow(title, description, control, extra) {
      var desc = description ? h("div", { class: "srow-desc" }) : null;
      if (typeof description === "function") {
        desc.textContent = description();
        sink.push(function () { desc.textContent = description(); });
      } else if (desc) {
        desc.textContent = description;
      }
      return h("div", { class: "srow" },
        h("div", { class: "srow-text" },
          h("div", { class: "srow-title", text: title }),
          desc
        ),
        h("div", { class: "srow-control" }, control),
        extra || null
      );
    }

    function card(rows) {
      return h("div", { class: "card" }, rows);
    }

    function addSync(control) {
      sink.push(control.sync);
      return control.el;
    }

    // builds with controls registering into list, then restores the Settings list
    function withSink(list, build) {
      var saved = sink;
      sink = list;
      try { return build(); } finally { sink = saved; }
    }

    function cfg() {
      return S.config;
    }

    function langOptions() {
      return [{ value: "auto", label: "Auto-detect" }].concat(LANGUAGES);
    }

    // the controls below serve both Settings and the Customize step of setup
    function pushRow() {
      return settingRow("Push to talk", "Hold to talk, release to paste.", addSync(hotkeyControl("hold")));
    }

    function outputLanguageRow() {
      return settingRow("Output language", "English translates what you say and can shorten long speech. Spoken language keeps every word.", addSync(segmented({
        role: "radiogroup", label: "Output language",
        items: [{ value: "english", label: "English" }, { value: "spoken", label: "Spoken language" }],
        value: function () { return cfg().translate ? "english" : "spoken"; },
        onChange: function (v) { patchConfig({ translate: v === "english" }); }
      })));
    }

    function spokenLanguageRow() {
      return settingRow("Spoken language", "Auto-detect works for most speech.", addSync(select({
        label: "Spoken language", options: langOptions,
        value: function () { return cfg().language; },
        onChange: function (v) { patchConfig({ language: v }); }
      })));
    }

    function pasteRow() {
      return settingRow("Paste into the focused app", "Off copies to the clipboard only.", addSync(toggle({
        label: "Paste into the focused app",
        value: function () { return cfg().paste; },
        onChange: function (v) { patchConfig({ paste: v }); }
      })));
    }

    function smartFormatRow() {
      return settingRow("Smart formatting", "Turns spoken lists (\"point one, point two\" or \"first, second\") into numbered lists, removes \"uh\" and \"um\", and fixes spacing.", addSync(toggle({
        label: "Smart formatting",
        value: function () { return cfg().smart_format; },
        onChange: function (v) { patchConfig({ smart_format: v }); }
      })));
    }

    function saveHistoryRow() {
      return settingRow("Save history", "Off keeps nothing on disk and deletes what is saved.", addSync(toggle({
        label: "Save history",
        value: function () { return cfg().save_history; },
        onChange: function (v) {
          if (v) patchConfig({ save_history: true });
          else askTurnOffHistory();
        }
      })));
    }

    function startAtLoginRow() {
      return settingRow("Start at login", "Opens local-stt when you log in. Turning it on installs local-stt first if needed.", addSync(toggle({
        label: "Start at login",
        value: function () { return typeof S.autostartEnabled === "boolean" ? S.autostartEnabled : cfg().autostart; },
        onChange: function (v) { patchConfig({ autostart: v }); }
      })));
    }

    // saves on change (blur or Enter) and shows the app's refusal under the field
    function nameControl() {
      var input = h("input", { class: "ob-input srow-input", type: "text", maxlength: "60", placeholder: "Your name", autocomplete: "off", spellcheck: "false", "aria-label": "Your name" });
      var error = h("div", { class: "field-error", role: "alert", hidden: true });
      input.addEventListener("change", function () {
        var name = input.value.trim();
        if (name === savedName()) {
          error.hidden = true;
          return;
        }
        Promise.resolve(invoke("set_config", { patch: { user_name: name } })).then(function (saved) {
          if (saved && typeof saved === "object") S.config = saved;
          error.hidden = true;
          syncSettings();
        }).catch(function (err) {
          error.textContent = errText(err);
          error.hidden = false;
        });
      });
      function sync() {
        if (document.activeElement !== input) input.value = savedName();
      }
      sync();
      return { el: h("div", { class: "hk-wrap" }, input, error), sync: sync };
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
      var turnOff = slot === "toggle" ? h("button", { class: "btn btn-quiet", type: "button", text: "Turn Off", "aria-label": "Turn off hands-free", onclick: function () {
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

    // the right macOS needs for the shortcut and paste, with a way straight to the setting
    function accessControl() {
      var status = h("span", { class: "tag" });
      var open = h("button", { class: "btn", type: "button", text: "Open Settings", onclick: function () {
        Promise.resolve(invoke("open_accessibility_settings")).catch(function (err) { toast(errText(err)); });
      } });
      function sync() {
        var ok = S.accessibility === true;
        status.className = "tag " + (ok ? "tag-green" : "tag-red");
        status.textContent = ok ? "Allowed" : "Not allowed";
        open.hidden = ok;
      }
      sync();
      return { el: h("div", { class: "srow-controls" }, status, open), sync: sync };
    }

    function accessDescription() {
      return S.accessibility === true
        ? "The shortcut, paste and call names in notes work."
        : "Needed for the shortcut, paste and call names in notes. Already on? Remove local-stt with the minus button and add it again.";
    }

    // quits, installs again (the newest version when one is out), and reopens
    function reinstallControl() {
      var busy = false;
      var button = h("button", { class: "btn", type: "button", onclick: askReinstall });
      function sync() {
        button.disabled = busy;
        button.textContent = busy ? "Reinstalling..." : "Reinstall...";
      }
      function askReinstall() {
        if (busy) return;
        clear(dialog);
        dialog.appendChild(h("h2", { class: "dlg-title", id: "dlg-title", text: "Reinstall local-stt?" }));
        var mac = S.platform !== "windows";
        var intro = "local-stt quits, installs again (the newest version when one is out) and opens again." + (mac ? " macOS then asks for its permissions afresh." : "");
        var body = h("p", { class: "dlg-body", text: intro });
        dialog.appendChild(body);
        var wipe = h("input", { type: "checkbox", class: "check", id: "reinstall-clean" });
        dialog.appendChild(h("label", { class: "dlg-check", for: "reinstall-clean" }, wipe, h("span", { text: "Also delete my settings, history, notes, insights, voice fingerprint and logs. Downloaded models stay." })));
        var cancel = h("button", { class: "btn", type: "button", text: "Cancel", onclick: function () { closeDialog(); } });
        var go = h("button", { class: "btn btn-action", type: "button", text: "Reinstall", onclick: function () {
          var clean = wipe.checked;
          closeDialog();
          busy = true;
          sync();
          Promise.resolve(invoke("reinstall_app", { clean: clean })).then(function (message) {
            toast(typeof message === "string" && message !== "" ? message : "Reinstalled. local-stt opens again in a moment.");
          }).catch(function (err) {
            busy = false;
            sync();
            toast("Couldn't reinstall: " + errText(err));
          });
        } });
        // deleting data turns the confirm red, like every other delete
        wipe.addEventListener("change", function () {
          body.textContent = intro + (wipe.checked ? " Setup runs again afterwards." : "");
          go.className = "btn " + (wipe.checked ? "btn-danger-solid" : "btn-action");
          go.textContent = wipe.checked ? "Delete and Reinstall" : "Reinstall";
        });
        dialog.appendChild(h("div", { class: "dlg-actions" }, cancel, go));
        if (typeof dialog.showModal === "function") dialog.showModal();
        else dialog.setAttribute("open", "");
        cancel.focus();
      }
      sync();
      return { el: button, sync: sync };
    }

    // saves a report in the app's folder and shows it in Finder or Explorer; nothing is sent anywhere
    function reportControl() {
      var busy = false;
      var button = h("button", { class: "btn", type: "button", onclick: function () {
        if (busy) return;
        busy = true;
        sync();
        Promise.resolve(invoke("export_diagnostics")).then(function (message) {
          toast(typeof message === "string" && message !== "" ? message : "Report saved.");
        }).catch(function (err) {
          toast("Couldn't save the report: " + errText(err));
        }).then(function () {
          busy = false;
          sync();
        });
      } });
      function sync() {
        button.disabled = busy;
        button.textContent = busy ? "Saving..." : "Export Report";
      }
      sync();
      return { el: button, sync: sync };
    }

    // the button reads Checking... until the app answers with a notice or update-available
    function checkControl() {
      var button = h("button", { class: "btn", type: "button", onclick: function () {
        if (S.checkingUpdates) return;
        S.checkingUpdates = true;
        syncSettings();
        clearTimeout(checkTimer);
        checkTimer = setTimeout(finishCheck, 30000); // an unanswered check must not leave the button stuck
        Promise.resolve(invoke("check_for_updates")).catch(function (err) {
          finishCheck();
          toast("Couldn't check for updates: " + errText(err));
        });
      } });
      function sync() {
        button.textContent = S.checkingUpdates ? "Checking..." : "Check for Updates";
        button.disabled = S.checkingUpdates;
      }
      sync();
      return { el: button, sync: sync };
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

    // Light or Dark pins the window's look; System leaves it to the OS
    function applyAppearance() {
      var a = S.config && S.config.appearance;
      var doc = document.documentElement;
      if (a === "light" || a === "dark") doc.dataset.scheme = a;
      else delete doc.dataset.scheme;
    }

    // a tiny window drawn in one look, for the Appearance tiles
    function miniWindow(look) {
      return h("span", { class: "look-win look-" + look, "aria-hidden": "true" },
        h("span", { class: "look-bar" }, h("i"), h("i"), h("i")),
        h("span", { class: "look-body" },
          h("span", { class: "look-side" }),
          h("span", { class: "look-lines" }, h("i", { class: "look-accent" }), h("i"), h("i"))
        )
      );
    }

    function appearanceControl() {
      var system = S.platform === "windows" ? "Matches Windows" : "Matches macOS";
      var cards = [
        { value: "system", name: "System", note: system },
        { value: "light", name: "Light", note: "Always light" },
        { value: "dark", name: "Dark", note: "Always dark" }
      ];
      var group = h("div", { class: "themes looks", role: "radiogroup", "aria-label": "Appearance" });
      var current = function () { return (S.config && S.config.appearance) || "system"; };
      var buttons = cards.map(function (c) {
        var stage = h("div", { class: "theme-stage look-stage" },
          c.value === "system"
            ? h("span", { class: "look-split" }, miniWindow("light"), miniWindow("dark"))
            : miniWindow(c.value));
        var button = h("button", { class: "theme-card", type: "button", role: "radio" },
          stage,
          h("span", { class: "theme-name", text: c.name }),
          h("span", { class: "theme-note", text: c.note })
        );
        button.addEventListener("click", function () {
          if (current() !== c.value) patchConfig({ appearance: c.value });
        });
        group.appendChild(button);
        return button;
      });
      function sync() {
        buttons.forEach(function (button, i) {
          var on = current() === cards[i].value;
          button.setAttribute("aria-checked", on ? "true" : "false");
          button.tabIndex = on ? 0 : -1;
        });
      }
      group.addEventListener("keydown", function (event) {
        var at = cards.findIndex(function (c) { return c.value === current(); });
        var next = stepTarget(event, at, cards.length);
        if (next === -1) return;
        event.preventDefault();
        buttons[next].focus();
        patchConfig({ appearance: cards[next].value });
      });
      sync();
      return { el: group, sync: sync };
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
      var reset = h("button", { class: "btn", type: "button", text: "Reset Position", onclick: function () {
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
      function sync() { move.textContent = S.moving ? "Done" : "Move Pill"; }
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
      sink = syncers;
      var micOptions = function () {
        return [{ value: "", label: "System default" }].concat(S.inputs.map(function (name) { return { value: name, label: name }; }));
      };

      var freeKey = hotkeyControl("toggle");
      syncers.push(freeKey.sync);

      var dictation = card([
        pushRow(),
        settingRow("Hands-free", "Press once to start listening, again to stop. Adding the extra key while holding push to talk switches to hands-free.", freeKey.el),
        outputLanguageRow(),
        spokenLanguageRow()
      ]);

      var outputCard = card([
        pasteRow(),
        smartFormatRow(),
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
      var looks = appearanceControl();
      syncers.push(looks.sync);
      var appearanceCard = card([h("div", { class: "srow srow-stack" }, looks.el)]);
      var overlayCard = card([
        settingRow("Show overlay", "A small indicator while you dictate.", addSync(toggle({
          label: "Show overlay",
          value: function () { return cfg().show_overlay; },
          onChange: function (v) { patchConfig({ show_overlay: v }); }
        }))),
        settingRow("Live transcription", function () {
          return cfg().show_overlay === false ? "Needs the overlay. Turn on Show overlay to use it." : "Shows your words above the pill while you speak.";
        }, addSync(toggle({
          label: "Live transcription",
          value: function () { return cfg().live_transcription; },
          disabled: function () { return cfg().show_overlay === false; },
          onChange: function (v) { patchConfig({ live_transcription: v }); }
        }))),
        h("div", { class: "srow srow-stack" }, h("div", { class: "srow-title", text: "Theme" }), themes.el),
        settingRow("Pill position", "Show the pill, then drag it where you like. It snaps to the corners.", addSync(pillPositionControl()))
      ]);

      var general = card([
        S.platform === "macos" && S.accessibility !== null ? settingRow("Accessibility", accessDescription, addSync(accessControl())) : null,
        saveHistoryRow(),
        installRow(),
        startAtLoginRow(),
        settingRow("Your name", "Used for greetings. Stays on " + thisDevice() + ".", addSync(nameControl())),
        settingRow("Learn my voice", "Lets notes tell you apart from people in the room. A voice fingerprint, never audio, stays on this computer. Off deletes it.", addSync(toggle({
          label: "Learn my voice",
          value: function () { return cfg().learn_my_voice !== false; },
          onChange: function (v) { patchConfig({ learn_my_voice: v }); }
        }))),
        settingRow("Run Setup Again", "Walks through setup again. Your settings stay as they are.", h("button", { class: "btn", type: "button", text: "Run Setup Again", onclick: openOnboarding })),
        settingRow("Updates", "local-stt " + dash(S.version), addSync(checkControl())),
        settingRow("Check automatically", "Looks for a new version once a day. Only the request is sent.", addSync(toggle({
          label: "Check automatically",
          value: function () { return cfg().check_updates; },
          onChange: function (v) { patchConfig({ check_updates: v }); }
        })))
      ]);

      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Appearance" }));
      settingsPanel.appendChild(appearanceCard);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Dictation" }));
      settingsPanel.appendChild(dictation);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Output" }));
      settingsPanel.appendChild(outputCard);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Overlay" }));
      settingsPanel.appendChild(overlayCard);
      var troubleshooting = card([
        settingRow("Troubleshooting log", "Keeps app events like starts, errors and timings on this computer. Never your words, audio or key presses. Off deletes it.", addSync(toggle({
          label: "Troubleshooting log",
          value: function () { return cfg().diagnostic_log !== false; },
          onChange: function (v) { patchConfig({ diagnostic_log: v }); }
        }))),
        settingRow("Diagnostic report", "Saves a text file with app, system and settings details and the log, to share when something goes wrong. Read it before you share it.", addSync(reportControl())),
        settingRow("Reinstall", "Fixes a broken install: installs local-stt again, resets its permissions and opens setup to allow them, keeping your settings and history unless you choose otherwise.", addSync(reinstallControl()))
      ]);

      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Notes" }));
      settingsPanel.appendChild(card([settingRow("Echo cancellation", "Takes the call's sound out of your microphone when you use speakers. Turn off if your own words go missing from notes.", addSync(toggle({
        label: "Echo cancellation",
        value: function () { return cfg().echo_cancellation !== false; },
        onChange: function (v) { patchConfig({ echo_cancellation: v }); }
      })))]));
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "General" }));
      settingsPanel.appendChild(general);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Troubleshooting" }));
      settingsPanel.appendChild(troubleshooting);
      settingsPanel.appendChild(h("p", { class: "footnote", text: "local-stt " + dash(S.version) + ". Audio is processed on this computer and is never uploaded." }));
      syncSettings();
    }

    // ---------- Insights ----------

    var WEEKDAYS = { Mon: "Monday", Tue: "Tuesday", Wed: "Wednesday", Thu: "Thursday", Fri: "Friday", Sat: "Saturday", Sun: "Sunday" };
    var DAY_STATES = {
      active: { say: "active", icon: "check" },
      frozen: { say: "covered by a freeze", icon: "freeze" },
      missed: { say: "missed", icon: "minus" },
      today: { say: "today", icon: null },
      upcoming: { say: "still to come", icon: null },
      rest: { say: "rest day", icon: "minus" },
      bonus: { say: "bonus", icon: "plus" }
    };
    var MIX = [
      { key: "noun", label: "Nouns", cls: "mix-noun" },
      { key: "verb", label: "Verbs", cls: "mix-verb" },
      { key: "adjective", label: "Adjectives", cls: "mix-adj" },
      { key: "adverb", label: "Adverbs", cls: "mix-adv" }
    ];
    var insightsPanel = panels.insights;

    function isNum(v) {
      return typeof v === "number" && isFinite(v);
    }

    function num(v) {
      return isNum(v) ? v.toLocaleString("en-US") : "-";
    }

    function percent(v) {
      return isNum(v) ? Math.round(v * 100) + "%" : "-";
    }

    // minutes read better as hours once they pass 60
    function duration(minutes) {
      if (!isNum(minutes)) return "-";
      if (minutes < 60) return num(Math.round(minutes)) + " min";
      return (Math.round(minutes / 6) / 10).toLocaleString("en-US") + " hours";
    }

    function shortDate(iso) {
      var m = typeof iso === "string" ? iso.match(/^(\d{4})-(\d{2})-(\d{2})$/) : null;
      if (!m) return "-";
      return new Date(+m[1], +m[2] - 1, +m[3]).toLocaleDateString("en-US", { day: "numeric", month: "short", year: "numeric" });
    }

    function stat(value, label) {
      return h("div", { class: "stat" }, h("div", { class: "stat-value", text: value }), h("div", { class: "stat-label", text: label }));
    }

    function insightsTitle(text) {
      return h("h3", { class: "group-title", text: text });
    }

    function weekRow(week) {
      var row = h("div", { class: "week" });
      for (var i = 0; i < 7; i++) {
        var day = Array.isArray(week) && week[i] ? week[i] : {};
        var info = DAY_STATES[day.state] || null;
        var name = WEEKDAYS[day.weekday] || dash(day.weekday);
        var dot = h("span", { class: "week-dot" }, info && info.icon ? icon(info.icon) : null);
        row.appendChild(h("div", { class: "week-cell state-" + (info ? day.state : "unknown"), role: "img", "aria-label": name + ", " + (info ? info.say : "no data") },
          h("span", { class: "week-name", "aria-hidden": "true", text: dash(day.weekday) }), dot));
      }
      return row;
    }

    // stays open across Insights refreshes until closed
    var freezeHelpOpen = false;

    // the rules as streak.rs applies them; freezes come from all-time active workdays, not the current streak
    var FREEZE_RULES = [
      "A freeze covers a missed workday, so your streak keeps going. It's used for you automatically.",
      "An active workday is a Monday to Friday with at least one dictation.",
      "You start with 5 freezes. Every 10 active workdays earns one more, up to 10.",
      "Weekends never count and never break a streak.",
      "If you miss more workdays than you have freezes, the streak restarts and you keep your freezes."
    ];

    function nextFreezeText(s) {
      if (isNum(s.nextFreezeIn)) return "+1 freeze after " + num(s.nextFreezeIn) + " more active " + (s.nextFreezeIn === 1 ? "workday" : "workdays");
      if (isNum(s.freezes) && s.freezes === s.freezeCap) return "Freezes are full: " + num(s.freezeCap) + " is the most you can hold";
      return null;
    }

    function freezesStat(s) {
      var help = h("div", { class: "freeze-help", id: "freeze-help" },
        h("div", { class: "freeze-help-title", text: "How freezes work" }),
        h("ul", {}, FREEZE_RULES.map(function (rule) { return h("li", { text: rule }); })));
      help.hidden = !freezeHelpOpen;
      var btn = h("button", { class: "icon-btn info-btn", type: "button", "aria-label": "How freezes work", "aria-controls": "freeze-help", "aria-expanded": String(freezeHelpOpen), onclick: function () {
        freezeHelpOpen = !freezeHelpOpen;
        help.hidden = !freezeHelpOpen;
        btn.setAttribute("aria-expanded", String(freezeHelpOpen));
      } }, icon("info"));
      var cell = h("div", { class: "stat" },
        h("div", { class: "stat-value", text: num(s.freezes) + " of " + num(s.freezeCap) }),
        h("div", { class: "stat-label stat-label-info" }, h("span", { text: "Freezes" }), btn));
      return { cell: cell, help: help };
    }

    function streakCard(v) {
      var s = v.streak || {};
      var text = nextFreezeText(s);
      var next = text ? h("div", { class: "ins-line", text: text }) : null;
      var freezes = freezesStat(s);
      return h("div", { class: "card ins-card" },
        h("div", { class: "streak-hero" },
          h("span", { class: "streak-num", text: num(s.current) }),
          h("span", { class: "streak-unit", text: "workdays" })),
        h("div", { class: "stats" },
          stat(num(s.longest), "Longest streak"),
          freezes.cell),
        next,
        freezes.help,
        weekRow(v.week));
    }

    function hoursChart(hours) {
      var values = Array.isArray(hours) && hours.length === 24 ? hours : null;
      var peak = 0;
      if (values) values.forEach(function (n) { if (isNum(n) && n > peak) peak = n; });
      var bars = h("div", { class: "hours", role: "img", "aria-label": values && peak > 0 ? "Words by hour of the day. Busiest hour: " + values.indexOf(peak) + ":00." : "Words by hour of the day. No data yet." });
      for (var i = 0; i < 24; i++) {
        var bar = h("i", { class: "hour" });
        var n = values ? values[i] : null;
        // a 2 px floor keeps an empty hour visible as a baseline
        bar.style.height = (isNum(n) && peak > 0 ? Math.max(2, Math.round(n / peak * 64)) : 2) + "px";
        bars.appendChild(bar);
      }
      var axis = h("div", { class: "hours-axis", "aria-hidden": "true" });
      ["0:00", "6:00", "12:00", "18:00", "23:00"].forEach(function (t) { axis.appendChild(h("span", { text: t })); });
      return h("div", { class: "card ins-card" }, bars, axis);
    }

    function wordList(title, items) {
      var list = h("ol", { class: "wlist" });
      var rows = Array.isArray(items) ? items : [];
      rows.forEach(function (item) {
        list.appendChild(h("li", {}, h("span", { class: "wlist-word", text: dash(item && item.word) }), h("span", { class: "wlist-count", text: num(item && item.count) })));
      });
      if (rows.length === 0) list.appendChild(h("li", {}, h("span", { class: "wlist-word", text: "-" })));
      return h("div", { class: "wcol" }, h("div", { class: "wcol-title", text: title }), list);
    }

    function mixBar(mix) {
      var bar = h("div", { class: "mix", role: "img" });
      var legend = h("ul", { class: "mix-legend" });
      var spoken = [];
      MIX.forEach(function (part) {
        var value = mix ? mix[part.key] : null;
        if (isNum(value) && value > 0) {
          var seg = h("i", { class: "mix-seg " + part.cls });
          seg.style.width = value * 100 + "%";
          bar.appendChild(seg);
        }
        spoken.push(part.label + " " + percent(value));
        legend.appendChild(h("li", {}, h("span", { class: "mix-dot " + part.cls, "aria-hidden": "true" }), part.label + " " + percent(value)));
      });
      bar.setAttribute("aria-label", "Part of speech mix. " + spoken.join(", "));
      return h("div", { class: "mixwrap" }, bar, legend);
    }

    function wordsCard(w) {
      return h("div", { class: "card ins-card" },
        h("div", { class: "wcols" }, wordList("Top nouns", w.nouns), wordList("Top verbs", w.verbs), wordList("Top words", w.top)),
        mixBar(w.mix));
    }

    function milestonesCard(list) {
      var card = h("div", { class: "card" });
      (Array.isArray(list) ? list : []).forEach(function (m) {
        card.appendChild(h("div", { class: "srow ins-row" },
          h("div", { class: "srow-title", text: num(m && m.words) + " words" }),
          h("div", { class: "ins-date", text: shortDate(m && m.reachedOn) })));
      });
      return card;
    }

    function askResetInsights() {
      askConfirm({
        title: "Reset Insights?",
        body: "Your streak, freezes, word counts and milestones will be removed from " + thisDevice() + ". This can't be undone.",
        confirm: "Reset",
        danger: true,
        onConfirm: function () {
          Promise.resolve(invoke("reset_insights")).then(loadInsights).catch(function (err) { toast("Couldn't reset: " + errText(err)); });
        }
      });
    }

    function insightsFooter() {
      var sw = toggle({
        label: "Word insights",
        value: function () { return !(S.config && S.config.word_insights === false); },
        onChange: function (v) { patchConfig({ word_insights: v }).then(loadInsights); }
      });
      sw.el.id = "ins-words-sw";
      S.insightsSync = sw.sync;
      return h("div", { class: "card" },
        h("div", { class: "ins-privacy", text: "Everything here stays on " + thisDevice() + "." }),
        settingRow("Word insights", "Counts the words you use most. Off deletes the counts.", sw.el),
        settingRow("Reset Insights", "Removes your streak, freezes, word counts and milestones.",
          h("button", { class: "btn btn-danger", id: "ins-reset", type: "button", text: "Reset Insights...", onclick: askResetInsights })));
    }

    function renderInsights() {
      var v = S.insights;
      var focusId = insightsPanel.contains(document.activeElement) ? document.activeElement.id : "";
      var top = scroller.scrollTop;
      clear(insightsPanel);
      if (!v) {
        if (S.insightsError) {
          insightsPanel.appendChild(emptyState("Couldn't load", S.insightsError, h("button", { class: "btn", type: "button", text: "Try Again", onclick: loadInsights })));
        }
        return;
      }
      if (v.empty === true) {
        insightsPanel.appendChild(emptyState("No insights yet", "Dictate once to start your first streak."));
      } else {
        var t = v.totals || {};
        var sp = v.speed || {};
        insightsPanel.appendChild(insightsTitle("Workday streak"));
        insightsPanel.appendChild(streakCard(v));
        insightsPanel.appendChild(insightsTitle("Words"));
        insightsPanel.appendChild(h("div", { class: "card ins-card" }, h("div", { class: "stats stats-4" },
          stat(num(t.wordsToday), "Today"), stat(num(t.wordsWeek), "This week"), stat(num(t.wordsAll), "All time"), stat(num(t.dictationsAll), "Dictations"))));
        insightsPanel.appendChild(insightsTitle("Speed and time saved"));
        insightsPanel.appendChild(h("div", { class: "card ins-card" },
          h("div", { class: "stats stats-3" },
            stat(isNum(sp.wpm) ? num(sp.wpm) + " wpm" : "-", "Speaking speed"),
            stat(duration(sp.minutesSavedWeek), "Time saved this week"),
            stat(duration(sp.minutesSavedAll), "Time saved all time")),
          h("div", { class: "ins-line", text: "Estimated against typing at " + num(sp.typingWpm) + " words a minute" })));
        insightsPanel.appendChild(insightsTitle("When you dictate"));
        insightsPanel.appendChild(hoursChart(v.hours));
        if (v.words) {
          insightsPanel.appendChild(insightsTitle("Your words"));
          insightsPanel.appendChild(wordsCard(v.words));
          insightsPanel.appendChild(h("p", { class: "footnote", text: "Approximate, English only" }));
        }
        insightsPanel.appendChild(insightsTitle("Milestones"));
        insightsPanel.appendChild(milestonesCard(v.milestones));
      }
      insightsPanel.appendChild(h("div", { class: "ins-gap" }));
      insightsPanel.appendChild(insightsFooter());
      scroller.scrollTop = top;
      var back = focusId ? document.getElementById(focusId) : null;
      if (back) back.focus();
    }

    function loadInsights() {
      var seq = ++S.insightsSeq;
      return Promise.resolve(invoke("get_insights")).then(function (view) {
        if (seq !== S.insightsSeq) return;
        S.insights = view && typeof view === "object" ? view : null;
        S.insightsError = S.insights ? "" : "No insights came back.";
        renderInsights();
      }).catch(function (err) {
        if (seq !== S.insightsSeq) return;
        if (!S.insights) {
          S.insightsError = errText(err);
          renderInsights();
        } else {
          toast(errText(err));
        }
      });
    }

    // ---------- Onboarding ----------

    var OB_STEPS = 5;
    var obView = h("section", { class: "ob", "aria-label": "Setup", hidden: true });
    root.appendChild(obView);

    function savedName() {
      return S.config && typeof S.config.user_name === "string" ? S.config.user_name : "";
    }

    function openOnboarding() {
      OB.on = true;
      OB.name = savedName();
      OB.nameError = "";
      OB.custom = false;
      OB.rows = null;
      OB.dl = null;
      OB.mic = { state: "idle", level: 0, error: "" };
      OB.access = "idle";
      OB.run += 1;
      stopMoving();
      toolbar.hidden = true;
      scroller.hidden = true;
      root.classList.add("is-onboarding");
      obView.hidden = false;
      obGo(1);
    }

    function closeOnboarding() {
      OB.on = false;
      OB.run += 1;
      obView.hidden = true;
      clear(obView);
      toolbar.hidden = false;
      scroller.hidden = false;
      root.classList.remove("is-onboarding");
      showTab("history");
      if (S.historyStale) loadHistory();
      document.getElementById("tab-" + S.tab).focus();
    }

    // saves the name and marks setup done; a name the app rejects sends the user back to step 1
    function obFinish() {
      var name = OB.name.trim();
      return Promise.resolve(invoke("set_config", { patch: { setup_done: true, user_name: name } })).then(function (saved) {
        if (saved && typeof saved === "object") S.config = saved;
        closeOnboarding();
        toast("local-stt lives in the " + (S.platform === "windows" ? "system tray" : "menu bar"));
      }).catch(function (err) {
        OB.nameError = errText(err);
        obGo(1);
      });
    }

    function obGo(step) {
      OB.step = step;
      OB.syncers = [];
      OB.refs = {};
      OB.primary = null;
      clear(obView);
      var body = [obWelcome, obCheck, obChoice, obPerms, obTry][step - 1]();
      var head = h("header", { class: "ob-head" },
        h("div", { class: "ob-step", text: "Step " + step + " of " + OB_STEPS }),
        h("button", { class: "linklike ob-skip", type: "button", text: "Skip Setup", onclick: obFinish })
      );
      var title = h("h1", { class: "ob-title", tabindex: "-1", id: "ob-title", text: body.title });
      obView.appendChild(h("div", { class: "ob-page" }, head, title, body.el, obActions(body)));
      obSyncContinue();
      if (body.focus) body.focus.focus();
      else title.focus();
      obView.scrollTop = 0;
    }

    function obActions(body) {
      var back = OB.step > 1 ? h("button", { class: "btn", type: "button", text: "Back", onclick: function () {
        if (OB.step === 3 && OB.custom) {
          OB.custom = false;
          obGo(3);
        } else {
          obGo(OB.step - 1);
        }
      } }) : null;
      var buttons = (body.buttons || []).map(function (b) {
        var node = h("button", { class: "btn" + (b.primary ? " btn-action" : ""), type: "button", text: b.label, disabled: b.disabled, onclick: b.onClick });
        if (b.primary) OB.primary = node;
        if (b.ref) OB.refs[b.ref] = node;
        return node;
      });
      return h("div", { class: "ob-actions" }, h("div", { class: "ob-actions-start" }, back), h("div", { class: "ob-actions-end" }, buttons));
    }

    // Enter presses the primary button, except in a multi-line box or on another button
    obView.addEventListener("keydown", function (event) {
      if (event.key !== "Enter" || event.isComposing || !OB.primary || OB.primary.disabled) return;
      var tag = event.target.tagName;
      if (tag === "BUTTON" || tag === "TEXTAREA" || tag === "SELECT" || tag === "A") return;
      event.preventDefault();
      OB.primary.click();
    });

    // ----- step 1 -----

    function obWelcome() {
      var input = h("input", { class: "ob-input", type: "text", id: "ob-name", maxlength: "60", placeholder: "Your name", autocomplete: "off", spellcheck: "false", "aria-describedby": "ob-name-hint" });
      input.value = OB.name;
      var error = h("div", { class: "field-error ob-error", role: "alert", text: OB.nameError, hidden: OB.nameError === "" });
      input.addEventListener("input", function () {
        OB.name = input.value;
        OB.nameError = "";
        error.hidden = true;
      });
      function next() {
        var name = OB.name.trim();
        OB.name = name;
        input.value = name;
        if (name === savedName()) return obGo(2);
        Promise.resolve(invoke("set_config", { patch: { user_name: name } })).then(function (saved) {
          if (saved && typeof saved === "object") S.config = saved;
          obGo(2);
        }).catch(function (err) {
          OB.nameError = errText(err);
          error.textContent = OB.nameError;
          error.hidden = false;
          input.focus();
        });
      }
      var el = h("div", { class: "card ob-card" },
        h("label", { class: "ob-label", for: "ob-name", text: "What should we call you?" }),
        input,
        h("div", { class: "ob-hint", id: "ob-name-hint", text: "Optional. Stays on " + thisDevice() + "." }),
        error
      );
      return {
        title: "Dictate anywhere. Your voice stays on " + thisDevice() + ".",
        el: el,
        buttons: [
          { label: "Skip", onClick: function () { OB.name = ""; input.value = ""; next(); } },
          { label: "Continue", primary: true, onClick: next }
        ]
      };
    }

    // ----- step 2 -----

    var ROW_MIN_MS = 300;

    function obRows() {
      var rows = [
        { id: "hw", title: "Checking your computer", doing: "Reading the chip, memory and disk" },
        { id: "model", title: "Choosing a speech model", doing: "Comparing models with " + thisDevice() },
        { id: "mic", title: "Finding a microphone", doing: "Looking for microphones" }
      ];
      if (S.platform === "macos" && S.accessibility !== null) rows.push({ id: "access", title: "Checking Accessibility", doing: "Asking macOS what local-stt may do" });
      rows.push({ id: "download", title: "Downloading the speech model", doing: "Starting the download" });
      rows.forEach(function (r) { r.state = "waiting"; r.detail = ""; });
      return rows;
    }

    function obRow(id) {
      return OB.rows ? OB.rows.filter(function (r) { return r.id === id; })[0] : null;
    }

    function setRow(row, state, detail) {
      row.state = state;
      row.detail = detail;
      paintRow(row);
      if (state === "done" || state === "needs" || state === "failed") announce(row.title + ": " + ROW_STATES[state].label + ". " + detail);
    }

    var ROW_STATES = {
      waiting: { label: "Waiting" },
      running: { label: "Running" },
      done: { label: "Done" },
      needs: { label: "Needs You" },
      failed: { label: "Failed" }
    };

    function rowNode(row) {
      var state = h("span", { class: "ob-state" });
      var detail = h("div", { class: "ob-detail" });
      var retry = h("button", { class: "btn btn-quiet ob-retry", type: "button", text: "Try Again", hidden: true, onclick: function () { retryRow(row); } });
      var node = h("li", { class: "ob-row", "data-row": row.id }, h("div", { class: "ob-row-main" }, h("div", { class: "ob-row-title", text: row.title }), detail), state, retry);
      row.node = { li: node, state: state, detail: detail, retry: retry };
      paintRow(row);
      return node;
    }

    function paintRow(row) {
      var n = row.node;
      if (!n) return;
      n.li.dataset.state = row.state;
      clear(n.state);
      if (row.state === "running") n.state.appendChild(h("i", { class: "spin", "aria-hidden": "true" }));
      if (row.state === "done") n.state.appendChild(icon("check"));
      if (row.state === "needs") n.state.appendChild(icon("info"));
      n.state.appendChild(document.createTextNode(ROW_STATES[row.state].label));
      n.detail.textContent = row.state === "running" && row.detail === "" ? row.doing : row.detail;
      n.retry.hidden = row.state !== "failed";
      obSyncContinue();
    }

    function hardwareText() {
      var hw = S.hardware || {};
      var memory = typeof hw.ram_gb === "number" ? hw.ram_gb + " GB memory" : "memory -";
      var disk = typeof hw.free_disk_gb === "number" ? Math.round(hw.free_disk_gb) + " GB free" : "free disk -";
      return dash(hw.cpu) + ", " + memory + ", " + disk;
    }

    // the model setup uses: the loaded one when it is on disk, else the recommended one
    function setupModel() {
      var active = S.models.filter(function (m) { return m.id === S.activeModel && m.downloaded; })[0];
      if (active) return active;
      var pick = S.models.filter(function (m) { return m.recommended && m.supported; })[0];
      return pick || S.models.filter(function (m) { return m.supported; })[0] || null;
    }

    function dlText(info) {
      var known = info && typeof info.total === "number" && info.total > 0 && typeof info.downloaded === "number";
      return known ? Math.round(info.downloaded / 1048576) + " of " + mb(info.total) : "Starting the download";
    }

    // shows Running for ROW_MIN_MS so rows do not flicker, then reads the live result
    function runRow(row, run, work) {
      setRow(row, "running", "");
      return new Promise(function (resolve) {
        setTimeout(resolve, ROW_MIN_MS);
      }).then(function () {
        if (run !== OB.run) return;
        var result = work();
        setRow(row, result.state, result.detail);
      });
    }

    function startChecks() {
      var run = ++OB.run;
      OB.rows = obRows();
      OB.dl = { id: null, status: "none", info: null, announced: 0 };
      paintAllRows();
      Promise.resolve(invoke("get_state")).then(function (state) {
        if (run !== OB.run) return null;
        applyState(state);
        syncSettings();
        var model = setupModel();
        planDownload(model);
        var chain = Promise.resolve();
        OB.rows.forEach(function (row) {
          chain = chain.then(function () {
            if (run !== OB.run) return null;
            return runRow(row, run, function () { return evalRow(row, model); });
          });
        });
        return chain;
      }).catch(function (err) {
        if (run !== OB.run) return;
        var first = OB.rows[0];
        setRow(first, "failed", errText(err));
      });
    }

    function paintAllRows() {
      if (OB.rows) OB.rows.forEach(paintRow);
    }

    function retryRow(row) {
      if (row.id === "download") {
        var model = setupModel();
        row.state = "running";
        row.detail = "";
        paintRow(row);
        planDownload(model);
        applyDownloadRow();
      } else {
        startChecks();
      }
    }

    function evalRow(row, model) {
      if (row.id === "hw") return { state: "done", detail: hardwareText() };
      if (row.id === "model") {
        return model ? { state: "done", detail: dash(model.label) + " fits " + thisDevice() } : { state: "needs", detail: "No speech model fits " + thisDevice() + "." };
      }
      if (row.id === "mic") {
        var n = S.inputs.length;
        return n > 0 ? { state: "done", detail: n === 1 ? "1 microphone found" : n + " microphones found" } : { state: "needs", detail: "No microphone found. Connect one, then try the microphone test." };
      }
      if (row.id === "access") {
        return S.accessibility === true ? { state: "done", detail: "Allowed" } : { state: "needs", detail: "You'll allow this in a moment" };
      }
      return downloadResult(row);
    }

    // decides what the download row follows; starts a download only when nothing is running or on disk
    function planDownload(model) {
      var dl = OB.dl;
      dl.announced = 0;
      if (S.downloading) {
        dl.id = S.downloading;
        dl.status = "running";
        dl.info = S.progress[S.downloading] || null;
      } else if (!model) {
        dl.status = "none";
      } else if (model.downloaded) {
        dl.id = model.id;
        dl.status = "already";
      } else {
        dl.id = model.id;
        dl.status = "running";
        dl.info = null;
        S.progress[model.id] = { downloaded: null, total: null };
        Promise.resolve(invoke("choose_model", { id: model.id })).catch(function (err) {
          delete S.progress[model.id];
          dl.status = "failed";
          dl.error = errText(err);
          applyDownloadRow();
        });
      }
    }

    function downloadResult() {
      var dl = OB.dl;
      if (dl.status === "already") return { state: "done", detail: "Already downloaded" };
      if (dl.status === "done") return { state: "done", detail: "Downloaded" };
      if (dl.status === "failed") return { state: "failed", detail: dl.error || "The download failed." };
      if (dl.status === "none") return { state: "needs", detail: "Nothing to download." };
      return { state: "running", detail: dlText(dl.info) };
    }

    // pushes the live download state into its row once that row has had its turn
    function applyDownloadRow() {
      var row = obRow("download");
      if (!row || row.state === "waiting") return;
      var result = downloadResult();
      if (result.state === "running") {
        row.state = "running";
        row.detail = result.detail;
        paintRow(row);
      } else {
        setRow(row, result.state, result.detail);
      }
      if (OB.step === 5) obTryDownload();
    }

    function obDownloadProgress(p) {
      var dl = OB.dl;
      if (!OB.on || !dl || p.id !== dl.id) return;
      dl.status = "running";
      dl.info = { downloaded: p.downloaded, total: p.total };
      var pct = p.total > 0 ? Math.floor((p.downloaded / p.total) * 4) : 0;
      if (pct > dl.announced && pct < 4) announce("Downloading the speech model, " + pct * 25 + " percent.");
      dl.announced = Math.max(dl.announced, pct);
      applyDownloadRow();
    }

    function obDownloadDone(p) {
      var dl = OB.dl;
      if (!OB.on || !dl || p.id !== dl.id) return;
      dl.status = p.ok === false ? "failed" : "done";
      dl.error = p.error || "";
      applyDownloadRow();
    }

    function obSyncContinue() {
      var go = OB.refs.next;
      if (!go || !OB.rows) return;
      var ready = OB.rows.every(function (r) { return r.id === "download" || r.state === "done" || r.state === "needs"; });
      go.disabled = !ready;
    }

    function obCheck() {
      if (!OB.rows) startChecks();
      var list = h("ul", { class: "card ob-list", "aria-label": "System check" });
      OB.rows.forEach(function (row) { list.appendChild(rowNode(row)); });
      var name = OB.name.trim();
      return {
        title: name ? "Setting things up for you, " + name : "Setting things up for you",
        el: list,
        buttons: [{ label: "Continue", primary: true, ref: "next", disabled: true, onClick: function () { obGo(3); } }]
      };
    }

    // ----- step 3 -----

    function yesNo(on) {
      return on ? "On" : "Off";
    }

    function recommendedList() {
      var model = setupModel();
      var cfgNow = S.config || {};
      var items = [
        ["Speech model", dash(model && model.label)],
        ["Push to talk", keycaps(slotParts("hold"))],
        ["Output", cfgNow.translate === false ? "Spoken language" : "English"],
        ["Smart formatting", yesNo(cfgNow.smart_format !== false)],
        ["Paste into the focused app", yesNo(cfgNow.paste !== false)],
        ["Save history", yesNo(cfgNow.save_history !== false)],
        ["Start at login", yesNo(typeof S.autostartEnabled === "boolean" ? S.autostartEnabled : cfgNow.autostart === true)]
      ];
      var list = h("ul", { class: "ob-facts", "aria-label": "Recommended settings" });
      items.forEach(function (item) {
        list.appendChild(h("li", {}, h("span", { class: "ob-fact-name", text: item[0] }), h("span", { class: "ob-fact-value" }, item[1])));
      });
      return list;
    }

    function obChoice() {
      if (OB.custom) return obCustomize();
      var recommended = h("div", { class: "card ob-choice" },
        h("h2", { class: "ob-choice-title", text: "Use Recommended Settings" }),
        h("p", { class: "ob-note", text: "Works well on " + thisDevice() + ". Nothing changes when you pick this." }),
        recommendedList(),
        h("button", { class: "btn btn-action", type: "button", text: "Use Recommended Settings", onclick: function () { obGo(4); } })
      );
      var customize = h("div", { class: "card ob-choice" },
        h("h2", { class: "ob-choice-title", text: "Customize" }),
        h("p", { class: "ob-note", text: "Pick the shortcut, language, paste, formatting, history and start at login yourself. Everything else stays in Settings." }),
        h("button", { class: "btn", type: "button", text: "Customize", onclick: function () { OB.custom = true; obGo(3); } })
      );
      return { title: "How do you want to set up?", el: h("div", { class: "ob-choices" }, recommended, customize), buttons: [] };
    }

    function obCustomize() {
      var rows = withSink(OB.syncers, function () {
        return [pushRow(), outputLanguageRow(), spokenLanguageRow(), pasteRow(), smartFormatRow(), saveHistoryRow(), startAtLoginRow()];
      });
      OB.syncers.forEach(function (fn) { fn(); });
      return {
        title: "Customize your setup",
        el: card(rows),
        buttons: [{ label: "Continue", primary: true, onClick: function () { obGo(4); } }]
      };
    }

    // ----- step 4 -----

    function levelBar() {
      var bar = h("i");
      var track = h("div", { class: "track ob-level", role: "progressbar", "aria-label": "Microphone level", "aria-valuemin": "0", "aria-valuemax": "100", "aria-valuenow": "0" }, bar);
      return { track: track, bar: bar };
    }

    function permCard(title, text, ref) {
      var status = h("div", { class: "ob-perm-status", role: "status" });
      var actions = h("div", { class: "ob-perm-actions" });
      var el = h("div", { class: "card ob-perm", "data-perm": ref }, h("h2", { class: "ob-choice-title", text: title }), h("p", { class: "ob-note", text: text }), status, actions);
      return { el: el, status: status, actions: actions };
    }

    function obPerms() {
      var cards = [];
      var mic = permCard("Microphone", "Needed to hear you. Audio stays on " + thisDevice() + " and is never saved.", "mic");
      OB.refs.mic = mic;
      mic.level = levelBar();
      mic.test = h("button", { class: "btn btn-action", type: "button", text: "Test Microphone", onclick: testMicrophone });
      mic.skip = h("button", { class: "btn btn-quiet", type: "button", text: "Skip For Now", onclick: function () { OB.mic.state = "skipped"; paintMic(); } });
      mic.actions.appendChild(mic.test);
      mic.actions.appendChild(mic.skip);
      mic.status.appendChild(mic.level.track);
      mic.line = h("div", { class: "ob-line" });
      mic.status.appendChild(mic.line);
      cards.push(mic.el);
      paintMic();

      if (S.platform === "macos" && S.accessibility !== null) {
        var ax = permCard("Accessibility", "Needed for the shortcut and to paste into the app you are using. Notes also use it to read who is in a Zoom call.", "access");
        OB.refs.ax = ax;
        ax.open = h("button", { class: "btn btn-action", type: "button", text: "Open Settings", onclick: function () {
          OB.access = "waiting";
          paintAccess();
          Promise.resolve(invoke("open_accessibility_settings")).catch(function (err) { toast(errText(err)); });
        } });
        ax.skip = h("button", { class: "btn btn-quiet", type: "button", text: "Skip For Now", onclick: function () { OB.access = "skipped"; paintAccess(); } });
        ax.actions.appendChild(ax.open);
        ax.actions.appendChild(ax.skip);
        ax.line = h("div", { class: "ob-line" });
        ax.status.appendChild(ax.line);
        cards.push(ax.el);
        paintAccess();
      }
      return {
        title: S.platform === "macos" ? "Allow what local-stt needs" : "Check your microphone",
        el: h("div", { class: "ob-choices" }, cards),
        buttons: [{ label: "Continue", primary: true, onClick: function () { obGo(5); } }]
      };
    }

    function tagLine(kind, text) {
      return h("span", { class: "tag tag-" + kind, text: text });
    }

    function paintMic() {
      var m = OB.refs.mic;
      if (!m) return;
      var s = OB.mic.state;
      var hint = S.platform === "macos" ? "Open System Settings > Privacy & Security > Microphone and turn on local-stt." : "Check that your microphone is connected and not muted.";
      clear(m.line);
      m.level.track.hidden = s !== "testing";
      m.test.disabled = s === "testing";
      m.test.textContent = s === "testing" ? "Listening..." : s === "heard" || s === "silent" || s === "error" ? "Try Again" : "Test Microphone";
      m.test.classList.toggle("btn-action", s !== "heard");
      m.skip.hidden = s === "testing" || s === "heard" || s === "skipped";
      if (s === "testing") m.line.appendChild(document.createTextNode("Listening... Say something."));
      if (s === "heard") m.line.appendChild(tagLine("green", "Heard you"));
      if (s === "silent") m.line.appendChild(h("div", {}, tagLine("red", "Didn't hear anything"), h("div", { class: "ob-note", text: hint })));
      if (s === "error") m.line.appendChild(h("div", { role: "alert" }, tagLine("red", "Couldn't test"), h("div", { class: "ob-note", text: OB.mic.error })));
      if (s === "skipped") m.line.appendChild(tagLine("grey", "Skipped for now"));
    }

    function paintMicLevel() {
      var m = OB.refs.mic;
      if (!m) return;
      var level = Math.max(0, Math.min(1, OB.mic.level));
      m.level.bar.style.transform = "scaleX(" + level.toFixed(3) + ")";
      m.level.track.setAttribute("aria-valuenow", String(Math.round(level * 100)));
    }

    function testMicrophone() {
      if (OB.mic.state === "testing") return;
      OB.mic = { state: "testing", level: 0, error: "" };
      paintMic();
      paintMicLevel();
      announce("Listening for 2 seconds. Say something.");
      Promise.resolve(invoke("check_microphone")).then(function (result) {
        OB.mic.state = result && result.heard === true ? "heard" : "silent";
        paintMic();
        announce(OB.mic.state === "heard" ? "Heard you." : "Didn't hear anything.");
      }).catch(function (err) {
        OB.mic.state = "error";
        OB.mic.error = errText(err);
        paintMic();
      });
    }

    function paintAccess() {
      var a = OB.refs.ax;
      if (!a) return;
      var allowed = S.accessibility === true;
      clear(a.line);
      a.open.hidden = allowed;
      a.skip.hidden = allowed || OB.access === "skipped";
      if (allowed) a.line.appendChild(tagLine("green", "Allowed"));
      else if (OB.access === "waiting") a.line.appendChild(h("span", { class: "ob-waiting", text: "Waiting for you to turn on local-stt" }));
      else if (OB.access === "skipped") a.line.appendChild(tagLine("grey", "Skipped for now"));
    }

    // ----- step 5 -----

    function obTry() {
      var el = h("div", { class: "ob-try" });
      OB.refs.try = el;
      var step = { title: "Try it", el: el, buttons: [{ label: "Skip", onClick: obFinish }, { label: "Finish", primary: true, onClick: obFinish }] };
      step.focus = paintTry();
      return step;
    }

    function tryBlocked() {
      return S.platform === "macos" && S.accessibility === false;
    }

    // repaints the Try it body for the current permission and download state; returns the node to focus
    function paintTry() {
      var el = OB.refs.try;
      clear(el);
      var keys = keycaps(slotParts("hold"));
      OB.tryBlocked = tryBlocked();
      if (OB.tryBlocked) {
        el.appendChild(h("div", { class: "card ob-perm" },
          h("h2", { class: "ob-choice-title", text: "Allow Accessibility first" }),
          h("p", { class: "ob-note", text: "The shortcut and paste need it." }),
          h("div", { class: "ob-perm-actions" }, h("button", { class: "btn btn-action", type: "button", text: "Open Settings", onclick: function () {
            Promise.resolve(invoke("open_accessibility_settings")).catch(function (err) { toast(errText(err)); });
          } })),
          h("div", { class: "ob-line" }, h("span", { class: "ob-waiting", text: "Waiting for you to turn on local-stt" }))
        ));
        return null;
      }
      var downloading = OB.dl ? OB.dl.status === "running" : !!S.downloading;
      var failed = !!OB.dl && OB.dl.status === "failed";
      var box = h("textarea", { class: "ob-box", id: "ob-box", rows: "4", placeholder: "Your words appear here", "aria-labelledby": "ob-title ob-prompt", disabled: downloading || failed });
      var prompt = h("p", { class: "ob-prompt", id: "ob-prompt" }, "Hold ", keys, " and say something.");
      var success = h("div", { class: "ob-success", role: "status", hidden: true }, icon("check"), h("span", {}, "That's it. Hold ", keycaps(slotParts("hold")), " in any app."));
      box.addEventListener("input", function () {
        var had = !success.hidden;
        success.hidden = box.value.trim() === "";
        if (!had && !success.hidden) announce("That's it. Hold " + slotParts("hold").join(" plus ") + " in any app.");
      });
      el.appendChild(prompt);
      el.appendChild(box);
      el.appendChild(success);
      OB.refs.box = box;
      if (failed) {
        el.appendChild(h("p", { class: "ob-note ob-error", role: "alert", text: "The speech model did not download. Pick a model in the Model tab once setup is done." }));
        return null;
      }
      if (downloading) {
        var track = h("div", { class: "track", role: "progressbar", "aria-label": "Download progress", "aria-valuemin": "0", "aria-valuemax": "100" }, h("i"));
        var line = h("div", { class: "progress-text", role: "status" });
        OB.refs.dl = { track: track, bar: track.firstChild, line: line };
        el.appendChild(h("div", { class: "ob-dl" }, h("p", { class: "ob-note", text: "The speech model is still downloading. You can try it as soon as it is ready." }), track, line));
        obTryDownload();
        return null;
      }
      return box;
    }

    function obTryDownload() {
      var ref = OB.refs.dl;
      var dl = OB.dl;
      if (OB.step !== 5 || !OB.refs.try) return;
      if (!ref || dl.status !== "running") {
        // the download finished while this step was open
        var focus = paintTry();
        if (focus) focus.focus();
        return;
      }
      var info = dl.info;
      var known = info && typeof info.total === "number" && info.total > 0 && typeof info.downloaded === "number";
      ref.bar.style.transform = "scaleX(" + (known ? Math.min(1, info.downloaded / info.total) : 0).toFixed(4) + ")";
      ref.line.textContent = known ? dlText(info) : "Starting the download";
    }

    // permission changes arrive through get_state; repaint whatever depends on them
    function obSync() {
      if (!OB.on) return;
      var row = obRow("access");
      if (row && row.state === "needs" && S.accessibility === true) setRow(row, "done", "Allowed");
      if (OB.step === 4) paintAccess();
      if (OB.step === 5 && OB.refs.try && OB.tryBlocked !== tryBlocked()) {
        var focus = paintTry();
        if (focus) focus.focus();
      }
    }

    // ---------- render ----------

    function renderAll() {
      if (!S.loaded) {
        [panels.history, panels.notes, panels.insights, panels.model, panels.settings].forEach(function (p) { clear(p); });
        if (S.loadError) {
          panels[S.tab].appendChild(emptyState("Couldn't load", S.loadError,
            h("button", { class: "btn", type: "button", text: "Try Again", onclick: refreshState })));
        }
        return;
      }
      if (!panels.history.contains(historyBar)) {
        clear(panels.history);
        panels.history.appendChild(historyBar);
        panels.history.appendChild(historyPick.bar);
        panels.history.appendChild(historyList);
      }
      if (!panels.notes.contains(notesListView)) {
        clear(panels.notes);
        panels.notes.appendChild(notesListView);
        panels.notes.appendChild(noteView);
      }
      renderNotesList();
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
      obDownloadProgress(p);
    });

    subscribe("model-done", function (p) {
      if (!p || !p.id) return;
      delete S.progress[p.id];
      if (S.loadingModel === p.id) S.loadingModel = "";
      if (p.ok === false) {
        S.modelErrors[p.id] = p.error || "The download failed. The previous model is still active.";
        toast(S.modelErrors[p.id]);
      }
      obDownloadDone(p);
      refreshState();
    });

    subscribe("summary-model-progress", function (p) {
      if (!p || !p.id) return;
      S.progress[p.id] = { downloaded: p.downloaded, total: p.total };
      if (summaryRefs[p.id]) paintProgress(p.id);
      else if (S.loaded) renderModels();
    });

    subscribe("summary-model-done", function (p) {
      if (!p || !p.id) return;
      delete S.progress[p.id];
      if (p.ok === false) {
        S.modelErrors[p.id] = p.error || "The download failed.";
        toast(S.modelErrors[p.id]);
      } else {
        toast("Summary model ready.");
      }
      refreshState();
    });

    subscribe("note-summary-progress", function (p) {
      if (!p || p.id !== S.summarizing) return;
      S.summaryStep = { done: p.done, total: p.total };
      var busy = noteView.querySelector(".sum-busy");
      if (busy && S.openNote && S.openNote.id === p.id && typeof p.total === "number" && p.total > 0) {
        busy.textContent = "Summarizing: step " + Math.min(p.done + 1, p.total) + " of " + p.total + "...";
      }
    });

    subscribe("note-summary", function (p) {
      if (!p || typeof p.id !== "string") return;
      S.summarizing = null;
      S.summaryStep = null;
      if (p.ok === false) toast("Couldn't summarize: " + (p.error || "the model stopped."));
      else announce("Summary ready.");
      if (S.openNote && S.openNote.id === p.id) openNote(p.id);
    });

    subscribe("mic-check-level", function (p) {
      if (!p || OB.mic.state !== "testing") return;
      OB.mic.level = typeof p.level === "number" ? p.level : 0;
      paintMicLevel();
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

    // the screenshot demo drives the window through this; unknown names are ignored
    subscribe("show-tab", function (p) {
      var known = p && TABS.some(function (t) { return t.value === p.tab; });
      if (known) showTab(p.tab);
      if (known && p.tab === "notes" && p.newNote === true) newNote();
    });

    subscribe("note-state", onNoteState);
    subscribe("note-segment", onNoteSegment);
    subscribe("notes-changed", function () {
      if (S.tab === "notes") loadNotes();
      else S.noteListStale = true;
    });

    subscribe("notice", function (p) {
      if (p && p.kind === "update") finishCheck(); // a manual check ends with this notice when nothing is new
      if (p && typeof p.message === "string" && p.message !== "") toast(p.message);
    });

    subscribe("update-available", function (p) {
      if (!p || typeof p !== "object") return;
      S.update = p;
      S.hiddenUpdate = "";
      S.offeredUpdate = typeof p.latest === "string" ? p.latest : ""; // the app only sends this when it wants it shown
      S.updateError = "";
      finishCheck();
      renderBanner();
      if (updateVisible()) announce("local-stt " + p.latest + " is available.");
    });

    subscribe("show-update", showUpdate);

    subscribe("insights-changed", function () {
      if (S.tab === "insights") loadInsights();
    });
    subscribe("permissions-changed", function () {
      reloadState();
      announce("Accessibility is allowed. The shortcut works now.");
    });

    subscribe("history-changed", function () {
      if (S.tab === "history") loadHistory();
      else S.historyStale = true;
    });

    // ---------- start ----------

    refreshState().then(function () {
      if (S.loaded) {
        loadHistory();
        loadNotes();
        if (S.config && S.config.setup_done === false && !S.demo) openOnboarding();
        Promise.resolve(invoke("take_update_notice")).then(function (updated) {
          if (updated === true) celebrateUpdate();
        }).catch(function () { /* an older app without the command */ });
      }
    });

    return {
      destroy: function () {
        unlisten.forEach(function (fn) { fn(); });
        unlisten = [];
        clearTimeout(toastTimer);
        clearTimeout(checkTimer);
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
