OpenCam adapts MIT-licensed Zeron code at commit
0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9:
https://github.com/zeronsh/zeron

- crates/ui/src/frost.rs → desktop/frost.rs (application theme dependency removed).
- crates/ui/src/surface_chrome.rs and shell.rs header_icon_button → desktop/ui.rs
  (sizes, radii, washes and control composition adapted for camera controls).
- crates/ui/src/theme.rs dark palette and typography.rs Geist families → desktop UI.
- Bundled Geist fonts retain their SIL OFL license in desktop/fonts/OFL.txt.

Zui and gpui-base are pinned to the same revisions as that Zeron checkout.
Their upstream licenses remain with the Cargo dependencies.

SVG assets copied unchanged from Zeron: action-play, stop, monitor, sun, refresh,
gallery, plus, alt-arrow-down, alt-arrow-up and check (renamed locally).
Solar Icons by 480 Design (Linear), CC BY 4.0:
https://creativecommons.org/licenses/by/4.0/
Zeron-authored glyphs retain Zeron's MIT license. Camera-specific additions are unchanged Solar Linear assets, with provenance in vendor/solar/NOTICE.md.
