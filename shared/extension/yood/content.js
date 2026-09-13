// Yood companion extension (Brave app mode).
// - Alt+L: small floating bar (top center) for YouTube links / IDs.
// - Download buttons inside YouTube's own button rows, with Yood's icon
//   (watch pages AND Shorts).
// - YouTube-styled download dialog: mode, quality, container, subtitles
//   (+language, incl. auto-translate), advanced yt-dlp options.
// - Ad blocking layers (cosmetic + request + player-response + skip):
//   Brave Shields stays on, declarativeNetRequest blocks dedicated ad
//   hosts at the network level (see rules.json), and this script hides
//   ad renderers, aborts ad fetches, strips ad payloads from YouTube's
//   player/next API responses, and fast-forwards/skips in-stream ads.
// Clicking Download hands everything to the Yood desktop app, which
// downloads via yt-dlp. No data collection, no remote code.
(function () {
  "use strict";
  if (window.__YOOD_EXT__) return;
  window.__YOOD_EXT__ = true;

  var MARK = "data-yood-download";
  var CARD_SELECTOR = "ytd-video-renderer, ytd-grid-video-renderer, ytd-rich-item-renderer, ytd-compact-video-renderer, ytd-playlist-video-renderer, ytd-reel-item-renderer";
  var DOWNLOAD_SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"><path d="M12.5535 16.5061C12.4114 16.6615 12.2106 16.75 12 16.75C11.7894 16.75 11.5886 16.6615 11.4465 16.5061L7.44648 12.1311C7.16698 11.8254 7.18822 11.351 7.49392 11.0715C7.79963 10.792 8.27402 10.8132 8.55352 11.1189L11.25 14.0682V3C11.25 2.58579 11.5858 2.25 12 2.25C12.4142 2.25 12.75 2.58579 12.75 3V14.0682L15.4465 11.1189C15.726 10.8132 16.2004 10.792 16.5061 11.0715C16.8118 11.351 16.833 11.8254 16.5535 12.1311L12.5535 16.5061Z" fill="currentColor"></path><path d="M3.75 15C3.75 14.5858 3.41422 14.25 3 14.25C2.58579 14.25 2.25 14.5858 2.25 15V15.0549C2.24998 16.4225 2.24996 17.5248 2.36652 18.3918C2.48754 19.2919 2.74643 20.0497 3.34835 20.6516C3.95027 21.2536 4.70814 21.5125 5.60825 21.6335C6.47522 21.75 7.57754 21.75 8.94513 21.75H15.0549C16.4225 21.75 17.5248 21.75 18.3918 21.6335C19.2919 21.5125 20.0497 21.2536 20.6517 20.6516C21.2536 20.0497 21.5125 19.2919 21.6335 18.3918C21.75 17.5248 21.75 16.4225 21.75 15.0549V15C21.75 14.5858 21.4142 14.25 21 14.25C20.5858 14.25 20.25 14.5858 20.25 15C20.25 16.4354 20.2484 17.4365 20.1469 18.1919C20.0482 18.9257 19.8678 19.3142 19.591 19.591C19.3142 19.8678 18.9257 20.0482 18.1919 20.1469C17.4365 20.2484 16.4354 20.25 15 20.25H9C7.56459 20.25 6.56347 20.2484 5.80812 20.1469C5.07435 20.0482 4.68577 19.8678 4.40901 19.591C4.13225 19.3142 3.9518 18.9257 3.85315 18.1919C3.75159 17.4365 3.75 16.4354 3.75 15Z" fill="currentColor"></path></svg>';

  // ================= Ad blocking (Brave app mode) =================
  // Layer 1 (network) lives in rules.json (declarativeNetRequest).
  // Layers 2-5 live here: cosmetic CSS, fetch/XHR abort, player-response
  // ad-payload stripping, and DOM removal of ad renderers. All lists below
  // only cover dedicated ad/tracking hosts and YouTube ad paths — never
  // content CDNs (googlevideo.com, ytimg.com, gstatic.com), so a mistake
  // can at worst leave an ad visible, never break playback.
  var AD_TOKENS = [
    "doubleclick.net",
    "googleadservices.com",
    "googlesyndication.com",
    "adservice.google.com",
    "jnn-pa.googleapis.com",
    "youtube.com/api/stats/ads",
    "youtube.com/pagead",
    "youtube.com/ptracking",
    "youtube.com/get_midroll_info",
    "youtube.com/ad_break"
  ];
  var AD_RENDERER_SELECTOR = [
    "ytd-ad-slot-renderer",
    "ytd-in-feed-ad-layout-renderer",
    "ytd-display-ad-renderer",
    "ytd-banner-promo-renderer",
    "ytd-promoted-video-renderer",
    "ytd-compact-promoted-video-renderer",
    "ytd-promoted-sparkles-web-renderer",
    "ytd-promoted-sparkles-text-search-renderer",
    "ytd-statement-banner-renderer",
    "ytd-companion-slot-renderer",
    "ytd-action-companion-ad-renderer",
    "ytd-player-legacy-desktop-watch-ads-renderer",
    "ytd-reel-player-overlay-renderer #reel-ad-badge",
    "#player-ads",
    "#masthead-ad",
    ".ytp-ad-module",
    ".ytp-ad-overlay-container",
    ".ytp-ad-player-overlay",
    ".ytp-ad-text",
    ".video-ads"
  ].join(",");
  var AD_CARD_SELECTOR = "ytd-rich-item-renderer, ytd-video-renderer, ytd-grid-video-renderer, ytd-compact-video-renderer, ytd-playlist-video-renderer, ytd-reel-item-renderer";
  var PLAYER_AD_KEYS = {
    adplacements: 1, adslots: 1, adcueranges: 1, playerads: 1,
    adparams: 1, adbreakheartbeatparams: 1, adplaybackcontext: 1,
    adbreakservicerenderer: 1, adslotrenderer: 1, adlayoutrenderer: 1,
    instreamvideoadrenderer: 1, playerlegacydesktopwatchadsrenderer: 1,
    promotedsparkleswebrenderer: 1, companionslotrenderer: 1,
    actioncompanionadrenderer: 1
  };

  function yoodEnsureAdStyle() {
    try {
      if (document.getElementById("yood-ad-style")) return;
      var head = document.head || document.documentElement;
      if (!head) return;
      var style = document.createElement("style");
      style.id = "yood-ad-style";
      // One rule per selector: if YouTube's engine rejects one modern
      // selector it discards only that rule, not the whole sheet.
      var parts = AD_RENDERER_SELECTOR.split(",");
      var css = "";
      for (var i = 0; i < parts.length; i++) {
        var sel = parts[i].replace(/^\s+|\s+$/g, "");
        if (sel) css += sel + " { display: none !important; }\n";
      }
      style.textContent = css;
      (document.head || head).appendChild(style);
    } catch (e) { /* never break the page */ }
  }

  function yoodFastBlocked(raw) {
    var url;
    try { url = new URL(String(raw), location.href); }
    catch (e) { return false; }
    // Safety net: content CDNs must never match.
    var host = url.hostname.toLowerCase();
    if (host === "googlevideo.com" || host.slice(-16) === ".googlevideo.com") return false;
    if (host === "ytimg.com" || host.slice(-10) === ".ytimg.com") return false;
    if (host === "gstatic.com" || host.slice(-12) === ".gstatic.com") return false;
    var href = url.href;
    for (var i = 0; i < AD_TOKENS.length; i++) {
      var token = AD_TOKENS[i];
      if (host === token || host.slice(-token.length - 1) === "." + token || href.indexOf(token) !== -1) {
        return true;
      }
    }
    return false;
  }

  function yoodIsPlayerApiRequest(url) {
    try {
      var parsed = new URL(url, location.href);
      var host = parsed.hostname.toLowerCase();
      if (host !== "youtube.com" && host.slice(-12) !== ".youtube.com") return false;
      return /^\/youtubei\/v1\/(player|next)(\/|$)/.test(parsed.pathname);
    } catch (e) { return false; }
  }

  function yoodStripPlayerAdPayload(payload) {
    var queue = [payload];
    var seen = [];
    var changed = false;
    var inspected = 0;
    while (queue.length && inspected < 20000) {
      inspected++;
      var current = queue.pop();
      if (!current || typeof current !== "object") continue;
      var already = false;
      for (var s = 0; s < seen.length; s++) {
        if (seen[s] === current) { already = true; break; }
      }
      if (already) continue;
      if (seen.length < 20000) seen.push(current);
      for (var key in current) {
        if (!Object.prototype.hasOwnProperty.call(current, key)) continue;
        if (PLAYER_AD_KEYS[key.toLowerCase()]) {
          try { delete current[key]; changed = true; } catch (e) { /* frozen */ }
        } else {
          var nested = current[key];
          if (nested && typeof nested === "object") queue.push(nested);
        }
      }
    }
    return changed;
  }

  function yoodStripResponse(response, url) {
    try {
      var contentType = response.headers.get("content-type") || "";
      if (!/json/i.test(contentType)) return response;
      return response.clone().text().then(function (raw) {
        if (!raw || raw.length > 8 * 1024 * 1024) return response;
        var payload;
        try { payload = JSON.parse(raw); } catch (e) { return response; }
        if (!yoodStripPlayerAdPayload(payload)) return response;
        var headers = new Headers(response.headers);
        headers.delete("content-length");
        headers.delete("content-encoding");
        return new Response(JSON.stringify(payload), {
          status: response.status,
          statusText: response.statusText,
          headers: headers
        });
      }).catch(function () { return response; });
    } catch (e) { return response; }
  }

  function yoodInstallRequestBlock() {
    if (window.__YOOD_REQBLOCK__) return;
    window.__YOOD_REQBLOCK__ = true;
    try {
      var originalFetch = window.fetch.bind(window);
      window.fetch = function (input, init) {
        var raw = null;
        try {
          raw = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
        } catch (e) { raw = null; }
        var url = null;
        try { url = raw ? new URL(String(raw), location.href).href : null; } catch (e) { url = null; }
        if (url && yoodFastBlocked(url)) {
          return Promise.reject(new DOMException("Blocked by Yood", "AbortError"));
        }
        var promise = originalFetch(input, init);
        if (url && yoodIsPlayerApiRequest(url)) {
          return promise.then(function (response) { return yoodStripResponse(response, url); });
        }
        return promise;
      };
    } catch (e) { /* fetch patch is best-effort */ }
    try {
      var OriginalXHR = window.XMLHttpRequest;
      var PatchedXHR = function () {
        var xhr = new OriginalXHR();
        var origOpen = xhr.open;
        var yoodUrl = "";
        xhr.open = function (method, url) {
          try { yoodUrl = new URL(String(url), location.href).href; } catch (e) { yoodUrl = ""; }
          return origOpen.apply(xhr, arguments);
        };
        var origSend = xhr.send;
        xhr.send = function (body) {
          if (yoodUrl && yoodFastBlocked(yoodUrl)) {
            try { xhr.abort(); } catch (e) { /* noop */ }
            return;
          }
          return origSend.call(xhr, body);
        };
        return xhr;
      };
      // Preserve prototype chain so instanceof checks keep working.
      try { PatchedXHR.prototype = OriginalXHR.prototype; } catch (e) { /* noop */ }
      // Only replace when safe: some YouTube builds cache the constructor.
      try { window.XMLHttpRequest = PatchedXHR; } catch (e) { /* noop */ }
    } catch (e) { /* XHR patch is best-effort */ }
  }

  function yoodRemoveKnownAds(root) {
    var removed = false;
    try {
      var scope = root || document;
      var renderers = [];
      if (scope instanceof Element && scope.matches && scope.matches(AD_RENDERER_SELECTOR)) {
        renderers.push(scope);
      }
      var found = scope.querySelectorAll ? scope.querySelectorAll(AD_RENDERER_SELECTOR) : [];
      for (var i = 0; i < found.length; i++) renderers.push(found[i]);
      for (var j = 0; j < renderers.length; j++) {
        var renderer = renderers[j];
        if (!renderer.isConnected) continue;
        var card = renderer.closest ? renderer.closest(AD_CARD_SELECTOR) : null;
        try {
          if (card) card.remove();
          else renderer.remove();
          removed = true;
        } catch (e) { /* next */ }
      }
    } catch (e) { /* never break the page */ }
    return removed;
  }

  function yoodInstallCosmeticLayer() {
    if (window.__YOOD_COSMETIC__) return;
    window.__YOOD_COSMETIC__ = true;
    yoodEnsureAdStyle();
    if (document.head) {
      yoodRemoveKnownAds(document);
    } else {
      document.addEventListener("DOMContentLoaded", function () {
        yoodEnsureAdStyle();
        yoodRemoveKnownAds(document);
      }, { once: true });
    }
    // Re-ensure the style element if YouTube wipes <head> children.
    try {
      var headTarget = document.head || document.documentElement;
      if (headTarget) {
        new MutationObserver(function () { yoodEnsureAdStyle(); })
          .observe(headTarget, { childList: true });
      }
    } catch (e) { /* noop */ }
  }

  // Install ad layers immediately (document_start): every millisecond of
  // delay is a chance for an ad request to slip through first.
  yoodEnsureAdStyle();
  yoodInstallRequestBlock();
  yoodInstallCosmeticLayer();

  var SUBTITLE_LANGS = [
    ["he", "Hebrew (\u05E2\u05D1\u05E8\u05D9\u05EA)"],
    ["en", "English"],
    ["ar", "\u0627\u0644\u0639\u0631\u0628\u064A\u0629 (Arabic)"],
    ["es", "Espa\u00F1ol (Spanish)"],
    ["fr", "Fran\u00E7ais (French)"],
    ["de", "Deutsch (German)"],
    ["ru", "\u0420\u0443\u0441\u0441\u043A\u0438\u0439 (Russian)"],
    ["pt", "Portugu\u00EAs (Portuguese)"],
    ["it", "Italiano (Italian)"],
    ["nl", "Nederlands (Dutch)"],
    ["pl", "Polski (Polish)"],
    ["tr", "T\u00FCrk\u00E7e (Turkish)"],
    ["hi", "\u0939\u093F\u0928\u094D\u0926\u0940 (Hindi)"],
    ["ja", "\u65E5\u672C\u8A9E (Japanese)"],
    ["ko", "\uD55C\uAD6D\uC5B4 (Korean)"],
    ["zh-Hans", "\u4E2D\u6587\u7B80\u4F53 (Chinese Simplified)"]
  ];

  function storedSubLang() {
    try { return localStorage.getItem("yood-sub-lang") || "he"; }
    catch (e) { return "he"; }
  }
  function storeSubLang(value) {
    try { localStorage.setItem("yood-sub-lang", value); } catch (e) { /* private mode */ }
  }

  // The app window should read "Yood", not a bare "YouTube". Video pages
  // keep their video title (that is the useful behavior there).
  function retitle() {
    try {
      if (document.title === "YouTube" || document.title === "YouTube Music") {
        document.title = "Yood";
      }
    } catch (e) { /* never break the page */ }
  }

  function videoUrlFrom(card) {
    var link = card.querySelector('a[href*="/watch?v="], a[href*="/shorts/"], a[href*="youtu.be/"]');
    if (!link) return null;
    try {
      var url = new URL(link.href, location.href);
      if (url.hostname === "youtu.be") {
        var id = url.pathname.slice(1).split("/")[0];
        return id ? "https://www.youtube.com/watch?v=" + id : null;
      }
      return url.href;
    } catch (e) { return null; }
  }

  function handToYood(pageUrl, options) {
    var target = "yood://download?url=" + encodeURIComponent(pageUrl);
    if (options) {
      if (options.mode && options.mode !== "video") target += "&mode=" + encodeURIComponent(options.mode);
      if (options.quality && options.quality !== "best") target += "&quality=" + encodeURIComponent(options.quality);
      if (options.container && options.container !== "mp4") target += "&container=" + encodeURIComponent(options.container);
      if (options.format_selector) target += "&format_selector=" + encodeURIComponent(options.format_selector);
      if (options.subtitles) {
        target += "&subtitles=1";
        if (options.subtitle_lang) target += "&subtitle_lang=" + encodeURIComponent(options.subtitle_lang);
      }
      if (options.embed_thumbnail) target += "&embed_thumbnail=1";
    }
    window.location.href = target;
  }

  // ---- Alt+L floating bar (top center): links, IDs, channels ----
  function parseBarInput(raw) {
    var trimmed = (raw || "").trim();
    if (!trimmed) return null;
    if (/^[A-Za-z0-9_-]{11}$/.test(trimmed)) {
      return "https://www.youtube.com/watch?v=" + trimmed;
    }
    if (/^UC[A-Za-z0-9_-]{22}$/.test(trimmed)) {
      return "https://www.youtube.com/channel/" + trimmed;
    }
    var candidate = /^https?:\/\//i.test(trimmed) ? trimmed : "https://" + trimmed;
    var url;
    try { url = new URL(candidate); } catch (e) { return null; }
    if (url.protocol !== "https:") return null;
    var host = url.hostname.toLowerCase();
    if (host === "youtu.be") {
      var id = url.pathname.slice(1).split("/")[0];
      return id ? "https://www.youtube.com/watch?v=" + id + url.search : null;
    }
    var allowed = ["youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtube-nocookie.com"];
    for (var i = 0; i < allowed.length; i++) {
      if (host === allowed[i] || host.slice(-allowed[i].length - 1) === "." + allowed[i]) {
        return url.href;
      }
    }
    return null;
  }

  function showUrlBar() {
    closeDialog();
    var old = document.getElementById("yood-urlbar");
    if (old) old.remove();
    var wrap = document.createElement("div");
    wrap.id = "yood-urlbar";
    wrap.setAttribute("role", "dialog");
    wrap.setAttribute("aria-label", "Open YouTube link (Alt+L)");
    wrap.style.cssText = "position:fixed;top:12%;left:50%;transform:translateX(-50%);z-index:2147483647;width:min(560px,calc(100vw - 48px));background:#212121;color:#f1f1f1;border:1px solid #444;border-radius:14px;box-shadow:0 12px 48px #000b;padding:16px 18px;font:14px Roboto,Arial,sans-serif;";
    var head = document.createElement("div");
    head.style.cssText = "display:flex;align-items:center;gap:10px;margin-bottom:12px;";
    var logo = document.createElement("span");
    logo.textContent = "Y";
    logo.setAttribute("aria-hidden", "true");
    logo.style.cssText = "flex:none;width:26px;height:26px;border-radius:50%;background:#f00;color:#fff;font-size:15px;font-weight:700;display:flex;align-items:center;justify-content:center;";
    var heading = document.createElement("span");
    heading.textContent = "Open in Yood";
    heading.style.cssText = "font-weight:600;";
    var close = document.createElement("button");
    close.type = "button";
    close.textContent = "\u2715";
    close.setAttribute("aria-label", "Close (Esc)");
    close.style.cssText = "margin-left:auto;width:30px;height:30px;border:0;border-radius:50%;color:#eee;background:#ffffff14;cursor:pointer;font-size:16px;";
    head.appendChild(logo); head.appendChild(heading); head.appendChild(close);
    var row = document.createElement("div");
    row.style.cssText = "display:flex;gap:10px;";
    var input = document.createElement("input");
    input.type = "text";
    input.placeholder = "YouTube link, video ID, channel ID, @handle URL…  (Alt+L)";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.style.cssText = "flex:1;min-width:0;color:#f1f1f1;background:#303030;border:1px solid #666;border-radius:8px;padding:10px 12px;font-size:14px;outline:none;";
    var go = document.createElement("button");
    go.type = "button";
    go.textContent = "Go";
    go.style.cssText = "border:0;border-radius:18px;padding:9px 18px;color:#fff;background:#065fd4;cursor:pointer;white-space:nowrap;";
    row.appendChild(input); row.appendChild(go);
    var hint = document.createElement("div");
    hint.style.cssText = "color:#aaa;font-size:12px;margin-top:10px;";
    hint.textContent = "YouTube links only — videos, channels, playlists, Shorts.";
    var error = document.createElement("div");
    error.setAttribute("role", "alert");
    error.hidden = true;
    error.style.cssText = "color:#f28b82;font-size:12px;margin-top:8px;";
    wrap.appendChild(head); wrap.appendChild(row); wrap.appendChild(hint); wrap.appendChild(error);
    (document.body || document.documentElement).appendChild(wrap);
    var closeBar = function () { wrap.remove(); };
    close.addEventListener("click", closeBar);
    var submit = function () {
      var destination = parseBarInput(input.value);
      if (!destination) {
        error.textContent = "This isn't a YouTube link, video ID or channel.";
        error.hidden = false;
        return;
      }
      error.hidden = true;
      wrap.remove();
      window.location.href = destination;
    };
    go.addEventListener("click", submit);
    input.addEventListener("keydown", function (event) {
      event.stopPropagation();
      if (event.key === "Enter") { event.preventDefault(); submit(); }
      if (event.key === "Escape") { event.preventDefault(); closeBar(); }
    });
    setTimeout(function () { input.focus(); }, 0);
  }

  document.addEventListener("keydown", function (event) {
    var tag = (event.target && event.target.tagName) || "";
    if (/^(input|textarea|select)$/i.test(tag) || (event.target && event.target.isContentEditable)) return;
    if (event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && (event.code === "KeyL" || event.key.toLowerCase() === "l")) {
      event.preventDefault();
      event.stopPropagation();
      var bar = document.getElementById("yood-urlbar");
      if (bar) bar.remove();
      else showUrlBar();
    }
  }, true);

  // Minimal-browser policy: DevTools must not be openable. Brave app mode
  // has no menu, so the keyboard chords are the only path — swallow them
  // in the capture phase before they reach Brave. Covers F12,
  // Ctrl+Shift+C (element picker) and the rest of the inspector chords.
  // Mirrors installDevtoolsBlock in the webview integration layer.
  document.addEventListener("keydown", function (event) {
    var key = "";
    try { key = (event.key || "").toLowerCase(); } catch (e) { key = ""; }
    if (event.key === "F12" || event.code === "F12") {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.shiftKey &&
        (key === "i" || key === "j" || key === "c" || key === "k" || key === "e")) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.metaKey && event.altKey && (key === "i" || key === "j" || key === "c")) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.ctrlKey && !event.shiftKey && !event.metaKey && !event.altKey && key === "u") {
      event.preventDefault();
      event.stopPropagation();
    }
  }, true);

  // ---- Download buttons (inside YouTube's own rows) + dialog ----
  function chipButton(pageUrl) {
    var button = document.createElement("button");
    button.type = "button";
    button.setAttribute(MARK, pageUrl);
    button.setAttribute("aria-label", "Download with Yood");
    button.title = "Download with Yood";
    button.style.cssText = "display:inline-flex;align-items:center;gap:6px;border:0;border-radius:18px;padding:8px 14px 8px 10px;margin:4px;color:#f1f1f1;background:#272727;cursor:pointer;font:500 14px Roboto,Arial,sans-serif;white-space:nowrap;";
    var icon = document.createElement("span");
    icon.setAttribute("aria-hidden", "true");
    icon.style.cssText = "display:inline-flex;width:24px;height:24px;";
    icon.innerHTML = DOWNLOAD_SVG;
    var label = document.createElement("span");
    label.textContent = "Download";
    button.appendChild(icon);
    button.appendChild(label);
    button.addEventListener("click", function (event) {
      event.preventDefault();
      event.stopPropagation();
      showDownloadDialog(pageUrl);
    });
    return button;
  }

  // YouTube reuses #top-level-buttons-computed in several places, including
  // HIDDEN engagement-panel headers (duplicate IDs!). Always pick a host
  // that is actually visible, otherwise the button renders at 0x0.
  // NOTE: keep this selector list a superset of the reference project's
  // list, in the same order — the download button must look and sit
  // exactly like there (same dark pill inside YouTube's own rows). The
  // extra variants only extend coverage to newer YouTube DOM where nodes
  // were renamed; they add no new styling.
  function visibleActionsHost() {
    var candidates = document.querySelectorAll("ytd-watch-metadata #top-level-buttons-computed, #top-level-buttons-computed, ytd-watch-metadata #actions-inner, ytd-watch-metadata #actions, ytd-reel-player-header-renderer #actions, ytd-reel-player-overlay-renderer #actions, ytd-shorts #actions, ytd-reel-video-renderer #actions, #top-level-buttons");
    var fallback = null;
    for (var i = 0; i < candidates.length; i++) {
      var h = candidates[i];
      if (!(h instanceof Element)) continue;
      if (h.closest("ytd-engagement-panel-section-list-renderer, ytd-engagement-panel-title-header-renderer, [hidden]")) continue;
      var r;
      try { r = h.getBoundingClientRect(); } catch (e) { continue; }
      if (r.width > 40 && r.height > 0) return h;
      if (!fallback) fallback = h;
    }
    return fallback;
  }

  function injectVideoButton() {
    var url = location.href;
    if (!/youtube\.com\/(watch|shorts\/)/i.test(url)) return;
    var isShorts = /youtube\.com\/shorts\//i.test(url);
    var host = visibleActionsHost();
    // Drop stale buttons: wrong video, detached, hidden — or parked in the
    // floating fallback while a real host is now visible. YouTube renders
    // its rows lazily, so the fallback often wins the first race; without
    // the re-homing check the button would sit bottom-left forever instead
    // of moving into the Like/Share/Save row once it exists.
    var mains = document.querySelectorAll("[" + MARK + "][data-yood-main]");
    var good = null;
    for (var i = 0; i < mains.length; i++) {
      var node = mains[i];
      var rect = null;
      try { rect = node.getBoundingClientRect(); } catch (e) { rect = null; }
      if (!(node.isConnected && node.getAttribute(MARK) === url && rect && rect.width > 40 && rect.height > 0)) {
        dropMainButton(node);
        continue;
      }
      if (host && !host.contains(node)) {
        // On Shorts the chip lives inside a rail container rather than the
        // returned host — that IS home, keep it (avoids remove/re-add
        // churn every scan when several rail candidates are visible).
        var inRail = false;
        try {
          inRail = isShorts && !!node.closest("ytd-shorts, ytd-reel-video-renderer, ytd-reel-player-overlay-renderer, ytd-reel-player-header-renderer");
        } catch (e) { inRail = false; }
        if (!inRail) {
          dropMainButton(node);
          continue;
        }
      }
      if (!good) good = node;
      else dropMainButton(node);
    }
    if (good) return;
    if (isShorts) {
      injectShortsChip(url);
      return;
    }
    var button = chipButton(url);
    button.setAttribute("data-yood-main", "true");
    if (host) {
      host.appendChild(button);
    } else if (document.body) {
      var wrap = document.createElement("div");
      wrap.setAttribute("data-yood-main", "true");
      wrap.style.cssText = "position:fixed;left:16px;bottom:16px;z-index:2147483640;";
      wrap.appendChild(button);
      document.body.appendChild(wrap);
    }
  }

  // Removes a main button plus its wrapper when the wrapper is left empty
  // (floating fallback / Shorts slot). Prevents empty shells piling up
  // across SPA navigations.
  function dropMainButton(node) {
    var shell = null;
    try {
      var parent = node.parentNode;
      if (parent && parent instanceof Element && parent !== document.body &&
          parent.hasAttribute("data-yood-main") && !parent.hasAttribute(MARK)) {
        shell = parent;
      }
    } catch (e) { shell = null; }
    try { node.remove(); } catch (e) { /* gone */ }
    try {
      if (shell && shell.childNodes.length === 0) shell.remove();
    } catch (e) { /* keep */ }
  }

  // Shorts rail candidates, visible one preferred inside the active reel.
  function shortsRail() {
    var rails = document.querySelectorAll("ytd-reel-player-overlay-renderer #actions, ytd-shorts #actions, ytd-reel-video-renderer #actions, ytd-reel-player-header-renderer #actions");
    var fallback = null;
    for (var i = 0; i < rails.length; i++) {
      var rail = rails[i];
      if (!(rail instanceof Element) || !rail.isConnected) continue;
      if (rail.closest("[hidden]")) continue;
      var rect = null;
      try { rect = rail.getBoundingClientRect(); } catch (e) { continue; }
      if (!rect || rect.width <= 0 || rect.height <= 0) continue;
      try {
        if (rail.closest("ytd-reel-video-renderer[is-active]")) return rail;
      } catch (e) { /* noop */ }
      if (!fallback) fallback = rail;
    }
    return fallback;
  }

  // The rail action labelled "Share" — the chip goes directly beneath it.
  function shortsShareAnchor(rail) {
    var kids = rail.children;
    for (var i = 0; i < kids.length; i++) {
      var kid = kids[i];
      if (!(kid instanceof Element)) continue;
      if (kid.hasAttribute(MARK)) continue;
      var aria = "";
      try { aria = (kid.getAttribute("aria-label") || "").trim().toLowerCase(); } catch (e) { aria = ""; }
      if (aria === "share") return kid;
      var text = "";
      try { text = (kid.textContent || "").trim().toLowerCase(); } catch (e) { text = ""; }
      if (text === "share") return kid;
      try {
        if (kid.querySelector('[aria-label="Share"], [aria-label="share"]')) return kid;
      } catch (e) { /* next */ }
    }
    return null;
  }

  // Shorts: the SAME dark pill as everywhere else, inserted into the rail
  // directly beneath the Share button. Only when no rail exists at all
  // does it fall back to the floating bottom-left chip.
  function injectShortsChip(url) {
    var button = chipButton(url);
    button.setAttribute("data-yood-main", "true");
    var rail = shortsRail();
    if (rail) {
      var slot = document.createElement("div");
      slot.setAttribute("data-yood-main", "true");
      slot.style.cssText = "display:flex;justify-content:center;margin:6px 0;max-width:100%;";
      slot.appendChild(button);
      var anchor = shortsShareAnchor(rail);
      try {
        rail.insertBefore(slot, anchor ? anchor.nextSibling : null);
      } catch (e) { /* detached rail; fall through to fallback */ }
      if (slot.isConnected) return;
      try { slot.remove(); } catch (e2) { /* noop */ }
      button = chipButton(url);
      button.setAttribute("data-yood-main", "true");
    }
    if (document.body) {
      var wrap = document.createElement("div");
      wrap.setAttribute("data-yood-main", "true");
      wrap.style.cssText = "position:fixed;left:16px;bottom:16px;z-index:2147483640;";
      wrap.appendChild(button);
      document.body.appendChild(wrap);
    }
  }

  function injectCardButton(card) {
    if (card.querySelector("[" + MARK + "]")) return;
    var url = videoUrlFrom(card);
    if (!url) return;
    var host = card.querySelector("#menu, #buttons, #menu-container") || card;
    host.appendChild(chipButton(url));
  }

  function closeDialog() {
    var overlay = document.querySelector(".yood-overlay");
    if (overlay) overlay.remove();
  }

  // ---- Download dialog, styled like YouTube's own menus ----
  var YT_BG = "#212121";
  var YT_HOVER = "#3d3d3d";
  var YT_TEXT = "#f1f1f1";
  var YT_MUTED = "#aaa";
  var YT_BLUE = "#3ea6ff";
  var YT_PRIMARY = "#065fd4";

  function ytRow(label, sub, selected, disabled) {
    var row = document.createElement("div");
    row.setAttribute("role", "option");
    row.setAttribute("aria-selected", selected ? "true" : "false");
    row.style.cssText = "display:flex;align-items:center;gap:14px;padding:11px 16px;cursor:" +
      (disabled ? "default" : "pointer") + ";opacity:" + (disabled ? ".45" : "1") + ";";
    var radio = document.createElement("span");
    radio.setAttribute("aria-hidden", "true");
    radio.style.cssText = "flex:none;width:18px;height:18px;border-radius:50%;border:2px solid " +
      (selected ? YT_BLUE : "#909090") + ";display:inline-flex;align-items:center;justify-content:center;";
    if (selected) {
      var dot = document.createElement("span");
      dot.style.cssText = "width:10px;height:10px;border-radius:50%;background:" + YT_BLUE + ";";
      radio.appendChild(dot);
    }
    var texts = document.createElement("span");
    texts.style.cssText = "flex:1;min-width:0;";
    var main = document.createElement("div");
    main.style.cssText = "font-size:14px;color:" + YT_TEXT + ";";
    main.textContent = label;
    texts.appendChild(main);
    if (sub) {
      var secondary = document.createElement("div");
      secondary.style.cssText = "font-size:12px;color:" + YT_MUTED + ";margin-top:2px;";
      secondary.textContent = sub;
      texts.appendChild(secondary);
    }
    row.appendChild(radio);
    row.appendChild(texts);
    if (!disabled) {
      row.addEventListener("mouseenter", function () { row.style.background = YT_HOVER; });
      row.addEventListener("mouseleave", function () { row.style.background = "transparent"; });
    }
    return row;
  }

  function ytSectionHeader(text) {
    var el = document.createElement("div");
    el.style.cssText = "font-size:12px;font-weight:500;letter-spacing:.06em;text-transform:uppercase;color:" + YT_MUTED + ";padding:14px 16px 4px;";
    el.textContent = text;
    return el;
  }

  function showDownloadDialog(pageUrl) {
    closeDialog();
    var bar = document.getElementById("yood-urlbar");
    if (bar) bar.remove();
    var state = {
      mode: "video",
      quality: "best",
      container: "mp4",
      subtitles: false,
      subtitle_lang: storedSubLang(),
      format_selector: "",
      embed_thumbnail: false
    };
    var QUALITY_ROWS = [
      ["best", "Best available", "Highest quality video + audio"],
      ["1080", "1080p", "Full HD or best below it"],
      ["720", "720p", "HD or best below it"],
      ["480", "480p", "Smaller file"],
      ["worst", "Smallest file", "Lowest available quality"]
    ];
    var MODE_ROWS = [
      ["video", "Video", "MP4 / MKV with audio"],
      ["audio", "Audio", "MP3 only"],
      ["songs", "Songs", "MP3 + music metadata"]
    ];

    var overlay = document.createElement("div");
    overlay.className = "yood-overlay";
    overlay.style.cssText = "position:fixed;inset:0;z-index:2147483646;background:rgba(0,0,0,.6);display:flex;align-items:center;justify-content:center;font:14px Roboto,Arial,sans-serif;";
    var card = document.createElement("section");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.style.cssText = "width:min(480px,calc(100vw - 32px));max-height:min(720px,calc(100vh - 40px));overflow:auto;background:" + YT_BG + ";color:" + YT_TEXT + ";border-radius:12px;box-shadow:0 8px 40px #000c;padding:8px 0 0;";
    var head = document.createElement("div");
    head.style.cssText = "display:flex;align-items:center;padding:12px 8px 8px 20px;";
    var title = document.createElement("div");
    title.style.cssText = "flex:1;font-size:16px;font-weight:500;";
    title.textContent = "Download";
    var x = document.createElement("button");
    x.type = "button";
    x.textContent = "\u2715";
    x.setAttribute("aria-label", "Close");
    x.style.cssText = "width:40px;height:40px;border:0;border-radius:50%;background:transparent;color:" + YT_TEXT + ";font-size:18px;cursor:pointer;";
    head.appendChild(title);
    head.appendChild(x);
    card.appendChild(head);
    var videoName = document.createElement("div");
    videoName.style.cssText = "font-size:12px;color:" + YT_MUTED + ";padding:0 20px 4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;";
    videoName.textContent = document.title.replace(/ - YouTube$/, "");
    card.appendChild(videoName);

    function refresh() {
      var audioOnly = state.mode !== "video";
      card.querySelectorAll("[data-yood-q]").forEach(function (row) {
        var on = row.getAttribute("data-yood-q") === state.quality && !audioOnly;
        row.setAttribute("aria-selected", on ? "true" : "false");
        row.style.opacity = audioOnly ? ".45" : "1";
        var radio = row.firstChild;
        radio.style.borderColor = on ? YT_BLUE : "#909090";
        radio.innerHTML = on ? '<span style="width:10px;height:10px;border-radius:50%;background:' + YT_BLUE + ';"></span>' : "";
      });
      card.querySelectorAll("[data-yood-m]").forEach(function (row) {
        var on = row.getAttribute("data-yood-m") === state.mode;
        row.setAttribute("aria-selected", on ? "true" : "false");
        var radio = row.firstChild;
        radio.style.borderColor = on ? YT_BLUE : "#909090";
        radio.innerHTML = on ? '<span style="width:10px;height:10px;border-radius:50%;background:' + YT_BLUE + ';"></span>' : "";
      });
      var contRow = card.querySelector("[data-yood-cont]");
      if (contRow) {
        contRow.style.opacity = audioOnly ? ".45" : "1";
        var sel = contRow.querySelector("select");
        if (sel) { sel.disabled = audioOnly; sel.value = state.container; }
      }
      var subRow = card.querySelector("[data-yood-sub]");
      if (subRow) {
        var on = state.subtitles;
        subRow.setAttribute("aria-selected", on ? "true" : "false");
        var radio = subRow.firstChild;
        radio.style.borderColor = on ? YT_BLUE : "#909090";
        radio.innerHTML = on ? '<span style="width:10px;height:10px;border-radius:50%;background:' + YT_BLUE + ';"></span>' : "";
      }
      var langRow = card.querySelector("[data-yood-lang]");
      if (langRow) {
        langRow.style.opacity = state.subtitles ? "1" : ".45";
        var langSel = langRow.querySelector("select");
        if (langSel) { langSel.disabled = !state.subtitles; langSel.value = state.subtitle_lang; }
      }
    }

    card.appendChild(ytSectionHeader("Format"));
    MODE_ROWS.forEach(function (item) {
      var row = ytRow(item[1], item[2], state.mode === item[0], false);
      row.setAttribute("data-yood-m", item[0]);
      row.addEventListener("click", function () { state.mode = item[0]; refresh(); });
      card.appendChild(row);
    });

    card.appendChild(ytSectionHeader("Quality"));
    QUALITY_ROWS.forEach(function (item) {
      var row = ytRow(item[1], item[2], state.quality === item[0], false);
      row.setAttribute("data-yood-q", item[0]);
      row.addEventListener("click", function () {
        if (state.mode !== "video") return;
        state.quality = item[0];
        refresh();
      });
      card.appendChild(row);
    });

    var contRow = document.createElement("div");
    contRow.setAttribute("data-yood-cont", "true");
    contRow.style.cssText = "display:flex;align-items:center;gap:14px;padding:11px 16px;";
    var contLabel = document.createElement("span");
    contLabel.style.cssText = "flex:1;font-size:14px;";
    contLabel.textContent = "Container";
    var contSel = document.createElement("select");
    contSel.style.cssText = "color:" + YT_TEXT + ";background:#303030;border:1px solid #666;border-radius:8px;padding:8px;";
    [["mp4", "MP4"], ["mkv", "MKV"]].forEach(function (pair) {
      var opt = document.createElement("option");
      opt.value = pair[0];
      opt.textContent = pair[1];
      contSel.appendChild(opt);
    });
    contSel.value = state.container;
    contSel.addEventListener("change", function () { state.container = contSel.value; });
    contRow.appendChild(contLabel);
    contRow.appendChild(contSel);
    card.appendChild(ytSectionHeader("File"));
    card.appendChild(contRow);

    card.appendChild(ytSectionHeader("Subtitles"));
    var subRow = ytRow("Download subtitles", "Manual captions when available", false, false);
    subRow.setAttribute("data-yood-sub", "true");
    subRow.addEventListener("click", function () { state.subtitles = !state.subtitles; refresh(); });
    card.appendChild(subRow);
    var langRow = document.createElement("div");
    langRow.setAttribute("data-yood-lang", "true");
    langRow.style.cssText = "display:flex;align-items:center;gap:14px;padding:11px 16px;";
    var langLabel = document.createElement("span");
    langLabel.style.cssText = "flex:1;";
    var langTitle = document.createElement("div");
    langTitle.style.cssText = "font-size:14px;";
    langTitle.textContent = "Language";
    var langSub = document.createElement("div");
    langSub.style.cssText = "font-size:12px;color:" + YT_MUTED + ";margin-top:2px;";
    langSub.textContent = "Auto-translated when manual captions are missing";
    langLabel.appendChild(langTitle);
    langLabel.appendChild(langSub);
    var langSel = document.createElement("select");
    langSel.style.cssText = "color:" + YT_TEXT + ";background:#303030;border:1px solid #666;border-radius:8px;padding:8px;max-width:55%;";
    SUBTITLE_LANGS.forEach(function (pair) {
      var opt = document.createElement("option");
      opt.value = pair[0];
      opt.textContent = pair[1];
      langSel.appendChild(opt);
    });
    langSel.value = state.subtitle_lang;
    langSel.addEventListener("change", function () {
      state.subtitle_lang = langSel.value;
      storeSubLang(langSel.value);
    });
    langRow.appendChild(langLabel);
    langRow.appendChild(langSel);
    card.appendChild(langRow);

    var advanced = document.createElement("details");
    advanced.style.cssText = "margin:4px 16px 0;";
    var summary = document.createElement("summary");
    summary.textContent = "Advanced";
    summary.style.cssText = "cursor:pointer;padding:8px 0;color:" + YT_MUTED + ";";
    advanced.appendChild(summary);
    var formatInput = document.createElement("input");
    formatInput.type = "text";
    formatInput.placeholder = "yt-dlp format selector (optional)";
    formatInput.style.cssText = "width:100%;box-sizing:border-box;color:" + YT_TEXT + ";background:#303030;border:1px solid #666;border-radius:8px;padding:10px;margin:4px 0 10px;";
    formatInput.addEventListener("input", function () { state.format_selector = formatInput.value.trim(); });
    var thumbRow = ytRow("Embed thumbnail and metadata", "", false, false);
    thumbRow.addEventListener("click", function () {
      state.embed_thumbnail = !state.embed_thumbnail;
      var on = state.embed_thumbnail;
      thumbRow.setAttribute("aria-selected", on ? "true" : "false");
      var radio = thumbRow.firstChild;
      radio.style.borderColor = on ? YT_BLUE : "#909090";
      radio.innerHTML = on ? '<span style="width:10px;height:10px;border-radius:50%;background:' + YT_BLUE + ';"></span>' : "";
    });
    advanced.appendChild(formatInput);
    advanced.appendChild(thumbRow);
    card.appendChild(advanced);

    var foot = document.createElement("div");
    foot.style.cssText = "display:flex;justify-content:flex-end;gap:4px;padding:8px 12px 12px;";
    var cancel = document.createElement("button");
    cancel.type = "button";
    cancel.textContent = "Cancel";
    cancel.style.cssText = "border:0;border-radius:18px;padding:10px 16px;color:" + YT_BLUE + ";background:transparent;font-size:14px;font-weight:500;cursor:pointer;";
    cancel.addEventListener("click", closeDialog);
    var start = document.createElement("button");
    start.type = "button";
    start.textContent = "Download";
    start.style.cssText = "border:0;border-radius:18px;padding:10px 16px;color:#fff;background:" + YT_PRIMARY + ";font-size:14px;font-weight:500;cursor:pointer;";
    start.addEventListener("click", function () {
      start.disabled = true;
      try {
        handToYood(pageUrl, {
          mode: state.mode,
          quality: state.quality,
          container: state.container,
          format_selector: state.format_selector || undefined,
          subtitles: state.subtitles,
          subtitle_lang: state.subtitles ? state.subtitle_lang : undefined,
          embed_thumbnail: state.embed_thumbnail || state.mode === "songs"
        });
      } finally {
        closeDialog();
      }
    });
    foot.appendChild(cancel);
    foot.appendChild(start);
    card.appendChild(foot);

    overlay.appendChild(card);
    overlay.addEventListener("click", function (event) {
      if (event.target === overlay) closeDialog();
    });
    var onKey = function (event) {
      if (event.key === "Escape") {
        closeDialog();
        document.removeEventListener("keydown", onKey);
      }
    };
    document.addEventListener("keydown", onKey);
    x.addEventListener("click", function () {
      closeDialog();
      document.removeEventListener("keydown", onKey);
    });
    document.body.appendChild(overlay);
    refresh();
  }

  // ---- In-player ad skip (backup for video ads Shields may miss) ----
  // Video ads stream from the same CDN as real content, so no URL blocker
  // can stop them. When one still plays, mute it, speed it up, jump past
  // unskippable bumpers and press Skip the moment YouTube enables it.
  // Runs ONLY while the player itself reports an ad with ad UI present —
  // normal playback is never touched, and nothing polls while idle.
  var yoodSkipTimer;
  var yoodMutedForAd = false;
  var yoodRateForAd = false;

  function yoodPlayerRoots() {
    var roots = [];
    try {
      var main = document.getElementById("movie_player");
      if (main && main.isConnected && roots.indexOf(main) === -1) roots.push(main);
    } catch (e) { /* noop */ }
    try {
      // Shorts: each reel is its own player; only the active one can show an ad.
      var reels = document.querySelectorAll("ytd-reel-video-renderer[is-active], ytd-shorts ytd-reel-video-renderer");
      for (var i = 0; i < reels.length && roots.length < 4; i++) {
        if (reels[i] instanceof Element && reels[i].isConnected && roots.indexOf(reels[i]) === -1) {
          roots.push(reels[i]);
        }
      }
    } catch (e) { /* noop */ }
    return roots;
  }
  function yoodIsAd(node) {
    try {
      return node.classList.contains("ad-showing") || node.classList.contains("ad-interrupting");
    } catch (e) { return false; }
  }
  function yoodStopTimer() {
    if (yoodSkipTimer !== undefined) {
      clearInterval(yoodSkipTimer);
      yoodSkipTimer = undefined;
    }
  }
  function yoodRestore(node) {
    var video = null;
    try { video = node.querySelector("video.html5-main-video, video"); } catch (e) { video = null; }
    if (!video) return;
    try {
      if (yoodMutedForAd) { video.muted = false; yoodMutedForAd = false; }
      if (yoodRateForAd) { video.playbackRate = 1; yoodRateForAd = false; }
    } catch (e) { /* ad is gone anyway */ }
  }
  function yoodRestoreAll() {
    var roots = yoodPlayerRoots();
    for (var i = 0; i < roots.length; i++) yoodRestore(roots[i]);
  }
  function yoodAdFrame() {
    var roots = yoodPlayerRoots();
    if (!roots.length) { yoodStopTimer(); return; }
    var anyAd = false;
    for (var i = 0; i < roots.length; i++) {
      var node = roots[i];
      if (!yoodIsAd(node)) {
        yoodRestore(node);
        continue;
      }
      anyAd = true;
      var skip = null;
      var overlay = null;
      try {
        skip = node.querySelector(".ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-skip-ad-button-modern, .ytp-ad-skip-button-modern");
        overlay = node.querySelector(".ytp-ad-player-overlay, .ytp-ad-text, .ytp-ad-message-container, .ytp-ad-skip-button-container, .ytp-ad-player-overlay-layout");
      } catch (e) { skip = null; overlay = null; }
      if (!skip && !overlay) continue;
      var video = null;
      try { video = node.querySelector("video.html5-main-video, video"); } catch (e) { video = null; }
      if (video) {
        try {
          if (!video.muted) { video.muted = true; yoodMutedForAd = true; }
          if (video.playbackRate !== 16) { video.playbackRate = 16; yoodRateForAd = true; }
          if (isFinite(video.duration) && video.duration > 0 && video.duration <= 120 &&
              video.currentTime < video.duration - 0.3) {
            video.currentTime = video.duration - 0.1;
          }
        } catch (e) { /* next tick retries */ }
      }
      if (skip && skip.offsetParent !== null) {
        try { skip.click(); } catch (e) { /* next tick */ }
      }
    }
    if (!anyAd) {
      yoodStopTimer();
      yoodRestoreAll();
    }
  }
  function yoodAdClassChanged() {
    var roots = yoodPlayerRoots();
    var found = false;
    for (var i = 0; i < roots.length; i++) {
      if (yoodIsAd(roots[i])) { found = true; break; }
    }
    if (found) {
      yoodAdFrame();
      if (yoodSkipTimer === undefined) yoodSkipTimer = setInterval(yoodAdFrame, 400);
    } else {
      yoodStopTimer();
      yoodRestoreAll();
    }
  }
  function yoodWatchNode(node) {
    if (!node || node.hasAttribute("data-yood-adwatch")) return false;
    try {
      node.setAttribute("data-yood-adwatch", "");
      new MutationObserver(yoodAdClassChanged).observe(node, { attributes: true, attributeFilter: ["class"] });
      return true;
    } catch (e) { return false; }
  }
  function installPlayerAdSkip() {
    var roots = yoodPlayerRoots();
    if (!roots.length) return;
    for (var i = 0; i < roots.length; i++) yoodWatchNode(roots[i]);
    yoodAdClassChanged();
  }

  // ---- Scan loop (event-driven, debounced) ----
  var scheduled = false;
  function schedule() {
    if (scheduled) return;
    scheduled = true;
    setTimeout(function () {
      scheduled = false;
      try {
        scan();
        retitle();
      } catch (e) { /* never break the page */ }
    }, 250);
  }

  function scan() {
    if (!/youtube\.com|youtu\.be/i.test(location.hostname)) return;
    yoodEnsureAdStyle();
    if (document.querySelector("ytd-app, ytd-page-manager")) {
      var cards = document.querySelectorAll(CARD_SELECTOR);
      for (var i = 0; i < cards.length; i++) {
        try { injectCardButton(cards[i]); } catch (e) { /* next card */ }
      }
      try { injectVideoButton(); } catch (e) { /* next scan */ }
      try { yoodRemoveKnownAds(document); } catch (e) { /* next scan */ }
      try { installPlayerAdSkip(); } catch (e) { /* next scan */ }
    }
  }

  ["yt-navigate-finish", "yt-page-data-updated", "popstate"].forEach(function (event) {
    window.addEventListener(event, schedule);
  });
  window.addEventListener("yt-navigate-finish", function () {
    setTimeout(installPlayerAdSkip, 600);
  });
  window.addEventListener("yt-page-data-updated", function () {
    setTimeout(installPlayerAdSkip, 600);
  });
  retitle();
  installPlayerAdSkip();
  if (document.head) {
    new MutationObserver(retitle).observe(
      document.head.querySelector("title") || document.head,
      { childList: true, subtree: true, characterData: true }
    );
  }
  if (document.documentElement) {
    new MutationObserver(function (records) {
      for (var i = 0; i < records.length; i++) {
        var added = records[i].addedNodes;
        for (var j = 0; j < added.length; j++) {
          var n = added[j];
          if (n instanceof Element) {
            // New ad renderer? Remove synchronously without a full scan.
            try {
              if (n.matches && n.matches(AD_RENDERER_SELECTOR)) yoodRemoveKnownAds(n.parentNode || document);
              else if (n.querySelector && n.querySelector(AD_RENDERER_SELECTOR)) yoodRemoveKnownAds(n);
            } catch (e) { /* fall through to scan */ }
            schedule();
            return;
          }
        }
      }
    }).observe(document.documentElement, { childList: true, subtree: true });
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () { scan(); retitle(); yoodEnsureAdStyle(); }, { once: true });
  } else {
    scan();
  }
})();
