---
version: 1
slug: "desktop-ui-rs"
primary_target: "desktop/ui.rs"
related_targets: ["desktop/main.rs", "desktop/session.rs", "desktop/connection.rs", "android/app/src/main/java/dev/opencam/MainActivity.java", "android/app/src/main/res/layout/settings_sheet.xml"]
---

# Connection flow polish

Mode: Operate. Refine the existing desktop Zeron and Android Material 3 worlds, preserving camera capabilities, authenticated transport, direct native rendering and the intrinsic mobile layout.

THESIS: Make the next connection action and recovery path clear without changing the camera studio's visual identity.
OWN-WORLD: Preserve desktop Geist, neutral surfaces, lavender selection, compact rails and floating palettes. Android retains semantic Material type/color roles and native sheets. Reuse spacing, control and minimum-target tokens.
STORY: Open the phone app, select a discovered phone or paste its complete link, enter a saved password when required, choose Wi-Fi or USB, and see connecting, canceled, interrupted or recovered state clearly. Camera errors remain visible independently of pairing guidance.
FIRST VIEWPORT: A native camera-centered desktop with a labeled Connection panel and meaningful empty preview; Android keeps readiness, contained preview and reachable labeled controls. Connection errors have recovery guidance and an action.
FORM: Code-led refinement of user-pinned native conventions. No new comp or concept seed; fixed desktop palette geometry is incumbent, while mobile content remains measured. Mounted and handheld phone use remain equally important in portrait and landscape.
FINISH: Real native captures and functional checks, independent finish verdict and documentation reconciliation before a signed release and per-user PC installation.

## Verification limits

Desktop UI is captured from the actual Linux compositor; Xvfb could not present the native renderer. Android uses API 36 emulator phone/tablet captures, including landscape, large text and keyboard states. Protocol fixtures are explicitly synthetic. Windows and macOS receive hosted native build/test/package checks; physical handset, camera-driver and TalkBack comfort require hardware.
