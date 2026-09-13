# Yood — Technical Product Specification

**Version:** 1.0  
**Status:** Development Specification  
**Target Platforms:** Windows and Linux  
**Primary Use:** Private/family use  
**Project Type:** Desktop YouTube-focused application

---

## 1. Product Overview

Yood is a lightweight desktop application for Windows and Linux whose purpose is to provide a YouTube-first experience without functioning as a general-purpose web browser.

The application should feel and behave like the normal YouTube website:

- Real YouTube interface
- Real Google/YouTube account
- Real subscriptions
- Real recommendations
- Real playlists
- Real comments
- Real history
- Real Watch Later
- Real notifications
- Real YouTube settings
- Real YouTube player

The application should remove advertising from the viewing experience and add an integrated download system powered by bundled `yt-dlp` and `FFmpeg`.

The application must prioritize:

1. Very low RAM usage
2. Very low idle CPU usage
3. No memory leaks
4. Fast startup
5. Native-feeling desktop behavior
6. Minimal background activity
7. No application telemetry
8. No application analytics
9. No user-data collection
10. A UI that feels like YouTube itself rather than a generic browser

Yood is not intended to become a general-purpose browser.

---

# 2. Core Technology Decision

## 2.1 Recommended Stack

The implementation should use:

| Component | Technology |
|---|---|
| Main application | Rust |
| Desktop framework | Tauri |
| Frontend | HTML/CSS/TypeScript |
| Windows WebView | WebView2 |
| Linux WebView | WebKitGTK |
| Download engine | yt-dlp |
| Media muxing/conversion | FFmpeg |
| Configuration | Rust-managed local configuration |
| Secure credentials/session data | OS-native secure storage where practical |
| Packaging | Native Windows installer + Linux package/AppImage as appropriate |

Electron and a bundled Chromium browser should **not** be used.

CEF should also not be used unless a later technical investigation proves that the required YouTube functionality cannot be implemented reliably with Tauri's platform WebViews.

The primary goal is to avoid shipping a complete browser engine and therefore reduce memory footprint.

---

# 3. Application Architecture

The architecture should be separated into the following major components:

```text
┌─────────────────────────────────────────────┐
│                  Yood App                   │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │         YouTube WebView Layer         │  │
│  │                                       │  │
│  │  Official YouTube website             │  │
│  │  Google authentication                 │  │
│  │  YouTube player                        │  │
│  │  YouTube UI                            │  │
│  └───────────────────┬───────────────────┘  │
│                      │                      │
│  ┌───────────────────▼───────────────────┐  │
│  │        Yood Integration Layer         │  │
│  │                                       │  │
│  │  Ad blocking                           │  │
│  │  Download button injection             │  │
│  │  Download UI integration               │  │
│  │  Native player integration             │  │
│  │  Keyboard shortcuts                    │  │
│  │  Deep links                            │  │
│  └───────────────────┬───────────────────┘  │
│                      │                      │
│  ┌───────────────────▼───────────────────┐  │
│  │          Rust Application Core        │  │
│  │                                       │  │
│  │  Download manager                     │  │
│  │  yt-dlp controller                    │  │
│  │  FFmpeg controller                    │  │
│  │  Settings                             │  │
│  │  Secure storage                       │  │
│  │  Filter-list manager                  │  │
│  │  Update manager                       │  │
│  │  Logging                              │  │
│  └───────────────────┬───────────────────┘  │
│                      │                      │
│        ┌─────────────┴──────────────┐       │
│        ▼                            ▼       │
│     yt-dlp                        FFmpeg    │
└─────────────────────────────────────────────┘
```

---

# 4. YouTube Integration

## 4.1 Official YouTube

The application should display the official YouTube web experience rather than recreating YouTube through an alternative API.

This is required so that:

- Existing accounts work
- Recommendations work
- Subscriptions work
- Playlists work
- Comments work
- Likes work
- History works
- Notifications work
- YouTube UI updates automatically
- New YouTube features can appear without rebuilding the entire application

The application should not implement its own replacement for YouTube's backend.

---

# 5. WebView

## 5.1 No Browser UI

The application must not expose:

- Address bar
- Browser tabs
- Browser history UI
- Browser bookmarks
- Generic URL navigation UI
- Browser extensions UI

