"use strict";
/* Yood's intentionally small YouTube integration layer. It is installed once
 * by Tauri as an initialization script and survives YouTube's SPA navigation. */
const invoke = async (command, args) => {
    const fn = window.__TAURI__?.core?.invoke;
    if (!fn)
        throw new Error("Yood integration is not available");
    return fn(command, args);
};
// Returns a no-op unlisten function when the Tauri event bridge is
// unavailable so callers never need to null-check the result.
async function listenEvent(event, callback) {
    const listen = window.__TAURI__?.event?.listen;
    if (!listen)
        return () => undefined;
    try {
        return await listen(event, callback);
    }
    catch {
        return () => undefined;
    }
}
let selectedAppearance = "system";
let systemAppearance;
let detectedSystemAppearance;
let appearanceRevision = 0;
let appearanceTimers = [];
let downloadShortcut = "d";
// YouTube owns the rest of the document theme.  Restrict Yood's override to
// the documented root attribute and colour-scheme hint; changing several
// legacy attributes makes YouTube's own theme controller repeatedly rebuild
// its UI on some WebKit versions.
const THEME_SETTLE_DELAYS = [160, 1100];
function effectiveAppearance(appearance) {
    return appearance === "system"
        ? (detectedSystemAppearance ?? (systemAppearance?.matches ? "dark" : "light"))
        : appearance;
}
function clearAppearanceTimers() {
    appearanceTimers.forEach((timer) => window.clearTimeout(timer));
    appearanceTimers = [];
}
function applyEffectiveAppearance() {
    const effective = effectiveAppearance(selectedAppearance);
    const dark = effective === "dark";
    const root = document.documentElement;
    if (!root)
        return;
    root.style.setProperty("color-scheme", effective, "important");
    // This is the attribute YouTube uses for its colour variables.  Do not
    // modify body classes or YouTube's legacy theme attributes: doing so causes
    // its asynchronous theme code to fight the integration layer.
    root.toggleAttribute("dark", dark);
    let style = document.getElementById("yood-theme-style");
    if (!style && document.head) {
        style = document.createElement("style");
        style.id = "yood-theme-style";
        document.head.appendChild(style);
    }
    if (style) {
        const background = dark ? "#0f0f0f" : "#fff";
        style.textContent = `:root { color-scheme: ${effective} !important; background-color: ${background} !important; }`;
    }
}
function stabilizeAppearance() {
    clearAppearanceTimers();
    applyEffectiveAppearance();
    // YouTube applies its saved preference shortly after a navigation.  A
    // bounded pair of follow-up writes wins that initial race without a
    // perpetual MutationObserver loop or background work while the page idles.
    appearanceTimers = THEME_SETTLE_DELAYS.map((delay) => window.setTimeout(() => {
        applyEffectiveAppearance();
    }, delay));
}
function applyAppearance(appearance, userInitiated = false) {
    if (userInitiated)
        appearanceRevision += 1;
    selectedAppearance = appearance;
    const root = document.documentElement;
    if (!root) {
        window.addEventListener("DOMContentLoaded", () => applyAppearance(appearance), { once: true });
        return;
    }
    stabilizeAppearance();
}
function installAppearance() {
    systemAppearance ?? (systemAppearance = window.matchMedia("(prefers-color-scheme: dark)"));
    detectedSystemAppearance = systemAppearance.matches ? "dark" : "light";
    applyAppearance("system");
    systemAppearance.addEventListener("change", () => {
        detectedSystemAppearance = systemAppearance?.matches ? "dark" : "light";
        if (selectedAppearance === "system")
            applyAppearance("system");
    });
    const reapply = () => applyAppearance(selectedAppearance);
    window.addEventListener("DOMContentLoaded", reapply, { once: true });
    window.addEventListener("yt-navigate-finish", reapply);
    window.addEventListener("yt-page-data-updated", reapply);
    void invoke("get_system_appearance").then((appearance) => {
        if (appearance === "dark" || appearance === "light") {
            detectedSystemAppearance = appearance;
            if (selectedAppearance === "system")
                applyAppearance("system");
        }
    }).catch(() => undefined);
    const settingsRequestRevision = appearanceRevision;
    void invoke("get_settings").then((settings) => {
        const appearance = settings.appearance;
        if (appearanceRevision === settingsRequestRevision && (appearance === "light" || appearance === "dark" || appearance === "system")) {
            applyAppearance(appearance);
        }
    }).catch(() => undefined);
}
const css = `
#yood-root { font-family: Roboto, Arial, sans-serif; color-scheme: light dark; }
#yood-root button { font: inherit; }
.yood-download-button { border: 0; border-radius: 18px; padding: 8px 14px; margin: 4px; color: #f1f1f1; background: #272727; cursor: pointer; font-weight: 500; }
.yood-download-button:hover { background: #3f3f3f; }
.yood-download-button[aria-busy=true] { opacity: .65; }
.yood-native-player-button { border: 1px solid #3ea6ff; border-radius: 18px; padding: 8px 14px; margin: 4px; color: #3ea6ff; background: transparent; cursor: pointer; font-weight: 500; }
.yood-native-player-button:hover { background: #263850; }
.yood-video-actions-fallback { position: fixed; left: 16px; bottom: 68px; z-index: 2147483640; display: flex; gap: 6px; align-items: center; padding: 5px; border: 1px solid #444; border-radius: 22px; background: #181818e8; box-shadow: 0 4px 16px #0008; }
#yood-navigation { position: fixed; left: 16px; bottom: 16px; z-index: 2147483640; display: flex; gap: 4px; padding: 5px; border: 1px solid #444; border-radius: 22px; background: #181818e8; box-shadow: 0 4px 16px #0008; direction: ltr; }
#yood-navigation button { width: 34px; height: 34px; border: 0; border-radius: 17px; color: #f1f1f1; background: transparent; font-size: 20px; cursor: pointer; }
#yood-navigation button.yood-url-button { width: auto; font-size: 11px; font-weight: 600; letter-spacing: .04em; padding: 0 7px; border-radius: 12px; color: #8ab4f8; }
#yood-navigation button:hover { background: #3f3f3f; }
/* Tiling compositors (Hyprland, niri) draw no window decorations, so Yood's
   own floating controls stay tucked at the bottom edge and never mimic a
   title bar.  Plasma/GNOME keep the standard placement next to their native
   decorations. */
:root[data-yood-desktop="hyprland"] #yood-navigation,
:root[data-yood-desktop="niri"] #yood-navigation,
:root[data-yood-desktop="hyprland"] #yood-launcher,
:root[data-yood-desktop="niri"] #yood-launcher { left: 12px; bottom: 12px; opacity: .92; }
.yood-overlay { position: fixed; inset: 0; z-index: 2147483646; background: rgba(0,0,0,.58); display: flex; align-items: center; justify-content: center; }
.yood-card { width: min(520px, calc(100vw - 28px)); max-height: min(760px, calc(100vh - 32px)); overflow: auto; background: #212121; color: #f1f1f1; border: 1px solid #444; border-radius: 14px; box-shadow: 0 12px 48px #000b; padding: 20px; }
.yood-card h2 { font-size: 20px; margin: 0 0 16px; }
.yood-card h3 { font-size: 14px; margin: 18px 0 8px; }
.yood-row { display: flex; gap: 10px; align-items: center; margin: 9px 0; }
.yood-row label { flex: 1; }
.yood-card select, .yood-card input { color: inherit; background: #303030; border: 1px solid #666; border-radius: 6px; padding: 8px; max-width: 100%; }
.yood-card input[type=text] { width: 100%; box-sizing: border-box; }
.yood-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.yood-primary { border: 0; border-radius: 18px; padding: 9px 18px; color: #fff; background: #065fd4; cursor: pointer; }
.yood-secondary { border: 1px solid #666; border-radius: 18px; padding: 8px 16px; color: inherit; background: transparent; cursor: pointer; }
.yood-muted { color: #aaa; font-size: 12px; }
#yood-urlbar { position: fixed; top: 0; left: 0; right: 0; z-index: 2147483647; display: flex; gap: 10px; align-items: center; padding: 10px 14px; background: #171717f2; border-bottom: 1px solid #444; box-shadow: 0 4px 24px #000a; backdrop-filter: blur(10px); transform: translateY(-100%); animation: yood-urlbar-in .3s cubic-bezier(.2,.9,.3,1.18) forwards; }
@keyframes yood-urlbar-in { from { transform: translateY(-100%); opacity: .3; } to { transform: translateY(0); opacity: 1; } }
#yood-urlbar .yood-urlbar-logo { flex: none; width: 26px; height: 26px; border-radius: 50%; background: #ff0000; color: #fff; font-size: 15px; font-weight: 700; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px #f004; }
#yood-urlbar input { flex: 1; min-width: 0; color: #f1f1f1; background: #0000; border: 1px solid #555; border-radius: 10px; padding: 9px 12px; font-size: 14px; outline: none; transition: border-color .15s, box-shadow .15s; }
#yood-urlbar input:focus { border-color: #3ea6ff; box-shadow: 0 0 0 2px #3ea6ff44; }
#yood-urlbar.yood-urlbar-loading input { pointer-events: none; animation: yood-urlbar-glow .9s ease-in-out infinite; }
#yood-urlbar.yood-urlbar-loading .yood-urlbar-go { animation: yood-urlbar-pulse .9s ease-in-out infinite; }
@keyframes yood-urlbar-glow { 0%,100% { border-color: #3ea6ff; box-shadow: 0 0 0 2px #3ea6ff44; } 50% { border-color: #9a6cff; box-shadow: 0 0 14px 2px #3ea6ff99; } }
@keyframes yood-urlbar-pulse { 0%,100% { background: #3ea6ff; } 50% { background: #6b7cff; box-shadow: 0 0 14px #3ea6ff99; } }
#yood-urlbar.yood-urlbar-shake { animation: yood-urlbar-shake .42s ease; }
@keyframes yood-urlbar-shake { 0%,100% { transform: translateY(0); } 20% { transform: translateY(5px); } 40% { transform: translateY(-5px); } 60% { transform: translateY(3px); } 80% { transform: translateY(-2px); } }
.yood-urlbar-close { flex: none; width: 30px; height: 30px; border: 0; border-radius: 50%; color: #eee; background: #ffffff14; cursor: pointer; font-size: 16px; line-height: 1; }
.yood-urlbar-close:hover { background: #ffffff2e; }
#yood-urlbar .yood-urlbar-error { position: absolute; top: calc(100% + 8px); left: 16px; color: #f28b82; font-size: 12px; background: #000c; border: 1px solid #5c2b2b; border-radius: 8px; padding: 6px 10px; box-shadow: 0 4px 14px #000a; }
.yood-entry { display: flex; gap: 8px; align-items: center; padding: 7px 0; border-bottom: 1px solid #383838; }
.yood-entry label { flex: 1; font-size: 13px; }
.yood-progress { height: 5px; background: #444; border-radius: 4px; overflow: hidden; margin-top: 4px; }
.yood-progress > i { display: block; height: 100%; background: #3ea6ff; }
#yood-launcher { position: fixed; right: 18px; bottom: 18px; z-index: 2147483640; border: 0; border-radius: 22px; padding: 10px 16px; color: #fff; background: #065fd4; box-shadow: 0 4px 16px #0008; cursor: pointer; }
#yood-toast { position: fixed; left: 50%; bottom: 30px; transform: translateX(-50%); z-index: 2147483647; background: #292929; color: #fff; padding: 11px 16px; border-radius: 6px; box-shadow: 0 3px 12px #0008; }
#yood-auth-fallback { position: fixed; left: 50%; bottom: 84px; transform: translateX(-50%); z-index: 2147483640; display: flex; gap: 10px; align-items: center; padding: 10px 14px; border: 1px solid #444; border-radius: 12px; background: #181818e8; box-shadow: 0 4px 16px #0008; font-size: 13px; max-width: min(560px, calc(100vw - 32px)); }
#yood-auth-fallback span { flex: 1; }
#yood-auth-fallback button { flex: none; }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation-duration: .001ms !important; transition-duration: .001ms !important; } }
`;
function ensureRoot() {
    let root = document.getElementById("yood-root");
    if (!document.head || !document.body)
        return document.documentElement;
    if (!root) {
        const style = document.createElement("style");
        style.id = "yood-style";
        style.textContent = css;
        document.head.appendChild(style);
        root = document.createElement("div");
        root.id = "yood-root";
        document.body.appendChild(root);
    }
    return root;
}
function currentUrl() { return window.location.href; }
// Accepts only YouTube URLs, matching the host allow-list the Rust side
// uses for its own navigation checks. Bare video IDs and youtu.be short
// links are normalised to a full watch URL.
function parseYoutubeUrlInput(raw) {
    const trimmed = raw.trim();
    if (!trimmed)
        return null;
    const candidate = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
    let url;
    try {
        url = new URL(candidate);
    }
    catch {
        return null;
    }
    if (url.protocol !== "https:")
        return null;
    const host = url.hostname.toLowerCase();
    const allowedHosts = ["youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtube-nocookie.com"];
    if (host === "youtu.be") {
        const id = url.pathname.slice(1).split("/")[0];
        return id ? `https://www.youtube.com/watch?v=${id}${url.search}` : null;
    }
    if (allowedHosts.some((allowed) => host === allowed || host.endsWith(`.${allowed}`))) {
        return url.href;
    }
    return null;
}
function showUrlBar() {
    closeOverlay();
    const bar = document.createElement("div");
    bar.id = "yood-urlbar";
    bar.setAttribute("role", "dialog");
    bar.setAttribute("aria-label", "Open YouTube link");
    const logo = document.createElement("span");
    logo.className = "yood-urlbar-logo";
    logo.textContent = "Y";
    logo.setAttribute("aria-hidden", "true");
    const input = document.createElement("input");
    input.type = "text";
    input.placeholder = "Paste a YouTube video, shorts, or channel link…";
    input.autocomplete = "off";
    input.spellcheck = false;
    const go = button("Go", true);
    go.className = "yood-primary yood-urlbar-go";
    const close = document.createElement("button");
    close.type = "button";
    close.className = "yood-urlbar-close";
    close.textContent = "✕";
    close.setAttribute("aria-label", "Close");
    const error = document.createElement("div");
    error.className = "yood-urlbar-error";
    error.setAttribute("role", "alert");
    error.hidden = true;
    bar.append(logo, input, go, close, error);
    document.body.appendChild(bar);
    const closeBar = () => bar.remove();
    close.addEventListener("click", closeBar);
    const showError = (message) => {
        error.textContent = message;
        error.hidden = false;
        bar.classList.remove("yood-urlbar-shake");
        void bar.offsetWidth;
        bar.classList.add("yood-urlbar-shake");
    };
    const clearError = () => {
        error.hidden = true;
        bar.classList.remove("yood-urlbar-shake");
    };
    const submit = () => {
        const destination = parseYoutubeUrlInput(input.value);
        if (!destination) {
            showError("This link isn't a YouTube link.");
            return;
        }
        clearError();
        bar.classList.add("yood-urlbar-loading");
        input.disabled = true;
        go.disabled = true;
        window.setTimeout(() => {
            bar.remove();
            window.location.href = destination;
        }, 420);
    };
    go.addEventListener("click", submit);
    input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
            event.preventDefault();
            submit();
        }
        if (event.key === "Escape") {
            event.preventDefault();
            closeBar();
        }
    });
    input.addEventListener("input", clearError);
    window.setTimeout(() => input.focus(), 0);
}
function videoUrlFrom(element) {
    const link = element.querySelector('a[href*="/watch?v="], a[href*="/shorts/"], a[href*="youtu.be/"]');
    if (!link)
        return null;
    try {
        const url = new URL(link.href, location.href);
        if (url.hostname === "youtu.be")
            return `https://www.youtube.com/watch?v=${url.pathname.slice(1)}`;
        return url.href;
    }
    catch {
        return null;
    }
}
function textFrom(element, selectors) {
    for (const selector of selectors) {
        const node = element.querySelector(selector);
        const text = node?.textContent?.trim();
        if (text)
            return text;
    }
    return "";
}
function entryFromElement(element) {
    const url = videoUrlFrom(element);
    if (!url)
        return null;
    const parsed = new URL(url);
    const id = parsed.searchParams.get("v") || parsed.pathname.split("/").pop() || url;
    return {
        id,
        url,
        title: textFrom(element, ["#video-title", "a#video-title", "yt-formatted-string"]),
        channel: textFrom(element, ["#channel-name", "ytd-channel-name", ".ytd-channel-name"]),
    };
}
function makeButton(url, context, title = "Download") {
    const button = document.createElement("button");
    button.className = "yood-download-button";
    button.dataset.yoodDownload = url;
    button.type = "button";
    button.setAttribute("aria-label", `${title} with Yood`);
    button.textContent = "↓ Download";
    button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        if (context === "playlist" || context === "channel")
            void showPlaylistDialog(url, context);
        else
            showDownloadDialog(url, context);
    });
    return button;
}
function makeNativePlayerButton(url, title) {
    const button = document.createElement("button");
    button.className = "yood-native-player-button";
    button.type = "button";
    button.dataset.yoodNativePlayer = url;
    button.setAttribute("aria-label", "Play this video in Yood Native Player");
    button.textContent = "▶ Yood Player";
    button.addEventListener("click", async (event) => {
        event.preventDefault();
        event.stopPropagation();
        button.disabled = true;
        try {
            await invoke("open_native_player_url", { url, title });
            toast("Opening Yood Native Player");
        }
        catch (error) {
            toast(`Could not open Yood Player: ${String(error)}`);
        }
        finally {
            button.disabled = false;
        }
    });
    return button;
}
function injectCardButton(card) {
    if (card.querySelector("[data-yood-download]"))
        return;
    const entry = entryFromElement(card);
    if (!entry)
        return;
    const host = card.querySelector("#menu, #buttons, #top-level-buttons-computed, #menu-container") || card;
    host.appendChild(makeButton(entry.url, "video", entry.title || "Download"));
}
function injectVideoButton() {
    const url = currentUrl();
    const collection = /youtube\.com\/playlist\?[^#]*list=/i.test(url) ? "playlist" : /youtube\.com\/channel\//i.test(url) ? "channel" : "video";
    if (!/youtube\.com\/(watch|shorts\/|playlist|channel\/)/i.test(url))
        return;
    const existing = document.querySelector("[data-yood-main-download]");
    if (existing?.dataset.yoodDownload === url && existing.isConnected)
        return;
    document.querySelectorAll("[data-yood-main-action]").forEach((node) => node.remove());
    document.getElementById("yood-video-actions-fallback")?.remove();
    const host = document.querySelector("ytd-watch-metadata #top-level-buttons-computed, #top-level-buttons-computed, ytd-reel-player-overlay-renderer #actions, ytd-shorts #actions, ytd-reel-video-renderer #actions, ytd-reel-player-header-renderer #actions, #top-level-buttons, ytd-menu-renderer #menu");
    document.querySelectorAll("ytd-download-button-renderer").forEach((node) => {
        node.style.display = "none";
    });
    const button = makeButton(url, collection);
    button.dataset.yoodMainDownload = "true";
    button.dataset.yoodMainAction = "true";
    const player = collection === "video" ? makeNativePlayerButton(url, document.title.replace(/ - YouTube$/, "")) : null;
    if (player)
        player.dataset.yoodMainAction = "true";
    if (host) {
        host.append(button);
        if (player)
            host.appendChild(player);
        return;
    }
    // Some YouTube deployments defer their action menu or omit it on Shorts.
    // Keep the functionality available rather than leaving the page without the
    // mandatory Yood controls.
    const fallback = document.createElement("div");
    fallback.id = "yood-video-actions-fallback";
    fallback.className = "yood-video-actions-fallback";
    fallback.dataset.yoodMainAction = "true";
    fallback.append(button);
    if (player)
        fallback.appendChild(player);
    document.body?.appendChild(fallback);
}
function ensureNavigation() {
    if (!document.body || document.getElementById("yood-navigation"))
        return;
    const navigation = document.createElement("nav");
    navigation.id = "yood-navigation";
    navigation.setAttribute("aria-label", "Yood navigation");
    const back = document.createElement("button");
    back.type = "button";
    back.textContent = "←";
    back.title = "Back";
    back.setAttribute("aria-label", "Back");
    back.addEventListener("click", () => window.history.back());
    const forward = document.createElement("button");
    forward.type = "button";
    forward.textContent = "→";
    forward.title = "Forward";
    forward.setAttribute("aria-label", "Forward");
    forward.addEventListener("click", () => window.history.forward());
    const reload = document.createElement("button");
    reload.type = "button";
    reload.textContent = "⟳";
    reload.title = "Reload";
    reload.setAttribute("aria-label", "Reload");
    reload.addEventListener("click", () => window.location.reload());
    const url = document.createElement("button");
    url.type = "button";
    url.className = "yood-url-button";
    url.textContent = "URL";
    url.title = "Open a YouTube link (Ctrl+L)";
    url.setAttribute("aria-label", "Open YouTube link");
    url.addEventListener("click", () => showUrlBar());
    navigation.append(back, forward, reload, url);
    document.body.appendChild(navigation);
}
function scan() {
    ensureRoot();
    if (document.querySelector("ytd-app, ytd-page-manager"))
        window.__YOOD_BOOT__?.complete();
    document.querySelectorAll("ytd-video-renderer, ytd-grid-video-renderer, ytd-rich-item-renderer, ytd-compact-video-renderer, ytd-playlist-video-renderer, ytd-reel-item-renderer").forEach(injectCardButton);
    injectVideoButton();
    ensureNavigation();
    ensureLauncher();
    ensureAuthFallback();
}
const AUTH_FALLBACK_HOSTS = ["accounts.google.com", "accounts.youtube.com"];
function ensureAuthFallback() {
    const host = location.hostname.toLowerCase();
    const onAuthPage = AUTH_FALLBACK_HOSTS.some((candidate) => host === candidate || host.endsWith(`.${candidate}`));
    document.getElementById("yood-auth-fallback")?.remove();
    if (!onAuthPage)
        return;
    const banner = document.createElement("div");
    banner.id = "yood-auth-fallback";
    banner.setAttribute("role", "status");
    const message = document.createElement("span");
    message.textContent = "Sign-in is happening inside Yood. If Google refuses to sign in here, you can finish it in your regular browser.";
    const open = button("Open in browser", true);
    open.addEventListener("click", () => {
        open.disabled = true;
        void invoke("open_external", { url: location.href }).catch((error) => {
            toast(`Could not open browser: ${String(error)}`);
            open.disabled = false;
        });
    });
    banner.append(message, open);
    document.body?.appendChild(banner);
}
const SCANNABLE_CARD_SELECTOR = "ytd-video-renderer, ytd-grid-video-renderer, ytd-rich-item-renderer, ytd-compact-video-renderer, ytd-playlist-video-renderer, ytd-reel-item-renderer";
function needsCardScan(node) {
    return node instanceof Element
        && (node.matches(SCANNABLE_CARD_SELECTOR) || Boolean(node.querySelector(SCANNABLE_CARD_SELECTOR)));
}
function toast(message) {
    document.getElementById("yood-toast")?.remove();
    const node = document.createElement("div");
    node.id = "yood-toast";
    node.textContent = message;
    document.body.appendChild(node);
    window.setTimeout(() => node.remove(), 4200);
}
let closeManagerLive;
function closeOverlay() {
    closeManagerLive?.();
    closeManagerLive = undefined;
    document.querySelector(".yood-overlay")?.remove();
    document.getElementById("yood-urlbar")?.remove();
}
function button(label, primary = false) {
    const result = document.createElement("button");
    result.type = "button";
    result.textContent = label;
    result.className = primary ? "yood-primary" : "yood-secondary";
    return result;
}
function showDownloadDialog(url, context) {
    closeOverlay();
    const overlay = document.createElement("div");
    overlay.className = "yood-overlay";
    const card = document.createElement("section");
    card.className = "yood-card";
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.innerHTML = `<h2>Download with Yood</h2><div class="yood-muted">${escapeHtml(document.title.replace(/ - YouTube$/, ""))}</div>`;
    const modeRow = document.createElement("div");
    modeRow.className = "yood-row";
    modeRow.innerHTML = `<label for="yood-mode">Mode</label><select id="yood-mode"><option value="video">Video</option><option value="audio">Audio (MP3)</option><option value="songs">Songs (MP3 + metadata)</option></select>`;
    const qualityRow = document.createElement("div");
    qualityRow.className = "yood-row";
    qualityRow.innerHTML = `<label for="yood-quality">Quality</label><select id="yood-quality"><option value="best">Best available</option><option value="1080">1080p or best</option><option value="720">720p or best</option><option value="480">480p or best</option><option value="worst">Smallest available</option></select>`;
    const containerRow = document.createElement("div");
    containerRow.className = "yood-row";
    containerRow.innerHTML = `<label for="yood-container">Container</label><select id="yood-container"><option value="mp4">MP4</option><option value="mkv">MKV</option></select>`;
    const advanced = document.createElement("details");
    advanced.innerHTML = `<summary>Advanced</summary><div class="yood-row"><label for="yood-format">yt-dlp format selector</label><input id="yood-format" type="text" placeholder="optional"></div><div class="yood-row"><label><input id="yood-subs" type="checkbox"> Download subtitles</label></div><div class="yood-row"><label><input id="yood-thumb" type="checkbox"> Embed thumbnail and metadata</label></div>`;
    const actions = document.createElement("div");
    actions.className = "yood-actions";
    const cancel = button("Cancel");
    cancel.addEventListener("click", closeOverlay);
    const start = button("Download", true);
    start.addEventListener("click", async () => {
        start.disabled = true;
        try {
            const mode = card.querySelector("#yood-mode").value;
            await invoke("enqueue_download", { request: {
                    url, mode, quality: card.querySelector("#yood-quality").value,
                    container: card.querySelector("#yood-container").value,
                    format_selector: card.querySelector("#yood-format").value || undefined,
                    subtitles: card.querySelector("#yood-subs").checked,
                    embed_thumbnail: card.querySelector("#yood-thumb").checked || mode === "songs",
                    context,
                } });
            closeOverlay();
            toast("Added to Yood's download queue");
        }
        catch (error) {
            toast(`Could not queue download: ${String(error)}`);
            start.disabled = false;
        }
    });
    actions.append(cancel, start);
    card.append(modeRow, qualityRow, containerRow, advanced, actions);
    overlay.appendChild(card);
    overlay.addEventListener("click", (event) => { if (event.target === overlay)
        closeOverlay(); });
    document.body.appendChild(overlay);
    card.querySelector("#yood-mode").addEventListener("change", (event) => {
        const audio = event.target.value !== "video";
        card.querySelector("#yood-quality").disabled = audio;
        card.querySelector("#yood-container").disabled = audio;
    });
}
async function showPlaylistDialog(url, kind) {
    closeOverlay();
    const overlay = document.createElement("div");
    overlay.className = "yood-overlay";
    const card = document.createElement("section");
    card.className = "yood-card";
    card.innerHTML = `<h2>${kind === "playlist" ? "Playlist" : "Channel"} download</h2><p class="yood-muted">Loading available videos…</p>`;
    overlay.appendChild(card);
    document.body.appendChild(overlay);
    try {
        const result = await invoke("inspect_url", { url });
        const entries = result.entries || [];
        card.innerHTML = `<h2>${escapeHtml(result.title || (kind === "playlist" ? "Playlist" : "Channel"))}</h2>`;
        const selectActions = document.createElement("div");
        selectActions.className = "yood-actions";
        const selectAll = button("Select all");
        const selectNone = button("Deselect all");
        const list = document.createElement("div");
        entries.forEach((entry) => {
            const row = document.createElement("div");
            row.className = "yood-entry";
            row.innerHTML = `<input type="checkbox" checked data-yood-entry="${escapeHtml(entry.url)}"><label>${escapeHtml(entry.title || entry.id)}<span class="yood-muted">${escapeHtml(entry.channel || "")}</span></label>`;
            list.appendChild(row);
        });
        selectAll.addEventListener("click", () => list.querySelectorAll("input").forEach((input) => input.checked = true));
        selectNone.addEventListener("click", () => list.querySelectorAll("input").forEach((input) => input.checked = false));
        selectActions.append(selectAll, selectNone);
        card.append(selectActions, list);
        const options = document.createElement("div");
        options.innerHTML = `<h3>Download options</h3><div class="yood-row"><label>Mode</label><select id="yood-list-mode"><option value="video">Video</option><option value="audio">Audio (MP3)</option><option value="songs">Songs (MP3 + metadata)</option></select></div><div class="yood-row"><label>Quality</label><select id="yood-list-quality"><option value="best">Best available</option><option value="1080">1080p or best</option><option value="720">720p or best</option><option value="480">480p or best</option></select></div>`;
        card.appendChild(options);
        const actions = document.createElement("div");
        actions.className = "yood-actions";
        const cancel = button("Cancel");
        cancel.addEventListener("click", closeOverlay);
        const start = button("Download selected", true);
        start.addEventListener("click", async () => {
            const selected = [...list.querySelectorAll("input:checked")].map((input) => input.dataset.yoodEntry).filter((x) => Boolean(x));
            if (!selected.length)
                return toast("Select at least one video");
            start.disabled = true;
            try {
                const mode = card.querySelector("#yood-list-mode").value;
                const quality = card.querySelector("#yood-list-quality").value;
                for (const entryUrl of selected)
                    await invoke("enqueue_download", { request: { url: entryUrl, mode, quality, container: "mp4", context: kind } });
                closeOverlay();
                toast(`Added ${selected.length} video${selected.length === 1 ? "" : "s"} to the queue`);
            }
            catch (error) {
                toast(`Could not queue playlist: ${String(error)}`);
                start.disabled = false;
            }
        });
        actions.append(cancel, start);
        card.appendChild(actions);
    }
    catch (error) {
        card.innerHTML = `<h2>Unable to read ${kind}</h2><p>${escapeHtml(String(error))}</p>`;
    }
}
function ensureLauncher() {
    if (!document.body || document.getElementById("yood-launcher"))
        return;
    const launcher = document.createElement("button");
    launcher.id = "yood-launcher";
    launcher.type = "button";
    launcher.textContent = "Yood";
    launcher.title = "Open Yood downloads and settings";
    launcher.addEventListener("click", () => showManager());
    document.body.appendChild(launcher);
}
function managerState(status) {
    if (status === "downloading" || status === "paused")
        return "active";
    if (status === "queued" || status === "interrupted")
        return "queued";
    if (status === "completed")
        return "completed";
    return "failed";
}
function renderManagerRow(item) {
    const row = document.createElement("div");
    row.className = "yood-entry";
    row.dataset.yoodDownloadId = item.id;
    const safeTitle = escapeHtml(item.title || item.id);
    row.innerHTML = `<div style="flex:1"><strong>${safeTitle}</strong><div class="yood-muted">${escapeHtml(item.channel || "")} · ${escapeHtml(item.status)}${item.speed ? ` · ${escapeHtml(item.speed)}` : ""}${item.eta ? ` · ETA ${escapeHtml(item.eta)}` : ""}</div><div class="yood-progress"><i style="width:${Math.max(0, Math.min(100, item.progress || 0))}%"></i></div></div>`;
    if (["downloading", "paused", "queued", "interrupted"].includes(item.status)) {
        const action = button(item.status === "paused" ? "Resume" : "Pause");
        action.addEventListener("click", async () => { action.disabled = true; try {
            await invoke(item.status === "paused" ? "resume_download" : "pause_download", { id: item.id });
        }
        finally {
            action.disabled = false;
        } });
        row.appendChild(action);
        const cancel = button("Cancel");
        cancel.addEventListener("click", async () => { cancel.disabled = true; try {
            await invoke("cancel_download", { id: item.id });
        }
        finally {
            cancel.disabled = false;
        } });
        row.appendChild(cancel);
    }
    if (item.status === "failed") {
        const retry = button("Retry");
        retry.addEventListener("click", async () => { retry.disabled = true; try {
            await invoke("retry_download", { id: item.id });
        }
        finally {
            retry.disabled = false;
        } });
        row.appendChild(retry);
    }
    if (item.status === "completed" && item.output_path) {
        const open = button("Open");
        open.addEventListener("click", () => invoke("open_download", { id: item.id }));
        row.appendChild(open);
        const folder = button("Show folder");
        folder.addEventListener("click", () => invoke("open_download_folder", { id: item.id }));
        row.appendChild(folder);
        const player = button("Player");
        player.addEventListener("click", () => invoke("open_native_player", { id: item.id }));
        row.appendChild(player);
    }
    if (["completed", "failed", "cancelled"].includes(item.status)) {
        const remove = button("Remove");
        remove.addEventListener("click", async () => {
            remove.disabled = true;
            try {
                await invoke("remove_download", { id: item.id });
                row.remove();
            }
            catch (error) {
                toast(`Could not remove download: ${String(error)}`);
                remove.disabled = false;
            }
        });
        row.appendChild(remove);
    }
    return row;
}
async function showManager() {
    closeOverlay();
    const overlay = document.createElement("div");
    overlay.className = "yood-overlay";
    const card = document.createElement("section");
    card.className = "yood-card";
    card.innerHTML = "<h2>Yood Downloads</h2><p class='yood-muted'>Loading…</p>";
    overlay.appendChild(card);
    document.body.appendChild(overlay);
    try {
        const items = await invoke("list_downloads");
        card.innerHTML = "<h2>Yood Downloads</h2>";
        const sections = {};
        for (const state of ["active", "queued", "completed", "failed"]) {
            const heading = document.createElement("h3");
            heading.textContent = state[0].toUpperCase() + state.slice(1);
            card.appendChild(heading);
            sections[state] = document.createElement("div");
            card.appendChild(sections[state]);
        }
        items.forEach((item) => sections[managerState(item.status)].appendChild(renderManagerRow(item)));
        const actions = document.createElement("div");
        actions.className = "yood-actions";
        const settings = button("Settings");
        settings.addEventListener("click", () => showSettings());
        const close = button("Close", true);
        close.addEventListener("click", closeOverlay);
        actions.append(settings, close);
        card.appendChild(actions);
        // Live-update rows in place as the backend reports progress, rather than
        // re-fetching and rebuilding the whole dialog on a timer or leaving the
        // dialog static until it is reopened.
        let refreshQueued = false;
        const applyUpdate = (item) => {
            const existing = sections[managerState(item.status)].querySelector(`[data-yood-download-id="${item.id}"]`);
            const target = sections[managerState(item.status)];
            const replacement = renderManagerRow(item);
            if (existing && existing.parentElement === target) {
                existing.replaceWith(replacement);
                return;
            }
            // The item moved between sections (e.g. queued -> active) or is new.
            document.querySelector(`[data-yood-download-id="${item.id}"]`)?.remove();
            target.appendChild(replacement);
        };
        const scheduleFullRefresh = () => {
            if (refreshQueued)
                return;
            refreshQueued = true;
            void invoke("list_downloads").then((freshItems) => {
                refreshQueued = false;
                if (!document.body.contains(card))
                    return;
                const seen = new Set();
                freshItems.forEach((item) => { seen.add(item.id); applyUpdate(item); });
                card.querySelectorAll("[data-yood-download-id]").forEach((row) => {
                    if (!seen.has(row.dataset.yoodDownloadId || ""))
                        row.remove();
                });
            }).catch(() => { refreshQueued = false; });
        };
        const unlistenPromise = listenEvent("download-updated", (event) => {
            const item = event.payload;
            if (!item?.id || !document.body.contains(card))
                return;
            // A single item's progress/status update is applied directly. New
            // items and removals require the full list to know their place.
            if (card.querySelector(`[data-yood-download-id="${item.id}"]`)) {
                applyUpdate(item);
            }
            else {
                scheduleFullRefresh();
            }
        });
        closeManagerLive = () => { void unlistenPromise.then((unlisten) => unlisten()); };
    }
    catch (error) {
        card.innerHTML = `<h2>Downloads unavailable</h2><p>${escapeHtml(String(error))}</p>`;
    }
}
function formatTimestamp(value) {
    if (!value)
        return "Never";
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? "Never" : date.toLocaleString();
}
async function showSettings() {
    closeOverlay();
    const overlay = document.createElement("div");
    overlay.className = "yood-overlay";
    const card = document.createElement("section");
    card.className = "yood-card";
    card.innerHTML = "<h2>Yood Settings</h2><p class='yood-muted'>Loading…</p>";
    overlay.appendChild(card);
    document.body.appendChild(overlay);
    try {
        const [settings, appInfo, accountHints] = await Promise.all([
            invoke("get_settings"),
            invoke("get_app_info").catch(() => null),
            invoke("get_account_hints").catch(() => null),
        ]);
        card.innerHTML = "<h2>Yood Settings</h2>";
        card.innerHTML += `
      <h3>Appearance</h3>
      <div class="yood-row"><label for="yood-theme">Theme</label><select id="yood-theme"><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div>
      <h3>Accounts</h3>
      <div class="yood-row"><label for="yood-account-label">Account label<br><span class="yood-muted">A private note to tell accounts apart. Yood never stores your Google password.</span></label><input id="yood-account-label" type="text" placeholder="e.g. Family account"></div>
      <h3>Privacy &amp; Ad Blocking</h3>
      <div class="yood-row"><label>Ad blocking<br><span class="yood-muted">Always on and cannot be disabled.</span></label><span class="yood-muted">${appInfo ? escapeHtml(String(appInfo.filter.rule_count)) + " rules" : "—"}</span></div>
      <div class="yood-row"><label>Filter lists last updated</label><span class="yood-muted">${appInfo ? escapeHtml(formatTimestamp(appInfo.filter.last_success)) : "—"}</span></div>
      <div class="yood-row"><label for="yood-tracking">Tracking protection</label><input id="yood-tracking" type="checkbox"></div>
      <div class="yood-row"><label for="yood-aggressive-tracking">Aggressive tracking protection<br><span class="yood-muted">May affect some YouTube account features.</span></label><input id="yood-aggressive-tracking" type="checkbox"></div>
      <div class="yood-row"><button id="yood-update-filters" type="button" class="yood-secondary">Update filter lists now</button></div>
      <h3>General</h3>
      <div class="yood-row"><label for="yood-log-level">Log level<br><span class="yood-muted">Written to a local file only. Logs never contain passwords, cookies, or tokens.</span></label><select id="yood-log-level"><option>ERROR</option><option>WARN</option><option>INFO</option><option>DEBUG</option><option>TRACE</option></select></div>
      <div class="yood-row"><label for="yood-shortcut">Download shortcut<br><span class="yood-muted">Shift + this key opens the download dialog.</span></label><input id="yood-shortcut" type="text" maxlength="1" placeholder="d"></div>
      <h3>Downloads</h3>
      <div class="yood-row"><label for="yood-concurrency">Concurrent downloads</label><input id="yood-concurrency" type="number" min="1" max="4"></div>
      <div class="yood-row"><label for="yood-folder">Download folder</label><input id="yood-folder" type="text"></div>
      <div class="yood-row"><label for="yood-temp">Temporary folder</label><input id="yood-temp" type="text"></div>
      <div class="yood-row"><label for="yood-clean">Clean temporary files on exit</label><input id="yood-clean" type="checkbox"></div>
      <div class="yood-row"><label for="yood-overwrite">Replace existing files<br><span class="yood-muted">Off keeps both files with a number suffix.</span></label><input id="yood-overwrite" type="checkbox"></div>
      <h3>Advanced Downloads</h3>
      <div class="yood-row"><label for="yood-embed-thumb">Embed thumbnail and metadata by default</label><input id="yood-embed-thumb" type="checkbox"></div>
      <div class="yood-row"><label for="yood-subs">Download subtitles by default</label><input id="yood-subs" type="checkbox"></div>
      <div class="yood-row"><label for="yood-sub-lang">Subtitle language</label><input id="yood-sub-lang" type="text" maxlength="16" placeholder="en"></div>
      <h3>About</h3>
      <div class="yood-row"><label>Yood version</label><span class="yood-muted">${appInfo ? escapeHtml(appInfo.version) : "—"}</span></div>
      ${appInfo ? appInfo.binaries.map((binary) => `<div class="yood-row"><label>${escapeHtml(binary.name)}</label><span class="yood-muted">${binary.available ? escapeHtml(binary.version || "installed") + (binary.bundled ? " (bundled)" : " (system)") : "Not available"}</span></div>`).join("") : ""}
      <div class="yood-row"><button id="yood-update-ytdlp" type="button" class="yood-secondary">Update yt-dlp now</button></div>
    `;
        card.querySelector("#yood-theme").value = settings.appearance;
        card.querySelector("#yood-account-label").value = accountHints || "";
        card.querySelector("#yood-tracking").checked = settings.tracking_protection;
        card.querySelector("#yood-aggressive-tracking").checked = settings.aggressive_tracking_protection;
        card.querySelector("#yood-concurrency").value = String(settings.max_concurrent_downloads);
        card.querySelector("#yood-folder").value = settings.download_directory;
        card.querySelector("#yood-temp").value = settings.temp_directory;
        card.querySelector("#yood-clean").checked = settings.cleanup_temp_files;
        card.querySelector("#yood-overwrite").checked = settings.overwrite_existing;
        card.querySelector("#yood-embed-thumb").checked = settings.embed_thumbnail;
        card.querySelector("#yood-subs").checked = settings.download_subtitles;
        card.querySelector("#yood-sub-lang").value = settings.subtitle_language;
        card.querySelector("#yood-log-level").value = settings.log_level;
        card.querySelector("#yood-shortcut").value = settings.shortcut_download;
        const updateFilters = card.querySelector("#yood-update-filters");
        updateFilters?.addEventListener("click", async () => {
            updateFilters.disabled = true;
            const original = updateFilters.textContent;
            updateFilters.textContent = "Updating…";
            try {
                await invoke("update_filter_lists");
                toast("Filter lists updated");
                showSettings();
            }
            catch (error) {
                toast(`Could not update filter lists: ${String(error)}`);
                updateFilters.disabled = false;
                updateFilters.textContent = original;
            }
        });
        const updateYtdlp = card.querySelector("#yood-update-ytdlp");
        updateYtdlp?.addEventListener("click", async () => {
            updateYtdlp.disabled = true;
            const original = updateYtdlp.textContent;
            updateYtdlp.textContent = "Updating…";
            try {
                const version = await invoke("update_ytdlp");
                toast(`yt-dlp updated to ${String(version)}`);
                showSettings();
            }
            catch (error) {
                toast(`Could not update yt-dlp: ${String(error)}`);
                updateYtdlp.disabled = false;
                updateYtdlp.textContent = original;
            }
        });
        const actions = document.createElement("div");
        actions.className = "yood-actions";
        const cancel = button("Cancel");
        cancel.addEventListener("click", closeOverlay);
        const save = button("Save", true);
        save.addEventListener("click", async () => {
            save.disabled = true;
            try {
                const appearance = card.querySelector("#yood-theme").value;
                const accountLabel = card.querySelector("#yood-account-label").value.trim();
                if (accountLabel)
                    await invoke("set_account_hints", { value: accountLabel });
                else
                    await invoke("clear_account_hints").catch(() => undefined);
                await invoke("update_settings", { patch: {
                        appearance,
                        tracking_protection: card.querySelector("#yood-tracking").checked,
                        aggressive_tracking_protection: card.querySelector("#yood-aggressive-tracking").checked,
                        max_concurrent_downloads: Number(card.querySelector("#yood-concurrency").value),
                        download_directory: card.querySelector("#yood-folder").value,
                        temp_directory: card.querySelector("#yood-temp").value,
                        cleanup_temp_files: card.querySelector("#yood-clean").checked,
                        overwrite_existing: card.querySelector("#yood-overwrite").checked,
                        embed_thumbnail: card.querySelector("#yood-embed-thumb").checked,
                        download_subtitles: card.querySelector("#yood-subs").checked,
                        subtitle_language: card.querySelector("#yood-sub-lang").value.trim() || "en",
                        log_level: card.querySelector("#yood-log-level").value,
                        shortcut_download: card.querySelector("#yood-shortcut").value.trim().toLowerCase() || "d",
                    } });
                applyAppearance(appearance, true);
                downloadShortcut = card.querySelector("#yood-shortcut").value.trim().toLowerCase() || "d";
                closeOverlay();
                toast("Settings saved");
            }
            catch (error) {
                toast(`Could not save settings: ${String(error)}`);
                save.disabled = false;
            }
        });
        actions.append(cancel, save);
        card.appendChild(actions);
    }
    catch (error) {
        card.innerHTML = `<h2>Settings unavailable</h2><p>${escapeHtml(String(error))}</p>`;
    }
}
function escapeHtml(value) { const node = document.createElement("span"); node.textContent = value; return node.innerHTML; }
const AD_RENDERER_SELECTORS = [
    "ytd-ad-slot-renderer",
    "ytd-in-feed-ad-layout-renderer",
    "ytd-banner-promo-renderer",
    "ytd-display-ad-renderer",
    "ytd-promoted-video-renderer",
    "ytd-compact-promoted-video-renderer",
    "ytd-promoted-sparkles-web-renderer",
    "ytd-companion-slot-renderer",
    "ytd-action-companion-ad-renderer",
    "ytd-player-legacy-desktop-watch-ads-renderer",
    "#player-ads",
    "#masthead-ad",
    "#panels ytd-engagement-panel-section-list-renderer[target-id=engagement-panel-ads]",
    ".ytp-ad-module",
    ".ytp-ad-overlay-container",
    ".ytp-ad-player-overlay",
    ".ytp-ad-text",
    ".video-ads",
    ".ytd-ad-slot-renderer",
];
const AD_RENDERER_SELECTOR = AD_RENDERER_SELECTORS.join(",");
const AD_CARD_SELECTOR = [
    "ytd-rich-item-renderer",
    "ytd-video-renderer",
    "ytd-grid-video-renderer",
    "ytd-compact-video-renderer",
    "ytd-playlist-video-renderer",
    "ytd-reel-item-renderer",
].join(",");
const STATIC_AD_SELECTORS = [...AD_RENDERER_SELECTORS, "[data-yood-ad-removed]"]
    .join(",");
const FEED_CARD_SELECTOR = "ytd-rich-item-renderer";
const PLAYABLE_CARD_SELECTOR = 'a[href*="/watch?v="], a[href*="/shorts/"], a[href*="/live/"], ytd-rich-grid-media, ytd-reel-item-renderer';
function containsAdPayload(value) {
    const queue = [value];
    const seen = new Set();
    for (let inspected = 0; queue.length && inspected < 128; inspected += 1) {
        const current = queue.pop();
        if (!current || typeof current !== "object" || seen.has(current))
            continue;
        seen.add(current);
        for (const [key, nested] of Object.entries(current)) {
            if (/^(?:ad(?:slot|placements?|slots|break|layout|renderer|playbackcontext)|playerads|promoted(?:sparkles)?(?:renderer)?|companion(?:slot)?renderer)$/i.test(key))
                return true;
            if (nested && typeof nested === "object")
                queue.push(nested);
        }
    }
    return false;
}
function removeGhostFeedCards(root) {
    const cards = [];
    if (root instanceof Element && root.matches(FEED_CARD_SELECTOR))
        cards.push(root);
    cards.push(...root.querySelectorAll(FEED_CARD_SELECTOR));
    let removed = false;
    for (const card of cards) {
        if (!card.isConnected || card.querySelector(PLAYABLE_CARD_SELECTOR))
            continue;
        const data = card.data;
        if (card.querySelector(AD_RENDERER_SELECTOR) || containsAdPayload(data)) {
            card.setAttribute("data-yood-ad-removed", "");
            card.remove();
            removed = true;
        }
    }
    return removed;
}
function removeKnownAds(root = document) {
    const renderers = [];
    if (root instanceof Element && root.matches(AD_RENDERER_SELECTOR))
        renderers.push(root);
    renderers.push(...root.querySelectorAll(AD_RENDERER_SELECTOR));
    let removed = false;
    for (const renderer of renderers) {
        if (!renderer.isConnected)
            continue;
        // Removing the ad's own card (rather than just the inner ad element)
        // avoids leaving a visibly blank tile in the feed. YouTube's own grid
        // rows are a fixed-count layout decided by its JS at render time, not a
        // fluid CSS grid, so a removed card is *not* automatically backfilled by
        // the next video the way a real CSS grid would. Yood used to fix that by
        // physically moving later videos into the gap, but that corrupted
        // YouTube's internal row/virtualization bookkeeping and made rows render
        // with fewer items than fit on screen. Leaving a gap where an ad used to
        // be is standard, expected ad-blocker behaviour (this is what Brave and
        // uBlock Origin do too) and is far safer than reaching into YouTube's
        // component internals.
        const card = renderer.closest(AD_CARD_SELECTOR);
        if (card) {
            card.setAttribute("data-yood-ad-removed", "");
            card.remove();
        }
        else {
            renderer.remove();
        }
        removed = true;
    }
    if (removeGhostFeedCards(root))
        removed = true;
    return removed;
}
const PLAYER_AD_RESPONSE_KEYS = new Set([
    "adplacements",
    "adslots",
    "adcueranges",
    "playerads",
    "adparams",
    "adbreakheartbeatparams",
    "adplaybackcontext",
    "adbreakservicerenderer",
    "adslotrenderer",
    "adlayoutrenderer",
    "instreamvideoadrenderer",
    "playerlegacydesktopwatchadsrenderer",
    "promotedsparkleswebrenderer",
    "companionslotrenderer",
    "actioncompanionadrenderer",
]);
function stripPlayerAdPayload(payload) {
    const queue = [payload];
    const seen = new Set();
    let changed = false;
    for (let inspected = 0; queue.length && inspected < 20000; inspected += 1) {
        const current = queue.pop();
        if (!current || typeof current !== "object" || seen.has(current))
            continue;
        seen.add(current);
        for (const [key, nested] of Object.entries(current)) {
            if (PLAYER_AD_RESPONSE_KEYS.has(key.toLowerCase())) {
                delete current[key];
                changed = true;
            }
            else if (nested && typeof nested === "object") {
                queue.push(nested);
            }
        }
    }
    return changed;
}
function isPlayerApiRequest(url) {
    try {
        const parsed = new URL(url);
        return (parsed.hostname === "youtube.com" || parsed.hostname.endsWith(".youtube.com"))
            && /^\/youtubei\/v1\/(?:player|next)(?:\/|$)/.test(parsed.pathname);
    }
    catch {
        return false;
    }
}
async function stripPlayerAdsFromResponse(response) {
    const contentType = response.headers.get("content-type") || "";
    if (!/json/i.test(contentType))
        return response;
    try {
        const raw = await response.clone().text();
        if (raw.length > 8 * 1024 * 1024)
            return response;
        const payload = JSON.parse(raw);
        if (!stripPlayerAdPayload(payload))
            return response;
        const headers = new Headers(response.headers);
        headers.delete("content-length");
        headers.delete("content-encoding");
        return new Response(JSON.stringify(payload), {
            status: response.status,
            statusText: response.statusText,
            headers,
        });
    }
    catch {
        return response;
    }
}
function installAdAndTrackingLayer() {
    if (window.__yoodFilteringInstalled)
        return;
    window.__yoodFilteringInstalled = true;
    const adTokens = ["doubleclick.net", "googleadservices.com", "googlesyndication.com", "adservice.google.com", "jnn-pa.googleapis.com", "youtube.com/api/stats/ads", "youtube.com/pagead", "youtube.com/ptracking", "youtube.com/get_midroll_info", "youtube.com/ad_break"];
    const trackingTokens = [];
    const decisionCache = new Map();
    let cosmeticTimer;
    let cosmeticRequest = 0;
    let lastCosmeticUrl = "";
    void invoke("tracking_protection_enabled").then((enabled) => { if (enabled === true)
        trackingTokens.push("google-analytics.com", "googletagmanager.com", "youtube.com/api/stats/qoe"); }).catch(() => undefined);
    const requestUrl = (value) => { try {
        return new URL(String(value), location.href).href;
    }
    catch {
        return null;
    } };
    const fastBlocked = (value) => {
        const raw = requestUrl(value);
        if (!raw)
            return false;
        try {
            const url = new URL(raw);
            return [...adTokens, ...trackingTokens].some((token) => url.hostname === token || url.hostname.endsWith(`.${token}`) || url.href.includes(token));
        }
        catch {
            return false;
        }
    };
    const needsEngineCheck = (value) => {
        try {
            const url = new URL(value);
            // Do not delay normal YouTube API, image, and media requests through an
            // IPC round trip.  Those paths are handled by the native layer on Linux;
            // the browser-side engine is reserved for URLs that actually resemble an
            // ad/tracker rule and are not covered by the synchronous mandatory list.
            const candidate = `${url.hostname}${url.pathname}${url.search}`;
            return /(?:^|[./?&=_-])(?:ad|ads|adx|advert(?:isement)?|promoted|sponsor(?:ed)?)(?:$|[./?&=_-])/i.test(candidate);
        }
        catch {
            return false;
        }
    };
    const engineBlocked = async (url, requestType) => {
        const key = `${requestType}:${url}`;
        const cached = decisionCache.get(key);
        if (cached !== undefined)
            return cached;
        try {
            const blocked = await invoke("check_filter_request", { url, source_url: location.href, request_type: requestType }) === true;
            if (decisionCache.size >= 2048)
                decisionCache.delete(decisionCache.keys().next().value);
            decisionCache.set(key, blocked);
            return blocked;
        }
        catch {
            return false;
        }
    };
    const originalFetch = window.fetch.bind(window);
    window.fetch = (async (input, init) => {
        const url = requestUrl(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
        if (url && fastBlocked(url))
            throw new DOMException("Blocked by Yood", "AbortError");
        if (url && needsEngineCheck(url) && await engineBlocked(url, "fetch"))
            throw new DOMException("Blocked by Yood", "AbortError");
        const response = await originalFetch(input, init);
        return url && isPlayerApiRequest(url) ? stripPlayerAdsFromResponse(response) : response;
    });
    const OriginalXHR = window.XMLHttpRequest;
    window.XMLHttpRequest = class extends OriginalXHR {
        constructor() {
            super(...arguments);
            this.yoodUrl = "";
            this.yoodGeneration = 0;
            this.yoodCancelled = false;
            this.yoodAsync = true;
        }
        open(method, url, ...rest) {
            this.yoodUrl = requestUrl(url) || "";
            this.yoodGeneration += 1;
            this.yoodCancelled = false;
            this.yoodAsync = rest[0] !== false;
            super.open(method, url.toString(), rest[0] ?? true, rest[1], rest[2]);
        }
        send(body) {
            const generation = this.yoodGeneration;
            const url = this.yoodUrl;
            if (!url) {
                super.send(body);
                return;
            }
            if (fastBlocked(url)) {
                this.abort();
                return;
            }
            if (!this.yoodAsync || !needsEngineCheck(url)) {
                super.send(body);
                return;
            }
            void engineBlocked(url, "xmlhttprequest").then((blocked) => {
                if (generation !== this.yoodGeneration || this.yoodCancelled)
                    return;
                if (blocked) {
                    this.abort();
                    return;
                }
                super.send(body);
            });
        }
        abort() { this.yoodCancelled = true; super.abort(); }
    };
    const updateStyle = (selectors = []) => {
        if (!document.head)
            return;
        let style = document.getElementById("yood-ad-style");
        if (!style) {
            style = document.createElement("style");
            style.id = "yood-ad-style";
            document.head.appendChild(style);
        }
        // Each remote selector gets its own rule.  If YouTube's WebKit does not
        // understand one modern selector (for example :has()), it discards only
        // that rule instead of the entire built-in ad stylesheet.
        style.textContent = `${STATIC_AD_SELECTORS} { display: none !important; }\n${selectors.map((selector) => `${selector} { display: none !important; }`).join("\n")}`;
    };
    const refreshCosmetics = () => {
        cosmeticTimer = undefined;
        if (!/^https?:$/i.test(location.protocol))
            return;
        const pageUrl = location.href;
        const request = ++cosmeticRequest;
        lastCosmeticUrl = pageUrl;
        const classes = new Set();
        const ids = new Set();
        for (const element of Array.from(document.querySelectorAll("[class],[id]")).slice(0, 2500)) {
            element.classList.forEach((value) => classes.add(value));
            if (element.id)
                ids.add(element.id);
        }
        void invoke("get_filter_script_rules", { page_url: pageUrl, classes: [...classes], ids: [...ids] }).then((rules) => {
            if (request !== cosmeticRequest || pageUrl !== location.href)
                return;
            const value = rules;
            const selectors = (value.cosmetic_selectors || []).filter((selector) => typeof selector === "string" && selector.length <= 2048 && !/[{};]/.test(selector));
            updateStyle(selectors);
            removeKnownAds();
        }).catch(() => updateStyle());
    };
    const scheduleCosmetics = (force = false) => {
        if (!force && lastCosmeticUrl === location.href)
            return;
        if (cosmeticTimer === undefined)
            cosmeticTimer = window.setTimeout(refreshCosmetics, 450);
    };
    const installDomLayer = () => {
        updateStyle();
        window.__YOOD_RELEASE_AD_SHIELD__?.();
        window.__YOOD_RELEASE_AD_SHIELD__ = undefined;
        new MutationObserver((records) => {
            for (const record of records) {
                for (const node of record.addedNodes) {
                    if (node instanceof Element)
                        removeKnownAds(node);
                }
            }
        }).observe(document.documentElement, { childList: true, subtree: true });
        removeKnownAds();
        scheduleCosmetics(true);
    };
    if (document.head && document.documentElement)
        installDomLayer();
    else
        window.addEventListener("DOMContentLoaded", installDomLayer, { once: true });
    const refreshForNavigation = () => { lastCosmeticUrl = ""; scheduleCosmetics(); };
    window.addEventListener("yt-navigate-finish", refreshForNavigation);
    window.addEventListener("yt-page-data-updated", refreshForNavigation);
}
function installPlayerAdSkip() {
    // Backup layer for in-stream video ads (pre/mid/post-roll). Those segments
    // are served from googlevideo.com — the same CDN as real content — so they
    // can never be blocked by URL. When YouTube still schedules one (the
    // player-response stripping above missed it), fast-forward through it and
    // press Skip the moment YouTube enables it. Nothing here runs unless the
    // player itself reports an ad *and* ad UI is present, so normal playback is
    // never touched. No polling while idle: the interval only lives while an
    // ad is on screen.
    let skipTimer;
    let mutedForAd = false;
    let rateForAd = false;
    const player = () => {
        const node = document.getElementById("movie_player");
        return node && node.isConnected ? node : null;
    };
    const isAdShowing = (node) => node.classList.contains("ad-showing") || node.classList.contains("ad-interrupting");
    const stopTimer = () => {
        if (skipTimer !== undefined) {
            window.clearInterval(skipTimer);
            skipTimer = undefined;
        }
    };
    const restorePlayback = (node) => {
        const video = node.querySelector("video.html5-main-video");
        if (!video)
            return;
        try {
            if (mutedForAd) {
                video.muted = false;
                mutedForAd = false;
            }
            if (rateForAd) {
                video.playbackRate = 1;
                rateForAd = false;
            }
        }
        catch { /* transient state; the ad is gone anyway */ }
    };
    const handleAdFrame = () => {
        const node = player();
        if (!node || !isAdShowing(node)) {
            stopTimer();
            if (node)
                restorePlayback(node);
            return;
        }
        // Second confirmation from actual ad UI before touching anything.
        const skip = node.querySelector(".ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-skip-ad-button-modern");
        const overlay = node.querySelector(".ytp-ad-player-overlay, .ytp-ad-text, .ytp-ad-message-container, .ytp-ad-skip-button-container");
        if (!skip && !overlay)
            return;
        const video = node.querySelector("video.html5-main-video");
        if (video) {
            try {
                if (!video.muted) {
                    video.muted = true;
                    mutedForAd = true;
                }
                if (video.playbackRate !== 16) {
                    video.playbackRate = 16;
                    rateForAd = true;
                }
                // Unskippable bumper: jump near the end once duration is known.
                // Guarded to short clips so a misdetection can never skip real content.
                if (Number.isFinite(video.duration) && video.duration > 0 && video.duration <= 120
                    && video.currentTime < video.duration - 0.3) {
                    video.currentTime = video.duration - 0.1;
                }
            }
            catch { /* read-only in odd states; next tick retries */ }
        }
        if (skip && skip.offsetParent !== null)
            skip.click();
    };
    const onClassChange = () => {
        const node = player();
        if (!node)
            return;
        if (isAdShowing(node)) {
            handleAdFrame();
            if (skipTimer === undefined)
                skipTimer = window.setInterval(handleAdFrame, 400);
        }
        else {
            stopTimer();
            restorePlayback(node);
        }
    };
    const watch = () => {
        const node = player();
        if (!node)
            return;
        if (node.hasAttribute("data-yood-adwatch")) {
            onClassChange();
            return;
        }
        node.setAttribute("data-yood-adwatch", "");
        new MutationObserver(onClassChange).observe(node, { attributes: true, attributeFilter: ["class"] });
        onClassChange();
    };
    if (document.getElementById("movie_player"))
        watch();
    else
        window.addEventListener("DOMContentLoaded", watch, { once: true });
    // Watch pages mount the player lazily; re-attach after SPA navigations.
    window.addEventListener("yt-navigate-finish", () => window.setTimeout(watch, 600));
}
function installDevtoolsBlock() {
    // Minimal-browser policy: no DevTools / web inspector.
    // The native side is locked with `.devtools(false)` in Rust; this only
    // swallows the inspector keyboard shortcuts so they never reach YouTube
    // or trigger anything in debug builds. Normal browsing keys (Ctrl+L for
    // Yood's URL bar, Shift+D for download) are intentionally left alone.
    document.addEventListener("keydown", (event) => {
        const key = event.key.toLowerCase();
        if (event.key === "F12" || event.code === "F12") {
            event.preventDefault();
            event.stopPropagation();
            return;
        }
        if ((event.ctrlKey || event.metaKey) && event.shiftKey && ["i", "j", "c", "k", "e"].includes(key)) {
            event.preventDefault();
            event.stopPropagation();
            return;
        }
        if (event.metaKey && event.altKey && ["i", "j", "c"].includes(key)) {
            event.preventDefault();
            event.stopPropagation();
            return;
        }
        if (event.ctrlKey && !event.shiftKey && !event.metaKey && !event.altKey && key === "u") {
            event.preventDefault();
            event.stopPropagation();
        }
    }, true);
}
function install() {
    if (window.__YOOD__)
        return;
    window.__YOOD__ = { invoke, scan };
    installDevtoolsBlock();
    installPlayerAdSkip();
    installAppearance();
    void invoke("get_settings").then((settings) => {
        const shortcut = settings.shortcut_download;
        if (typeof shortcut === "string" && /^[a-z0-9]$/i.test(shortcut)) {
            downloadShortcut = shortcut.toLowerCase();
        }
    }).catch(() => undefined);
    void invoke("get_desktop_environment").then((desktop) => {
        if (["hyprland", "niri", "plasma", "gnome", "other"].includes(String(desktop))) {
            document.documentElement?.setAttribute("data-yood-desktop", String(desktop));
        }
    }).catch(() => undefined);
    let scheduled = false;
    let navigationRetry;
    const schedule = () => { if (scheduled)
        return; scheduled = true; window.setTimeout(() => { scheduled = false; scan(); }, 180); };
    const scheduleForNavigation = () => {
        schedule();
        if (navigationRetry !== undefined)
            window.clearTimeout(navigationRetry);
        // YouTube upgrades several custom elements after its navigation event. A
        // single bounded retry catches that phase without polling the whole DOM.
        navigationRetry = window.setTimeout(schedule, 850);
    };
    ["yt-navigate-finish", "yt-page-data-updated", "popstate", "DOMContentLoaded"].forEach((event) => window.addEventListener(event, scheduleForNavigation));
    const observePage = () => {
        if (document.documentElement) {
            new MutationObserver((records) => {
                for (const record of records) {
                    for (const node of record.addedNodes) {
                        if (needsCardScan(node)) {
                            schedule();
                            return;
                        }
                    }
                }
            }).observe(document.documentElement, { childList: true, subtree: true });
        }
    };
    observePage();
    if (!document.documentElement)
        window.addEventListener("DOMContentLoaded", observePage, { once: true });
    document.addEventListener("keydown", (event) => {
        if (event.shiftKey && event.key.toLowerCase() === downloadShortcut && !/input|textarea|select/i.test(event.target?.tagName || "")) {
            event.preventDefault();
            showDownloadDialog(currentUrl(), "shortcut");
            return;
        }
        // Ctrl+L / Cmd+L opens the URL bar. Matches on the physical key
        // (`code`), not the layout-dependent `key`, so it works under any
        // keyboard layout (e.g. Hebrew). Capture phase so YouTube's own
        // handlers cannot swallow it first.
        const isL = event.code === "KeyL" || event.key.toLowerCase() === "l";
        if ((event.ctrlKey || event.metaKey) && isL) {
            event.preventDefault();
            showUrlBar();
        }
    }, true);
    installAdAndTrackingLayer();
    if (document.readyState === "loading")
        window.addEventListener("DOMContentLoaded", scan, { once: true });
    else
        scan();
}
install();
