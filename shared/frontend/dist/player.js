const video = document.querySelector("#player");
const title = document.querySelector("#title");
const playerStatus = document.querySelector("#status");
const stats = document.querySelector("#stats");
const speed = document.querySelector("#speed");
const pictureInPicture = document.querySelector("#pip");
const fullscreen = document.querySelector("#fullscreen");
const timeline = document.querySelector("#timeline");
const timeLabel = document.querySelector("#time");
const volumeSlider = document.querySelector("#volume");
const muteButton = document.querySelector("#mute");
function formatTime(seconds) {
    if (!Number.isFinite(seconds) || seconds < 0)
        return "0:00";
    const whole = Math.floor(seconds);
    const minutes = Math.floor(whole / 60);
    const remaining = whole % 60;
    return `${minutes}:${remaining.toString().padStart(2, "0")}`;
}
function updateTimeDisplay() {
    timeLabel.textContent = `${formatTime(video.currentTime)} / ${formatTime(video.duration || 0)}`;
    if (video.duration && Number.isFinite(video.duration) && video.duration > 0) {
        timeline.value = String(Math.round((video.currentTime / video.duration) * 1000));
    }
}
function updateStats() {
    const resolution = video.videoWidth && video.videoHeight ? `${video.videoWidth}×${video.videoHeight}` : "unknown resolution";
    let source = "local file";
    try {
        const parsed = new URL(video.currentSrc || "");
        source = parsed.hostname || "local file";
    }
    catch { /* keep local file */ }
    stats.textContent = `${resolution} · ${source}`;
}
async function load(payload) {
    const data = payload;
    if (!data.url)
        return;
    video.src = data.url;
    title.textContent = data.title || "Yood Native Player";
    playerStatus.textContent = "Ready";
    try {
        await video.play();
    }
    catch {
        playerStatus.textContent = "Ready — press play";
    }
}
const tauri = window.__TAURI__;
void tauri?.event?.listen?.("player-load", (event) => { void load(event.payload); });
void (async () => {
    // The Rust side already resolves playback to a ready-to-use URL (either a
    // file:// URL for a completed download or an https:// stream URL for a
    // "play in Yood Player" request) and sends it as `source`. Do not attempt
    // to re-resolve it as a filesystem path here.
    const payload = await tauri?.core?.invoke?.("take_player_source");
    if (payload?.source) {
        await load({ url: payload.source, title: payload.title });
    }
})().catch(() => { playerStatus.textContent = "Native player source unavailable"; });
video.addEventListener("error", () => { playerStatus.textContent = "The media could not be played by the platform player."; });
video.addEventListener("loadedmetadata", () => {
    playerStatus.textContent = "Ready";
    updateStats();
    updateTimeDisplay();
});
video.addEventListener("timeupdate", updateTimeDisplay);
video.addEventListener("durationchange", updateTimeDisplay);
video.addEventListener("waiting", () => { playerStatus.textContent = "Buffering…"; });
video.addEventListener("playing", () => { playerStatus.textContent = "Playing"; });
video.addEventListener("pause", () => { playerStatus.textContent = "Paused"; });
speed.addEventListener("change", () => {
    video.playbackRate = Number(speed.value) || 1;
});
timeline.addEventListener("input", () => {
    if (video.duration && Number.isFinite(video.duration) && video.duration > 0) {
        video.currentTime = (Number(timeline.value) / 1000) * video.duration;
    }
});
function syncVolumeUi() {
    volumeSlider.value = String(video.volume);
    muteButton.textContent = video.muted || video.volume === 0 ? "Unmute" : "Mute";
}
volumeSlider.addEventListener("input", () => {
    video.muted = false;
    video.volume = Number(volumeSlider.value);
    syncVolumeUi();
});
muteButton.addEventListener("click", () => {
    video.muted = !video.muted;
    syncVolumeUi();
});
fullscreen.addEventListener("click", () => {
    void video.requestFullscreen?.();
});
pictureInPicture.addEventListener("click", async () => {
    const player = video;
    try {
        if (document.pictureInPictureElement) {
            await document.exitPictureInPicture();
        }
        else if (player.requestPictureInPicture && document.pictureInPictureEnabled) {
            await player.requestPictureInPicture();
        }
    }
    catch {
        playerStatus.textContent = "Picture-in-picture is unavailable on this platform.";
    }
});
window.addEventListener("keydown", (event) => {
    const target = event.target;
    if (target && ["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(target.tagName)) {
        if (target === timeline && (event.key === "ArrowLeft" || event.key === "ArrowRight"))
            return;
        return;
    }
    switch (event.key.toLowerCase()) {
        case " ":
        case "k":
            event.preventDefault();
            if (video.paused)
                void video.play();
            else
                video.pause();
            break;
        case "f":
            event.preventDefault();
            void video.requestFullscreen?.();
            break;
        case "m":
            event.preventDefault();
            video.muted = !video.muted;
            syncVolumeUi();
            break;
        case "j":
            video.currentTime = Math.max(0, video.currentTime - 10);
            break;
        case "l":
            video.currentTime = Math.min(video.duration || Infinity, video.currentTime + 10);
            break;
        case "arrowleft":
            video.currentTime = Math.max(0, video.currentTime - 5);
            break;
        case "arrowright":
            video.currentTime = Math.min(video.duration || Infinity, video.currentTime + 5);
            break;
        case "arrowup":
            event.preventDefault();
            video.volume = Math.min(1, video.volume + 0.05);
            syncVolumeUi();
            break;
        case "arrowdown":
            event.preventDefault();
            video.volume = Math.max(0, video.volume - 0.05);
            syncVolumeUi();
            break;
    }
});
syncVolumeUi();
export {};