The application should only provide the YouTube experience and Yood's own application UI.

## 5.2 Navigation Restrictions

Navigation should be controlled.

Allowed destinations should include the domains required by:

- YouTube
- Google authentication
- Google account functionality
- YouTube media/services
- Required YouTube/CDN resources

Unexpected external navigation should not silently turn Yood into a browser.

External links may either:

1. Be opened inside the application when required for YouTube functionality, or
2. Open through the user's normal external browser when they intentionally leave the supported YouTube experience.

---

# 6. Google Authentication

Users must be able to log into their existing Google/YouTube accounts.

Requirements:

- Login inside the application
- Persistent session
- Logout
- Multiple Google accounts
- Account switching
- 2FA compatibility
- Google security challenges must remain functional
- Cookies/session information must persist between launches
- Session information must be protected locally

## 6.1 Authentication Compatibility Layer

Because Google may change or restrict authentication behavior inside embedded WebViews, authentication must be isolated from the rest of the application.

The implementation should have:

```text
Authentication Manager
        │
        ├── Embedded authentication
        │
        └── Compatibility fallback
```

The fallback must be implemented in code and must never expose passwords or authentication tokens to the Rust backend unnecessarily.

The application must never request or store a user's Google password itself.

---

# 7. Advertising Removal

Advertising removal is mandatory and cannot be disabled by the user.

The goal is to remove anything that significantly interrupts the YouTube experience, including:

- In-stream advertisements
- Pre-roll advertisements
- Mid-roll advertisements
- Post-roll advertisements
- Display advertisements
- Banner advertisements
- Overlay advertisements
- Sponsored advertising UI where technically possible
- Advertisement placeholders
- Advertisement-related interruptions
- Other known advertising elements

The system should use a layered blocking strategy rather than relying on a single CSS rule.

---

# 8. Ad Blocking Architecture

The recommended implementation is:

```text
YouTube Request
       │
       ▼
Request/Content Filtering
       │
       ├── Blocked request
       │       └── Cancel
       │
       └── Allowed request
               │
               ▼
           WebView
               │
               ▼
        Cosmetic filtering
               │
               ▼
         YouTube UI
```

The implementation should support compatible filter lists where technically possible.

Potential sources include:

- EasyList-compatible rules
- YouTube-specific rules
- Additional privacy/filter rules selected by the project

The application must not hard-code the assumption that one filter list will always work.

---

# 9. Filter List Updates

Filter lists should update automatically every 24 hours.

Requirements:

- Background update
- Local cache
- Atomic replacement
- Validation before activation
- Failed update rollback
- Last-success timestamp
- No unnecessary repeated downloads
- No update when the current list is still valid and the server indicates no change
- Graceful operation while offline

The application must remain usable if filter-list servers are unavailable.

---

# 10. Tracking Protection

Tracking protection is separate from advertisement blocking.

Default:

```text
Ad Blocking: ON / Mandatory
Tracking Protection: OFF
```

The user may enable tracking protection from Settings.

Tracking protection may include:

- Third-party tracking requests
- Known analytics endpoints
- Known tracking domains
- Unnecessary telemetry requests

The setting must not interfere with normal YouTube account functionality unless the user explicitly enables aggressive filtering.

---

# 11. YouTube Download System

The most important Yood-specific feature is integrated downloading.

A Download button should be injected into appropriate YouTube UI locations.

The button should visually resemble YouTube's existing UI and should replace an existing YouTube download button when one is present.

It should appear in:

- Video pages
- Video cards
- Search results
- Recommended videos
- Playlist entries
- Channel video listings
- Watch History
- Watch Later
- Other relevant video contexts

---

# 12. Download Button Behavior

Clicking Download opens a compact Yood download dialog.

Example:

```text
┌─────────────────────────────┐
│ Download                    │
│                             │
│ ○ Video                     │
│   Quality: Best Available ▼ │
│                             │
│ ○ Audio                     │
│   MP3                       │
│                             │
│ ○ Songs                     │
│   Optimized audio download  │
│                             │
│        [ Download ]          │
└─────────────────────────────┘
```

The UI must feel native to YouTube.

---

