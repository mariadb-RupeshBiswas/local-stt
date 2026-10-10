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
      openNote: null, // the whole note shown in the Notes tab, or null for the list
      openStatus: "done", // recording, tidying or done
      progress: {},
      modelErrors: {},
      activeModel: null, // the model actually loaded, which config.model may not be
      downloading: null,
      loadingModel: "",
      autostartEnabled: null,
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
      if (name === "history" && S.historyStale) loadHistory();
    }

    // ---------- keyboard shortcuts ----------

    // Cmd on macOS, Ctrl on Windows: 1 to 4 pick a tab, F searches the history, W closes the window
    function onShortcut(event) {
      var mod = S.platform === "windows" ? event.ctrlKey : event.metaKey;
      if (!mod || event.altKey || event.shiftKey || S.capturingSlot || dialog.open) return;
      var tab = { "1": "history", "2": "notes", "3": "model", "4": "settings" }[event.key];
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
      S.update = state.update && typeof state.update === "object" ? state.update : null;
      if (state.notes && typeof state.notes === "object") {
        S.notes = {
          supported: state.notes.supported !== false,
          reason: typeof state.notes.reason === "string" ? state.notes.reason : null,
          active: state.notes.active && typeof state.notes.active.id === "string" ? state.notes.active : null
        };
      }
      if (S.downloading && !S.progress[S.downloading]) S.progress[S.downloading] = { downloaded: null, total: null };
      if (S.loadingModel && S.loadingModel === S.activeModel) S.loadingModel = "";
      S.loaded = true;
      S.loadError = "";
      root.dataset.platform = S.platform;
      renderBanner();
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

    // ---------- Update banner ----------

    function updateVisible() {
      var u = S.update;
      if (!(u && u.available === true && typeof u.latest === "string" && u.latest !== "" && u.latest !== S.hiddenUpdate)) return false;
      return S.offeredUpdate === u.latest || !(S.config && S.config.skip_update === u.latest);
    }

    function installButton() {
      return banner.querySelector(".btn-install");
    }

    function renderBanner() {
      var show = updateVisible();
      banner.hidden = !show;
      root.classList.toggle("has-banner", show);
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
      (note.segments || []).forEach(function (seg) {
        lines.push(speakerName(seg.speaker) + " (" + clock(seg.start_ms) + "): " + seg.text);
      });
      return lines.join("\n");
    }

    function speakerName(id) {
      return id === "me" ? "Me" : id === "others" ? "Others" : String(id || "-");
    }

    function turnRow(seg) {
      var row = h("div", { class: "turn turn-" + (seg.speaker === "me" ? "me" : "others") },
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

    // a function description is re-read on every sync, for text that depends on another setting
    function settingRow(title, description, control, extra) {
      var desc = description ? h("div", { class: "srow-desc" }) : null;
      if (typeof description === "function") {
        desc.textContent = description();
        syncers.push(function () { desc.textContent = description(); });
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
        settingRow("Smart formatting", "Turns spoken \"point one, point two\" into a numbered list, removes \"uh\" and \"um\", and fixes spacing.", addSync(toggle({
          label: "Smart formatting",
          value: function () { return cfg().smart_format; },
          onChange: function (v) { patchConfig({ smart_format: v }); }
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
        }))),
        settingRow("Updates", "local-stt " + dash(S.version), addSync(checkControl())),
        settingRow("Check automatically", "Looks for a new version once a day. Only the request is sent.", addSync(toggle({
          label: "Check automatically",
          value: function () { return cfg().check_updates; },
          onChange: function (v) { patchConfig({ check_updates: v }); }
        })))
      ]);

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
        settingRow("Diagnostic report", "Saves a text file with app, system and settings details and the log, to share when something goes wrong. Read it before you share it.", addSync(reportControl()))
      ]);

      settingsPanel.appendChild(h("h3", { class: "group-title", text: "General" }));
      settingsPanel.appendChild(general);
      settingsPanel.appendChild(h("h3", { class: "group-title", text: "Troubleshooting" }));
      settingsPanel.appendChild(troubleshooting);
      settingsPanel.appendChild(h("p", { class: "footnote", text: "local-stt " + dash(S.version) + ". Audio is processed on this computer and is never uploaded." }));
      syncSettings();
    }

    // ---------- render ----------

    function renderAll() {
      if (!S.loaded) {
        [panels.history, panels.notes, panels.model, panels.settings].forEach(function (p) { clear(p); });
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

    subscribe("history-changed", function () {
      if (S.tab === "history") loadHistory();
      else S.historyStale = true;
    });

    // ---------- start ----------

    refreshState().then(function () {
      if (S.loaded) {
        loadHistory();
        loadNotes();
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
