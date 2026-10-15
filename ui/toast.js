// The "call detected" toast: Start note, or close; it leaves on its own unless the pointer rests on it.
(function () {
  function el(tag, className, text) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  function mountToast(root, tauri) {
    var invoke = tauri && tauri.core && tauri.core.invoke ? tauri.core.invoke : function () { return Promise.resolve(); };
    var box = el("div", "toast");
    box.setAttribute("role", "alert");
    var title = el("div", "toast-title", "Call detected");
    var sub = el("div", "toast-sub", "Take notes on this call?");
    var text = el("div", "toast-text");
    text.append(title, sub);
    var start = el("button", "toast-start", "Start note");
    start.type = "button";
    var close = el("button", "toast-close", "×");
    close.type = "button";
    close.setAttribute("aria-label", "Dismiss");
    box.append(el("span", "toast-dot"), text, start, close);
    box.hidden = true;
    root.appendChild(box);

    var deadline = 0;
    var paused = false;
    var remaining = 0;
    var tick = null;

    function stop() {
      clearInterval(tick);
      tick = null;
      box.hidden = true;
    }

    function away() {
      stop();
      invoke("toast_dismiss").catch(function () {});
    }

    function show(payload) {
      var call = payload && typeof payload.call === "string" ? payload.call : "";
      title.textContent = call ? call + " call detected" : "Call detected";
      var scheme = payload && payload.theme;
      if (scheme === "light" || scheme === "dark") document.documentElement.setAttribute("data-scheme", scheme);
      else document.documentElement.removeAttribute("data-scheme");
      box.hidden = false;
      paused = false;
      deadline = Date.now() + (payload && payload.showMs > 0 ? payload.showMs : 20000);
      if (!tick) tick = setInterval(function () { if (!paused && Date.now() >= deadline) away(); }, 200);
    }

    // resting the pointer on the toast holds it; leaving gives back what was left, at least a moment
    box.addEventListener("mouseenter", function () {
      paused = true;
      remaining = deadline - Date.now();
    });
    box.addEventListener("mouseleave", function () {
      paused = false;
      deadline = Date.now() + Math.max(1500, remaining);
    });
    start.addEventListener("click", function () {
      stop();
      invoke("toast_start_note").catch(function () {});
    });
    close.addEventListener("click", away);
    document.addEventListener("keydown", function (event) { if (event.key === "Escape" && !box.hidden) away(); });

    if (tauri && tauri.event && tauri.event.listen) {
      tauri.event.listen("toast", function (event) { show(event.payload); });
    }
    return { show: show };
  }

  window.mountToast = mountToast;
  if (!window.__LSTT_PREVIEW__) mountToast(document.body, window.__TAURI__);
})();