# 13. Download Modes

## 13.1 Video

Default mode.

The user can select:

- Best available
- Any available resolution
- Available video formats
- MP4
- MKV
- Video only
- Video + audio

The default should be:

```text
Best available video + audio
```

When video and audio are separate streams, Yood must automatically download both and use FFmpeg to mux them.

---

# 14. Audio Mode

Audio mode downloads audio only.

Default output:

```text
MP3
```

The conversion should be handled by FFmpeg.

The application must not simply rename an incompatible audio file to `.mp3`.

---

# 15. Advanced Download Mode

An `Advanced` option should expose additional controls.

Possible controls:

- Exact format selection
- Exact video codec
- Exact audio codec
- Bitrate
- Audio quality
- Container
- FPS
- Resolution
- Video-only
- Audio-only
- Custom yt-dlp format selection
- Metadata handling
- Thumbnail embedding
- Subtitle handling
- Chapter handling
- Filename template

Advanced options should be hidden by default so normal downloading remains simple.

---

# 16. Songs Mode

A dedicated `Songs` option should optimize the download for music.

The intended behavior is:

```text
Songs
   │
   ├── Audio only
   ├── MP3
   ├── Best appropriate audio quality
   ├── Metadata when available
   ├── Artist/channel metadata
   ├── Title metadata
   └── Optional thumbnail embedding
```

The application should automatically choose the most appropriate audio source rather than asking the user to select video quality.

---

# 17. Filename Format

Default filename:

```text
Channel Name - Video Title.ext
```

Examples:

```text
PlayRood - Example Video.mp4
Artist Channel - Example Song.mp3
```

Invalid filesystem characters must be sanitized automatically.

The implementation must handle:

- Windows filename restrictions
- Linux filename compatibility
- Unicode
- Very long titles
- Duplicate filenames
- Existing files
- Special characters

---

# 18. Playlist Downloads

Downloading a playlist should open a playlist-specific download dialog.

Requirements:

- Display all videos
- All videos selected by default
- Individual selection/deselection
- Select all
- Deselect all
- Quality selection
- Video/Audio/Songs modes
- Download queue
- Per-item status
- Retry failed items

Example:

```text
Playlist
──────────────────────────────
☑ Video 01
☑ Video 02
☑ Video 03
☐ Video 04
☑ Video 05

Mode: Video
Quality: Best Available

[ Download 4 Videos ]
```

---

# 19. Channel Downloads

Channel downloading should support downloading videos from a channel using yt-dlp.

The UI should allow:

- Selecting videos
- Selecting all
- Filtering
- Choosing Video/Audio/Songs
- Choosing quality
- Queueing downloads

The application should avoid accidentally downloading an entire channel without an explicit user confirmation.

---

# 20. Download Manager

The application must contain a full download manager.

The visual design should be consistent with YouTube.

Sections:

```text
Downloads

Active
Queued
Completed
Failed
```

Each download should show:

- Thumbnail
- Video title
- Channel
- Progress
- Percentage
- Download speed
- ETA
- Current state
- File size when available

Controls:

- Pause
- Resume
- Cancel
- Retry
- Remove
- Open file
- Open containing folder

---

# 21. Download Queue

Downloads must be queued rather than spawning unlimited yt-dlp processes.

The queue must enforce configurable concurrency.

Default concurrency should be conservative to avoid:

- Excessive CPU usage
- Excessive RAM usage
- Network saturation
- Too many simultaneous FFmpeg processes

The application must provide backpressure.

---

# 22. yt-dlp Integration

yt-dlp must be bundled with the application.

The user should not need to install it manually.

The Rust backend should manage yt-dlp as a child process.

Requirements:

- Version detection
- Version reporting
- Controlled execution
- Structured progress parsing
- Exit-code handling
- Error parsing
- Cancellation
- Timeout handling
- Retry handling

yt-dlp must not be executed through a shell command string constructed from untrusted YouTube data.

Arguments must be passed using a safe process API.

---

# 23. yt-dlp Updates

yt-dlp should support automatic updates.

The application should:

1. Check for a newer compatible version.
2. Download the update.
3. Validate it.
4. Replace the bundled version atomically.
5. Keep a fallback copy.
6. Roll back if the new version fails validation.

