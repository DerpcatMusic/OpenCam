---
version: 1
slug: "android-app-src-main-res-layout-main-xml"
primary_target: "android/app/src/main/res/layout/main.xml"
related_targets: ["android/app/src/main/java/dev/opencam/MainActivity.java","android/app/src/main/java/dev/opencam/CameraControls.java"]
---

# Android camera surface

Mode: Operate. Scope: Android layouts, controls, theme resources and UI verification. Preserve camera capabilities, TLS, discovery, transport, desktop behavior and the preview transform.

## Direction contract

THESIS: A phone camera companion that makes preview and desktop readiness clear, with labeled controls within reach and advanced settings grouped by task.

OWN-WORLD: Native Material 3 Views and Material color, type, shape and motion roles. Preserve Geist and a lavender fallback; support system light/dark appearance and dynamic color. Semantic spacing resources and minimum touch targets govern geometry.

STORY: Grant camera access, frame the shot, connect from the desktop, then adjust Camera, Video, Exposure or Effects. Connection settings explain availability and password drafts.

FIRST VIEWPORT: A content-measured readiness header, a flexible contained preview and wrapping labeled camera tools. Compact windows place tools below the preview; wide windows place a scrollable tool column beside it. No rail offsets or manually assigned panel heights.

FORM: User-pinned native Material convention, implemented in code; no concept seed or image comp substitutes for real Android components. Native sheets provide titled settings, dismissal, scrolling and keyboard handling. The user confirmed mounted and handheld use are equally important; support both portrait and landscape, with legible mounted readiness and reachable handheld adjustments.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Verification

Actual emulator captures: compact portrait, landscape/wide, small phone with 200% text, password keyboard/error state, dark/light and tablet. Two batched visual inspection rounds maximum before the independent finish review. Source critique uses two isolated assessments; HTML/CSS detector has no Android verdict. Native camera/processing/connection tests remain required.
