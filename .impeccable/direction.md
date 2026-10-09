# OpenCam desktop and companion

Mode: Operate. User-pinned redesign reference: https://github.com/zeronsh/zeron,
checkout 0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9. The user confirmed Zeron's actual
Zui fork of GPUI and its gpui-component toolkit. This supersedes the OBS-like
bottom-dock layout. Prior requests for concise control-only copy and SVG actions
remain in force. Code-led implementation, no generated comp or artwork.

Latest user correction supersedes the permanent control sidebar and every clean-window/OBS capture affordance. The source is direct video frames. The desktop live preview starts after pairing and continuously reflects camera controls.

First viewport: Zeron's 38px compact chrome, full-height contained live preview, a narrow vertical action rail and a separate vertical tool rail. A selected tool opens one intrinsic floating palette over video; close it for full preview. Connection/lenses, format, exposure, focus/zoom, color, monitoring and camera output are separate tools. No always-visible settings wall or fixed bottom dock. HUD shows actual sensor ISO/shutter when available, zoom and a live torch toggle. The preview's monitor aids are excluded from camera output.

Resolution and FPS are actual gpui-base Combobox + Input compositions with both native typing and dropdown suggestions. Sensor mode validation stays separate from arbitrary output dimensions; custom even non-16:9 output is supported. Enter/check applies. Professional exposure includes shutter-angle presets. Blackmagic's control/HUD/monitoring conventions inform operation, with no unsupported hardware claims.

Reuse: Zeron's dark neutral #060606/#0d0d0d surfaces, white washes, compact 6px
control radii, 12px floating surfaces, Geist and Geist Mono, indigo selection/focus.
The actual single-scene-layer frost code from crates/ui/src/frost.rs blurs only
floating option menus; the live preview has no blur. Surface/control code is
adapted from surface_chrome.rs and shell.rs, with MIT attribution in vendor/zeron.
Zui and gpui-base are pinned to the same commits as the reference application.
No wallpaper is painted over camera content. Signature interaction: native
frosted selection menu over live video; no ornamental animation.

Required states: disconnected, connecting/error, live video, codec options,
keyboard focus, unavailable camera controls, benchmark progress/results. Tooltips
and accessibility labels name every SVG action. Preview contains rather than
crops the stream. Wide (1280x800 logical) and compact (800x560 logical) Linux
captures plus native Android phone capture form the verification matrix.

Native Android Views companion: the same dark palette and Geist family through
Material 3 controls, with a contained full-screen local preview, live resolution/FPS
status, a right-side tool rail, and one scrollable bottom palette for the selected
tool. Camera/focus, format, sensor, phone effects, and connection settings open from
separate tools. The connection palette retains Enable/Disable, the selectable and
copyable pairing URI, live status, and optional password protection. Camera preview
can run without the stream encoder; the encoder runs only during desktop streaming.
When phone processing is selected, the same GLES-processed frames feed the local
preview and encoder. Desktop-side effects do not return video to the phone; changing
phone effects switches processing to the phone. Phone and desktop controls sync in
both directions, and desktop acknowledgements update phone controls. The app stops
camera access when backgrounded. Nothing phone, Windows/macOS visuals, and physical
lens access still require hardware not present here.

FORM provenance: explicit user override of the concept roll: direct Zeron reference and toolkit confirmation. No
concept roll or seed was used. The reference source and screenshot files are the
visual authority; generated concept selection would contradict that instruction.

Later confirmed requirements: native Android NSD and desktop mDNS discovery,
optional password protection with a Material3 switch and persistently labeled masked editor, editable resolution/FPS comboboxes, non-16:9 output presets and Fit/Crop. Preserve all advertised sensor modes.
Use actual Zeron SVGs where applicable and unchanged Solar Linear assets from 480 Design for every camera-specific glyph. The password field must have a visible control label. The one-screen phone companion omits a redundant app-name toolbar under the user’s explicit control-only/no-unnecessary-title brief. Zeron’s pinned showcase is the quality bar, not a separately chosen catalog card.
Verification additionally covers protected Android pairing and discovered service
visibility. Emulator has no hardware video encoders; preview evidence uses the
explicitly labeled synthetic fixture, not a claimed Nothing phone stream.

The user additionally requested fastest lightweight phone-side processing, stretch, aspect manipulation, fisheye/local magnification and ML background blur. Extend the same vertical rail with geometry, blur and sensor-ISP tools. Each opens a compact palette using the existing native sliders/dropdowns and Solar glyphs. Effects must be visible in transmitted preview/source frames; monitor aids remain preview-only. Processing status reports actual phone GPU/ML measurements, never fixture-derived hardware claims.

The latest request adds heavy desktop effects. Geometry and blur palettes share a Process on Phone/Desktop selector. Desktop geometry exposes Auto benchmark/GPU/CPU plus an actual adapter selector. Desktop blur exposes available ONNX Runtime provider choices and fallback errors. Preserve the same intrinsic native control layout, vertical rails and user-pinned Zeron world; this is a processing extension, not another visual redesign.

The current transport extension adds H.264/HEVC versus uncompressed YUV420/RGBA in the existing format control, with source-size/FPS choices tied to that mode. Bitrate is shown only for compressed formats. RAW-capable lenses expose RAW size selection and a DNG capture action; RAW remains still capture. Connection recovery uses the existing status line and preserves these native surfaces. No extra marketing copy, panels or visual world change is authorized. Native phone portrait/landscape/large-text plus wide/compact Linux and format menus are rechecked after this extension.