The update system must never leave Yood without a working yt-dlp executable because an update failed.

---

# 24. FFmpeg Integration

FFmpeg must also be bundled.

It is required for:

- Video/audio muxing
- MP3 conversion
- Container conversion
- Advanced media operations

The FFmpeg binary must be selected for the appropriate operating system and architecture.

The application must not require users to manually install FFmpeg.

---

# 25. Native Player

Yood should provide two playback modes.

## Mode 1 — YouTube Player

Default.

Uses the official YouTube player through the YouTube WebView.

## Mode 2 — Yood Native Player

Optional.

The native player should provide:

- Play/pause
- Seek
- Volume
- Mute
- Fullscreen
- Picture-in-Picture
- Playback speed
- Quality selection where technically available
- Subtitle selection
- Subtitle loading
- Keyboard shortcuts
- Skip controls
- Timeline
- Chapters
- Audio track selection
- Video track selection
- Playback statistics

The native player should be designed to be more powerful than the standard YouTube player.

---

# 26. Native Player Architecture

The native player should not be loaded into the main YouTube WebView.

It should be a separate Yood component.

This prevents heavy player state from unnecessarily increasing the memory footprint of the main application.

The implementation should use a lightweight native media backend appropriate to the selected Rust ecosystem and target platforms.

---

# 27. YouTube Feature Compatibility

The application should preserve the normal YouTube experience, including:

- Home
- Search
- Shorts
- Subscriptions
- History
- Watch Later
- Playlists
- Channels
- Comments
- Likes
- Dislikes where available
- Notifications
- Recommendations
- Membership UI where available
- Live streams
- Premieres
- Captions
- Chapters
- Playback speed
- Quality controls
- Account settings
- YouTube settings
- Age/region-specific behavior
- Multiple accounts

The application must avoid recreating these systems unless necessary for Yood-specific functionality.

---

# 28. Deep Links

The operating system should be able to associate supported YouTube URLs with Yood.

Examples:

```text
https://www.youtube.com/watch?v=...
https://youtu.be/...
https://www.youtube.com/playlist?list=...
https://www.youtube.com/channel/...
```

When configured as the default handler, opening a supported YouTube URL should launch Yood and navigate directly to the corresponding content.

---

# 29. Settings

Settings should use a native-looking Yood settings interface.

Suggested categories:

```text
General
Appearance
Playback
Downloads
Advanced Downloads
Privacy
Ad Blocking
Tracking Protection
Accounts
Keyboard Shortcuts
Storage
Updates
About
```

---

# 30. Appearance

The application should support:

- Light
- Dark
- System

The YouTube interface should remain visually consistent with the selected mode.

---

# 31. Storage Settings

The user should be able to choose:

- Default download folder
- Temporary download folder
- Maximum concurrent downloads
- Temporary-file cleanup behavior
- Completed-download handling

The application must clearly distinguish incomplete temporary files from completed downloads.

---

# 32. Security

Security is a major requirement.

The application must:

- Avoid shell injection
- Avoid command injection
- Validate all external process arguments
- Sanitize filenames
- Validate downloaded paths
- Prevent directory traversal
- Avoid arbitrary executable execution
- Validate bundled binaries
- Protect session data
- Avoid logging credentials
- Avoid logging authentication cookies
- Avoid storing Google passwords
- Restrict WebView navigation
- Restrict application IPC commands
- Validate all frontend-to-Rust commands

---

# 33. Local Data Encryption

Sensitive local information should be protected using OS-appropriate secure storage.

Examples:

- Windows Credential Manager / DPAPI-compatible mechanisms
- Linux Secret Service / keyring where available

The application should not simply place authentication cookies in plaintext JSON files.

Non-sensitive preferences may remain in normal configuration storage.

---

# 34. Privacy

Yood must not collect user information.

No:

- Telemetry
- Analytics
- Advertising IDs
- Usage tracking
- User profiling
- Remote crash reporting
- Central user database
- Yood account
- Yood authentication server

The application should communicate directly with the services required for YouTube functionality and with the servers required for filter-list/yt-dlp maintenance.

---

# 35. Logging

Logging must be local.

Log levels:

```text
ERROR
WARN
INFO
DEBUG
TRACE
```

Default:

```text
INFO
```

Logs must never contain:

- Passwords
- OAuth secrets
- Cookies
- Authentication tokens
- Private session data

Debug logging must also sanitize potentially sensitive URLs and headers.

---

# 36. Memory Management

Low memory usage is a first-class requirement.

The application must avoid:

- Memory leaks
- Unbounded caches
- Unbounded download history
- Unlimited WebView-created objects
- Unbounded event listeners
- Repeated DOM injection
- Duplicate MutationObservers
- Orphaned timers
- Unreleased subprocess handles

The download manager must remove finished process objects and temporary resources.

---

# 37. CPU Usage

Idle CPU usage must be extremely low.

Background tasks should be event-driven rather than continuously polling.

Avoid:

```text
while(true) {
    checkSomething();
}
```

Prefer:

- Timers with appropriate intervals
- Events
- WebView events
- OS notifications where available
- Async tasks

Filter-list updates should occur once per 24 hours rather than through continuous polling.

---

# 38. WebView Resource Management

The application must avoid unnecessarily creating multiple WebViews.

Default architecture should use one primary YouTube WebView.

Additional WebViews should only be created when technically required.

Any temporary WebView must have an explicit lifecycle and guaranteed cleanup.

---

# 39. JavaScript Injection

Yood-specific YouTube functionality should be implemented through a controlled integration layer.

Responsibilities:

- Locate video elements
- Locate playlist elements
- Detect existing YouTube download controls
- Inject Yood download controls
- Detect navigation changes
- Detect dynamically loaded content
- Communicate with Rust
- Update UI when YouTube changes

The implementation must prevent duplicate buttons and duplicate event listeners.

A navigation/event-driven approach is preferred over aggressive DOM polling.

---

# 40. YouTube UI Changes

YouTube changes its frontend frequently.

Therefore, the integration layer must be resilient.

Do not depend exclusively on one fragile CSS selector.

Use multiple detection strategies where possible:

- Semantic attributes
- Stable DOM structures
- YouTube custom elements
- Known identifiers
- URL/context detection
- MutationObserver used carefully

Integration failures should degrade gracefully.

A YouTube UI change must not crash the entire application.

---

# 41. Error Handling

All major operations must have explicit error states.

Examples:

```text
YouTube unavailable
Network unavailable
Google login failed
Download failed
yt-dlp unavailable
FFmpeg unavailable
Format unavailable
Permission denied
Disk full
Invalid destination
Filter list unavailable
Native player unavailable
```

Errors should be presented in user-friendly language.

Raw technical errors should be available through an expandable diagnostic section.

---

# 42. Offline Behavior

If the internet is unavailable:

- The application should launch
- Settings should remain accessible
- Download history should remain accessible
- Existing local files should remain accessible
- The WebView should display an appropriate offline state
- Background updates should wait until connectivity returns

The application must not repeatedly retry failed network requests indefinitely.

---

# 43. Installation

A setup system must install everything required.

The user should not need to manually install:

- Rust
- Node.js
- Tauri
- yt-dlp
- FFmpeg
- Python
- WebView2 when it can be provisioned appropriately
- Other development dependencies

The development environment and end-user installation must be clearly separated.

---

# 44. Windows Installer

The Windows installer should:

- Install Yood
- Install/register required components
- Bundle yt-dlp
- Bundle FFmpeg
- Create Start Menu shortcut
- Optionally create Desktop shortcut
- Register supported YouTube URL protocols/file associations as appropriate
- Support clean uninstall

The installer must not require administrator privileges unless genuinely necessary.

---

# 45. Linux Installation

The project should provide a simple installation mechanism appropriate for private family use.

Preferred options should be evaluated based on:

- Dependency handling
- Size
- Startup performance
- WebKitGTK availability
- Ease of installation
- Ease of removal

An AppImage may be provided for portability, while a native package can be provided when dependency integration makes it preferable.

The final implementation should choose the smallest practical distribution method.

---

# 46. Startup

Startup should be optimized.

Do not initialize:

- Download manager workers
- Unnecessary FFmpeg processes
- yt-dlp
- Filter-list update downloads
- Large caches

until required.

The application should display its UI quickly and initialize secondary components asynchronously.

---

# 47. Resource Budget

The implementation should establish measurable performance targets.

Initial targets:

- No continuous high CPU usage while idle
- No continuously growing RAM usage during normal browsing
- No runaway WebView memory growth caused by Yood itself
- Download manager must release resources after completion
- No orphaned yt-dlp/FFmpeg processes
- No persistent background process when Yood is fully closed

Exact RAM targets should be measured separately on Windows and Linux because the underlying WebView implementations differ.

---

# 48. Process Lifecycle

Expected processes:

```text
Yood
 ├── WebView
 ├── yt-dlp (only while needed)
 └── FFmpeg (only while needed)
```

yt-dlp and FFmpeg should not remain running after their work is finished.

When Yood exits:

1. Stop active downloads according to configured behavior.
2. Terminate child processes safely.
3. Flush local state.
4. Close secure-storage handles.
5. Release WebView resources.
6. Exit without leaving orphan processes.

---

# 49. Download Resume

Downloads should support resuming where yt-dlp supports it.

Interrupted downloads should not automatically be treated as completed.

The manager must distinguish:

```text
Queued
Downloading
Paused
Interrupted
Completed
Failed
Cancelled
```

---

# 50. Duplicate Downloads

Before downloading a file, Yood should detect whether the target already exists.

Possible choices:

```text
File already exists

[ Replace ]
[ Keep Both ]
[ Cancel ]
```

For playlists, duplicate handling should be configurable.

---

# 51. Disk Space

Before starting large downloads where size information is available, the application should check available disk space.

If insufficient space is detected:

- Do not start the download
- Show a clear error
- Allow the user to change the destination

The download manager should monitor disk errors during downloads.

---

# 52. Subtitles

The application should support subtitles where technically available.

For downloads, Advanced mode may expose:

- Download subtitles
- Embed subtitles
- Subtitle language
- Auto-generated subtitles where available

The normal download UI should remain simple.

---

# 53. Chapters and Metadata

Advanced download settings may support:

- Chapters
- Metadata
- Thumbnails
- Artist
- Title
- Album
- Description
- Upload date

Songs mode should automatically use the most appropriate metadata available.

---

# 54. Keyboard Shortcuts

The application should support YouTube-like shortcuts and Yood-specific shortcuts.

Examples:

```text
Space       Play/Pause
K           Play/Pause
F           Fullscreen
M           Mute
J           Back
L           Forward
←/→         Seek
↑/↓         Volume
Shift+D     Download
```

Shortcuts should be configurable.

---

# 55. Accessibility

The application should preserve accessibility support provided by the underlying YouTube interface.

Yood-specific UI must support:

- Keyboard navigation
- Screen readers where supported
- Focus management
- Accessible labels
- Sufficient visual contrast
- Reduced-motion preference where practical

---

# 56. Configuration

Configuration should be versioned.

Example conceptual structure:

```text
config
├── appearance
├── playback
├── downloads
├── privacy
├── filtering
├── shortcuts
└── advanced
```

Configuration migrations must be supported when the schema changes.

---

# 57. Application Updates

Yood itself does not require an automatic update system at the current stage.

The project is intended for private/family use.

However, the architecture should not make future updates impossible.

The application should still expose version information in About.

---

# 58. Third-Party Component Updates

The application must independently manage:

- yt-dlp
- Filter lists
- FFmpeg when security/compatibility updates are required

These update mechanisms must be independent from Yood application updates.

---

# 59. Compliance and Compatibility

The implementation must not attempt to bypass DRM, paid access controls, account security, or authentication mechanisms.

Downloading should rely on yt-dlp's supported functionality and respect applicable rights and service restrictions.

The application should not attempt to defeat technical protections that are specifically intended to prevent access.

---

# 60. Project Structure

The final implementation should use a modular structure similar to:

```text
yood/
├── src-tauri/
│   ├── commands/
│   ├── download/
│   ├── player/
│   ├── auth/
│   ├── filtering/
│   ├── settings/
│   ├── security/
│   ├── storage/
│   ├── updates/
│   ├── process/
│   └── main.rs
│
├── frontend/
│   ├── youtube/
│   ├── downloads/
│   ├── player/
│   ├── settings/
│   ├── components/
│   ├── styles/
│   └── integration/
│
├── binaries/
│   ├── windows/
│   └── linux/
│
├── filters/
├── installer/
├── tests/
└── documentation/
```

The exact structure may change during implementation, but responsibilities should remain separated.

---

# 61. Testing Strategy

Testing must be comprehensive.

## Unit Tests

Test:

- Filename sanitization
- Path validation
- Configuration
- Queue logic
- Download state machine
- yt-dlp argument construction
- FFmpeg argument construction
- Filter-list parsing
- Settings migration
- Error conversion
- Secure-storage abstractions

## Integration Tests

Test:

- Rust ↔ frontend IPC
- Rust ↔ yt-dlp
- Rust ↔ FFmpeg
- Download lifecycle
- Cancellation
- Resume
- Retry
- Playlist handling
- Filter updates
- Configuration persistence

## UI Tests

Test:

- Download button injection
- Duplicate button prevention
- Dynamic YouTube navigation
- Playlist UI
- Download dialog
- Download manager
- Settings
- Player switching

---

# 62. Security Testing

Perform dedicated testing for:

- Command injection
- Argument injection
- Path traversal
- Malicious filenames
- Malicious playlist titles
- Malicious channel names
- Crafted YouTube metadata
- IPC authorization
- WebView navigation
- Cookie exposure
- Credential leakage
- Log leakage
- Temporary-file exposure
- Symlink attacks
- Executable replacement
- Malicious bundled binary replacement

---

# 63. Memory Leak Testing

Memory testing is mandatory.

Test scenarios should include:

1. Launch Yood.
2. Browse YouTube for several hours.
3. Open many videos.
4. Open playlists.
5. Search repeatedly.
6. Switch accounts.
7. Open/close settings.
8. Start/cancel downloads.
9. Download playlists.
10. Switch between YouTube and native player.
11. Repeat navigation cycles.

Measure:

- RSS
- Private memory
- CPU
- Process count
- Child processes
- Handle count where practical

Memory must not grow continuously because of Yood-owned resources.

WebView memory behavior must be distinguished from actual Yood memory leaks.

---

# 64. Long-Running Stability Test

A dedicated soak test should run for at least 24 hours.

It should simulate:

- Navigation
- Search
- Playback
- Playlist navigation
- Downloads
- Pauses
- Cancellations
- Network interruptions
- Reconnection
- Filter updates

The application must remain responsive and must not accumulate unbounded resources.

---

# 65. Network Failure Testing

Simulate:

- No internet
- DNS failure
- YouTube unavailable
- Google unavailable
- yt-dlp source unavailable
- Filter-list server unavailable
- Connection reset
- Slow network
- Network interruption during download

Every scenario must produce controlled behavior.

---

# 66. Download Testing Matrix

Test at minimum:

```text
Single video
Playlist
Channel
Short
Live stream where supported
Music video
Audio
Best quality
Lowest quality
1080p
720p
Other available resolutions
Video only
Video + audio
MP3
MP4
MKV
Advanced format
Existing file
Insufficient disk
Cancelled download
Interrupted download
Failed download
Multiple simultaneous downloads
```

Only formats and capabilities actually offered by the source should be displayed.

---

# 67. Cross-Platform Testing

Every major feature must be tested on:

## Windows

- Supported Windows versions
- WebView2
- Installer
- File associations
- Secure storage
- Download paths
- Native player

## Linux

At least the primary supported distribution/environment should be tested.

Test:

- WebKitGTK
- Package/AppImage installation
- Secure storage availability
- Download paths
- Native player
- System integration

---

# 68. Acceptance Criteria

The project is considered complete only when:

- YouTube works normally.
- Google login works.
- Persistent sessions work.
- Multiple accounts work.
- YouTube navigation works.
- YouTube recommendations work.
- YouTube playlists work.
- Comments work.
- History works.
- Notifications work.
- Advertising is consistently blocked to the extent technically possible.
- Ad blocking cannot be disabled.
- Tracking protection can be independently enabled/disabled.
- Download buttons appear throughout relevant YouTube UI.
- Existing YouTube download UI is replaced where appropriate.
- Video downloads work.
- Audio downloads work.
- MP3 downloads work.
- MP4 downloads work.
- MKV downloads work.
- Best-quality selection works.
- Available-quality discovery works.
- Songs mode works.
- Playlist downloads work.
- Channel downloads work.
- Download queue works.
- Pause/resume/cancel works.
- Retry works.
- yt-dlp is bundled.
- FFmpeg is bundled.
- yt-dlp can update safely.
- Filter lists update every 24 hours.
- Native player works.
- YouTube player works.
- Deep links work.
- No telemetry exists.
- Sensitive session information is protected.
- No credentials are logged.
- No persistent yt-dlp/FFmpeg processes remain after completion.
- No critical memory leaks are detected.
- No critical security vulnerabilities remain.
- Windows installation works.
- Linux installation works.
- The application remains responsive during normal downloads.
- The application does not behave like a general-purpose browser.

---

# 69. Development Priorities

Implementation should proceed in this order:

## Phase 1 — Foundation

- Rust/Tauri project
- Windows build
- Linux build
- WebView
- Basic YouTube loading
- Application lifecycle
- Settings infrastructure

## Phase 2 — YouTube Integration

- Navigation handling
- Google authentication
- Session persistence
- YouTube UI integration
- Deep links

## Phase 3 — Ad Blocking

- Request filtering
- Cosmetic filtering
- Filter-list management
- 24-hour updates
- Tracking protection

## Phase 4 — Downloads

- yt-dlp bundling
- FFmpeg bundling
- Download dialog
- Quality detection
- Video downloads
- Audio/MP3 downloads
- Songs mode

## Phase 5 — Download Manager

- Queue
- Progress
- Pause
- Resume
- Cancel
- Retry
- Playlist downloads
- Channel downloads

## Phase 6 — Native Player

- Playback engine
- Fullscreen
- Subtitles
- Chapters
- Quality
- Speed
- Keyboard controls
- Picture-in-Picture

## Phase 7 — Optimization

- RAM profiling
- CPU profiling
- WebView optimization
- Process lifecycle
- Leak testing
- 24-hour soak testing

## Phase 8 — Packaging

- Windows installer
- Linux distribution
- Bundled binaries
- File/URL associations
- Uninstaller
- Final QA

---

# 70. Engineering Principles

The implementation must follow these principles:

1. Prefer simple solutions.
2. Do not introduce a large framework when a small component is sufficient.
3. Avoid unnecessary background processes.
4. Avoid polling where events are available.
5. Avoid global mutable state.
6. Keep Rust modules small and responsibility-focused.
7. Keep frontend components modular.
8. Never trust YouTube metadata.
9. Never construct unsafe shell commands.
10. Never log credentials.
11. Never store passwords.
12. Do not create unnecessary WebViews.
13. Do not keep yt-dlp or FFmpeg alive unnecessarily.
14. Clean up every asynchronous task.
15. Clean up every event listener.
16. Clean up every timer.
17. Bound every cache and queue.
18. Measure memory instead of assuming it is low.
19. Measure CPU instead of assuming it is low.
20. Test long-running behavior.
21. Fail safely.
22. Prefer graceful degradation over crashes.
23. Do not make Yood into a general browser.
24. Keep the normal user workflow extremely simple.

---

# 71. Final Product Vision

The final user experience should be approximately:

```text
Launch Yood
     │
     ▼
YouTube opens immediately
     │
     ├── No ads
     ├── Normal Google account
     ├── Normal subscriptions
     ├── Normal recommendations
     ├── Normal playlists
     └── Normal YouTube experience
             │
             ▼
       [ Download ]
             │
             ▼
     ┌─────────────────┐
     │ Video           │
     │ Audio           │
     │ Songs           │
     │ Advanced        │
     └─────────────────┘
             │
             ▼
       Download Manager
             │
             ├── Queue
             ├── Progress
             ├── Pause
             ├── Resume
             └── Completed
```

Yood should feel like a lightweight, purpose-built YouTube desktop application rather than a browser with YouTube opened inside it.

The most important engineering goal is not maximum feature count. It is maintaining the full YouTube experience while keeping Yood lightweight, stable, secure, and free from unnecessary resource consumption.
