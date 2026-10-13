---
name: OpenCam
description: 'A direct live camera instrument: pinned Zeron desktop controls and adaptive native Material 3 Android Views.'
colors:
  primary: '#a5a0ff'
  neutral-bg: '#060606'
  neutral-rail: '#0d0d0d'
  neutral-panel: '#111111f5'
  neutral-variant: '#161616'
  neutral-menu: '#161616eb'
  neutral-divider: '#252525'
  neutral-outline: '#626262'
  neutral-text: '#e8e8e8'
  neutral-muted: '#a8a8a8'
  desktop-error: '#f0a0a0'
  neutral-slider-track: '#4b4d55'
  neutral-slider-thumb: '#cdced4'
  control-field-wash: rgba(255,255,255,0.055)
  control-rest-wash: rgba(255,255,255,0.035)
  control-selected-wash: rgba(255,255,255,0.11)
  android-light-primary: '#625B9B'
  android-light-on-primary: '#FFFFFF'
  android-light-primary-container: '#E7DEFF'
  android-light-on-primary-container: '#1E164B'
  android-light-secondary: '#625D71'
  android-light-on-secondary: '#FFFFFF'
  android-light-secondary-container: '#E8DEF8'
  android-light-on-secondary-container: '#1F192C'
  android-light-surface: '#FDF8FF'
  android-light-on-surface: '#1C1B20'
  android-light-surface-container-low: '#F7F2FA'
  android-light-surface-container: '#F1ECF4'
  android-light-surface-container-high: '#EBE6EE'
  android-light-surface-container-highest: '#E6E0E9'
  android-light-on-surface-variant: '#48454E'
  android-light-outline: '#79757F'
  android-light-outline-variant: '#CAC4CF'
  android-light-error: '#BA1A1A'
  android-light-on-error: '#FFFFFF'
  android-light-error-container: '#FFDAD6'
  android-light-on-error-container: '#410002'
  android-light-inverse-surface: '#313035'
  android-light-inverse-on-surface: '#F4EFF7'
  android-light-inverse-primary: '#CBC0FF'
  android-dark-primary: '#CBC0FF'
  android-dark-on-primary: '#332B61'
  android-dark-primary-container: '#4A4278'
  android-dark-on-primary-container: '#E7DEFF'
  android-dark-secondary: '#CBC2DB'
  android-dark-on-secondary: '#332D41'
  android-dark-secondary-container: '#4A4358'
  android-dark-on-secondary-container: '#E8DEF8'
  android-dark-surface: '#141318'
  android-dark-on-surface: '#E6E0E9'
  android-dark-surface-container-low: '#1C1B20'
  android-dark-surface-container: '#201F24'
  android-dark-surface-container-high: '#2B292F'
  android-dark-surface-container-highest: '#36343A'
  android-dark-on-surface-variant: '#CAC4CF'
  android-dark-outline: '#938F99'
  android-dark-outline-variant: '#48454E'
  android-dark-error: '#FFB4AB'
  android-dark-on-error: '#690005'
  android-dark-error-container: '#93000A'
  android-dark-on-error-container: '#FFDAD6'
  android-dark-inverse-surface: '#E6E0E9'
  android-dark-inverse-on-surface: '#313035'
  android-dark-inverse-primary: '#625B9B'
  android-camera-preview-background: '#000000'
typography:
  body:
    fontFamily: Geist, sans-serif
    fontSize: 12px
  telemetry:
    fontFamily: Geist Mono, monospace
    fontSize: 11px
  android-title-large:
    fontSize: 22sp
    lineHeight: 28sp
  android-title-medium:
    fontSize: 16sp
    lineHeight: 24sp
  android-body-medium:
    fontSize: 14sp
    lineHeight: 20sp
  android-label-large:
    fontSize: 14sp
    lineHeight: 20sp
  android-label-medium:
    fontSize: 12sp
    lineHeight: 16sp
rounded:
  sm: 6px
  md: 12px
spacing:
  desktop-rail: 44px
  desktop-icon-button: 32px
  desktop-tool-palette: 264px
  android-content-gutter: 16dp
  android-min-control-target: 48dp
  android-space-micro: 4dp
  android-space-compact: 8dp
  android-space-control: 12dp
  android-space-section: 24dp
  android-icon-action: 24dp
components:
  desktop-edit-field:
    backgroundColor: '{colors.control-field-wash}'
    textColor: '{colors.neutral-text}'
    rounded: '{rounded.sm}'
  desktop-rail-button:
    backgroundColor: '{colors.control-rest-wash}'
    textColor: '{colors.neutral-text}'
    rounded: '{rounded.sm}'
    width: '{spacing.desktop-icon-button}'
    height: '{spacing.desktop-icon-button}'
  desktop-rail-button-selected:
    backgroundColor: '{colors.control-selected-wash}'
    textColor: '{colors.primary}'
    rounded: '{rounded.sm}'
    width: '{spacing.desktop-icon-button}'
    height: '{spacing.desktop-icon-button}'
  desktop-navigation-rail:
    backgroundColor: '{colors.neutral-rail}'
    rounded: '{rounded.md}'
    width: '{spacing.desktop-rail}'
  desktop-tool-palette:
    backgroundColor: '{colors.neutral-panel}'
    textColor: '{colors.neutral-text}'
    rounded: '{rounded.md}'
    width: '{spacing.desktop-tool-palette}'
  android-primary-button:
    backgroundColor: '{colors.android-light-primary}'
    textColor: '{colors.android-light-on-primary}'
  android-password-field:
    textColor: '{colors.android-light-on-surface}'
  android-tonal-action:
    backgroundColor: '{colors.android-light-secondary-container}'
    textColor: '{colors.android-light-on-secondary-container}'
    typography: '{typography.android-label-large}'
  android-tool:
    backgroundColor: '{colors.android-light-surface-container-high}'
    textColor: '{colors.android-light-on-surface}'
    typography: '{typography.android-label-large}'
  android-tool-selected:
    backgroundColor: '{colors.android-light-primary-container}'
    textColor: '{colors.android-light-on-primary-container}'
    typography: '{typography.android-label-large}'
  android-settings-sheet:
    backgroundColor: '{colors.android-light-surface-container-low}'
    textColor: '{colors.android-light-on-surface}'
  android-advanced-disclosure:
    textColor: '{colors.android-light-primary}'
    typography: '{typography.android-label-large}'
---

# Design System: OpenCam

## Overview

**Creative North Star: "The Live Camera Instrument"**

OpenCam treats the live camera image as the main work surface. The desktop uses Zui and gpui-base in the pinned Zeron visual language: near-black planes, compact labeled controls, separate vertical action and tool rails, and one floating palette for the selected tool. Camera content stays clear while supported floating menus receive a restrained frost treatment.

The Android companion expresses the same direct camera purpose through native Material 3 Views. A measured readiness summary, flexible contained preview and labeled camera tools adapt to compact and wide windows. Native settings sheets group controls by task; system light/dark appearance, available dynamic color, text scaling and scrolling govern the phone presentation. The Android theme declares bundled Geist; semantic text appearances and control shapes inherit Material 3 rather than desktop density.

**Key Characteristics:**
- Direct preview first, controls kept to the edges.
- Pinned near-black Zeron desktop planes with one lavender selection accent.
- Adaptive native Material 3 Android controls, semantic color roles and task-grouped sheets.
- Intrinsic phone measurement, wrapping labels and scrolling for larger text.

## Colors

The desktop retains its near-black and lavender palette. Android uses semantic Material color roles with a lavender fallback in two appearances; dynamic color can replace those fallback values.

### Primary
- **Soft lavender:** Desktop selection and focus retain the existing `primary` token.
- **Android lavender action pair:** `android-light-primary` and `android-dark-primary`, with their corresponding on-primary roles, provide contained actions in fallback themes.
- **Android selected-tool pair:** Primary-container and on-primary-container roles distinguish the open camera tool without coloring the camera image.

### Secondary
- **Android muted lavender tonal pair:** Secondary-container and on-secondary-container roles carry the Connection and other tonal actions. Secondary and on-secondary roles remain the Material theme's supporting pair.

### Neutral
- **Night canvas, raised rail and floating panel:** Existing desktop background, rail and tool-palette tokens stay unchanged.
- **Desktop menu base and glass:** `neutral-variant` and `neutral-menu` support option menus; they no longer define Android surfaces.
- **Desktop ink and washes:** Existing text, muted text, divider, outline, slider, field, resting and selected tokens remain the desktop vocabulary.
- **Android surface ladder:** Surface, low, ordinary, high and highest container roles support canvas, native sheets, tonal controls and outlined fields. Each light/dark role is recorded separately in the frontmatter.
- **Android ink and outlines:** On-surface, on-surface-variant, outline and outline-variant roles carry text, helper copy and boundaries. The older `neutral-outline` value is retained for incumbent compatibility; native Android fields now use theme roles.
- **Android inverse roles:** Inverse-surface, inverse-on-surface and inverse-primary provide the inherited Material transient-surface vocabulary.
- **Camera black:** The Android preview stage uses its dedicated black resource for contained-image letterboxing in both appearances.

Desktop `desktop-error` is the shared functional ink for status failures and connection errors. Android error and on-error/container pairs are functional validation roles, recorded in the frontmatter rather than promoted to a decorative accent. Component frontmatter shows the light fallback assignments; `.impeccable/design.json` maps every Android resource role to its dark counterpart and theme attribute. `DynamicColors.applyToActivityIfAvailable` supplies supported Android 12+ system colors before view inflation. Do not force fallback swatches onto dynamic themes.

**The One-Accent Rule.** Use lavender for desktop selection and focus; let the live image hold the visual attention.

**The Native-Role Rule.** Resolve Android controls through Material theme attributes, including their paired foreground roles; the light/dark fallback tokens describe the fallback, not a fixed global palette.

## Typography

**Display Font:** None; the interface has no display headline.
**Body Font:** Geist on desktop; Android declares bundled Geist at theme level and uses native Material 3 semantic text appearances.
**Label/Mono Font:** Geist Mono for desktop changing stream measurements.

**Character:** Compact desktop labels keep camera settings scannable. Android uses the native semantic hierarchy, sentence case and scalable text rather than inheriting desktop pixel sizes.

### Hierarchy
- **Body and labels:** The existing desktop body token carries ordinary interface text.
- **Telemetry:** The existing desktop telemetry token carries frame rate, bitrate and frame age.
- **Android title large:** Settings-sheet and camera-access titles use `textAppearanceTitleLarge`.
- **Android title medium:** Readiness and settings section headings use `textAppearanceTitleMedium`.
- **Android body medium:** Supporting copy uses `TextAppearance.OpenCam.Supporting`, inherited from Material 3 BodyMedium with on-surface-variant color.
- **Android label large:** Action labels and slider labels use the Material 3 LabelLarge role.
- **Android label medium:** The live format summary uses `textAppearanceLabelMedium`.

Android metrics in the frontmatter are the inherited Material 1.14.0 role defaults, expressed in scalable pixels; actual rendered size follows Android font scaling. Material text appearances also carry their own regular/medium typeface assignments. A theme-level Geist declaration alone does not establish a verified family override for every inherited appearance; no unverified per-role font family or custom weight is canonized here.

**The Label-First Rule.** Keep desktop labels at the inherited 12px scale and reserve mono type for live measurements.

**The Scalable-Role Rule.** Use Android semantic text appearances and content measurement; never shrink labels or assign a fixed control height to compensate for large text.

## Layout

Desktop uses a 38px top bar, a flexible contained preview, two 44px vertical rails, and a 28px status strip. Icon targets are 32px square. The selected tool opens one 264px palette over the preview, inset from the top and tool rail; the preview keeps the remaining width. The initial logical window is 1280×800 and the minimum is 800×560. These are dense desktop logical pixels, not phone touch-target dimensions.

Android uses a root ConstraintLayout with the `android-content-gutter` resource plus system-bar, cutout and keyboard insets. The readiness summary wraps its text and Connection action through a native Flow. In compact windows a nested camera body fills the space below the measured summary. The naturally measured toolbar is anchored to the body bottom; a native relative guideline bounds overflowing tool content to the lower half of that remaining body. Constrained wrap height keeps ordinary tools at their measured height, while the preview stretches into the remaining space above them. This relative overflow bound is not a fixed toolbar height or a fixed screen position. Camera, Video, Exposure and Effects use wrapping labeled buttons, at most two per Flow row. The toolbar can scroll when content exceeds its allocation; there is no activity-calculated panel height.

The `w600dp` resources put a content-measured, vertically scrollable tool column beside the preview. Its width comes from native control measurement rather than a fixed panel-width token. The compact and wide preview stages remain flexible and preserve the camera transform. Android spacing comes from the reused micro, compact, control, section and gutter resources; sheet content uses the section inset and control/compact gaps. Minimum targets and icon size are separate tokens, not density conversions from the desktop rail.

Native sheets measure to content and provide their own bounded scrolling and keyboard resize. Sheet, editor and scroll state restore after activity recreation; uncommitted password drafts stay in a ViewModel.

Actual native emulator captures cover compact/wide, dark/light, tablet, large text and password keyboard/error states. The earlier corrected compact evidence is `.impeccable/review/mobile-finish/phone-dark.png` and `small-phone-large-text.png`, with earlier state captures under `mobile-round2`. The current connection refinement is captured under `docs/connection-polish`: actual Linux compositor output plus API 36 phone/tablet appearances, orientations, large text, keyboard, clipboard confirmation and listener error/recovery states. The corrected small-phone capture retains a visible preview at 200% text and scrolls oversized tools below it. The vivid camera images are synthetic emulator fixtures, not shipped imagery, competitor photographs or palette references. Native build, lint and tests passed on API 36 emulation; physical handset and TalkBack comfort remain unverified. The desktop minimum size is a source constraint, not an interactively verified layout; Windows and macOS visual behavior remains unverified. Hosted native builds cover compilation, tests and packaging on those platforms.

**The Open-Frame Rule.** Keep the desktop preview flexible, with two slim rails and only the selected tool floating over it.

**The Intrinsic-Phone Rule.** Let Android labels, summaries and native controls measure themselves; constrain and scroll overflow instead of introducing rail offsets or manually assigned panel heights.

## Elevation & Depth

The desktop shell relies on tonal surfaces and a thin divider. Floating palettes and selection menus use gpui-base `shadow_lg()` and the shared 16px backdrop-blur sigma inside one scene layer; on opaque platforms the surface passes through without blur. The preview itself is never blurred. Android uses Material 3 tonal surface roles and native BottomSheetDialog elevation and modal scrim; there is no custom Android shadow or backdrop-blur token. Native ripple, focus and sheet transitions stay with the inherited widgets rather than an invented app motion scale.

## Shapes

Desktop buttons and input frames use gently rounded 6px corners. The action rail, floating tool palettes, and option menus use 12px corners. Android retains Material 3 tonal/contained/outlined button, icon-button, outlined-field and modal-sheet shapes. Minimum targets do not fix the visible pill height or radius; labels may increase measured size. No screenshot-measured Android radius is promoted to a primitive token.

## Components

### Buttons
- **Desktop icon buttons:** Compact 32px squares with 16px supplied SVG artwork. Resting controls use a faint white wash; hover and selection increase that wash, selected icons use the accent, and keyboard focus adds a 1px accent border. Disabled controls fade to 40% opacity.
- **Desktop sliders:** A 2px neutral track with an accent progress segment and a small light thumb; the entire 20px control is keyboard focusable.
- **Android actions:** Native contained, tonal, outlined and text buttons use minimum targets rather than fixed heights. Connection and pairing-copy actions are tonal; password save and camera access are contained; advanced disclosures are outlined; discard is a text action. Action labels state the operation directly and can wrap where the app supplies native action styles.
- **Android tools:** Checked tool buttons resolve to primary-container/on-primary-container; unchecked tools use surface-container-high/on-surface. Compact and wide tools place the vector before the label, reducing natural toolbar height while retaining named controls. Both retain accessible text labels and native state feedback.

### Inputs / Fields
- **Desktop:** gpui-base input frames use a faint surface wash, 6px corners, and an accent border on focus. Resolution and frame-rate controls pair editable text with a suggestion menu and a separate apply action.
- **Android password:** Material 3 outlined field with a persistent label, helper/error text and native visibility toggle. Save and discard actions make the draft state explicit.
- **Android camera editors:** Native outlined exposed-dropdown fields permit supported suggestions or typed values, with a separately named check icon to apply. Focused draft text and selection restore across recreation; updates do not overwrite an active editor.
- **Android switches and sliders:** Capability-driven native widgets use labeled controls, native semantic states and minimum targets. Unsupported camera controls are not invented.

### Navigation
- **Desktop:** The connection/output action rail and camera-tool rail stay separate and icon-led. Selection is visible in the rail while its detailed controls live in a floating palette.
- **Android:** Readiness and Connection sit above the preview; labeled camera tools open task-specific sheets. No app-name toolbar or desktop navigation rail is added.

### Floating Tool Palette

The selected desktop tool is a compact surface over the preview, sized to its controls and vertically scrollable when needed. Selection menus use the same radius and blur treatment. Closing the palette returns the full preview.

### Connection Feedback

Desktop transport actions use direct labels and show Cancel connection during an attempt. Pairing and password fields accept Enter; discovered-phone selection reflects the active connection while the editable link remains a draft. Recovery guidance accompanies functional error ink. Android pairing guidance stays separate from camera errors, and listener failures offer a native Retry connection action. Password edits pause discovery until save or discard, including an emptied draft. Pairing copy uses a sensitive native clipboard item, system confirmation on Android 13+, and a snackbar on earlier versions.

### Native Settings Sheet

A native BottomSheetDialog contains a drag handle, semantic title, named close icon and scrollable task content. Camera, Video, Exposure, Effects and Connection have distinct groups. Advanced outlined disclosures reveal less common controls and announce expanded/collapsed state. The dialog fits content, skips collapsed presentation and opens expanded; dismissal clears tool selection. The keyboard resizes the sheet and hides the drag handle while editing. No app-defined percentage height, fixed panel height or handmade modal surface replaces the native dialog.

The sidecar keeps the existing desktop live-panel primitives and adds self-contained HTML/CSS translations of the shipped Android action, tool, field, disclosure and sheet patterns. These translations illustrate native states and fallback roles; CSS pixels and drawn shapes in panel samples are not additional Android dp, shape, motion or elevation tokens.

## Do's and Don'ts

### Do:
- **Do** keep the live preview at the center and let it fill the available desktop space.
- **Do** use the supplied Zeron and Solar vector assets for their applicable controls.
- **Do** keep Android controls at their native minimum target while allowing labels to wrap and large text to scroll.
- **Do** keep labels concise and make each icon action accessible by name.
- **Do** use Android semantic color and text roles with system appearance and available dynamic color.
- **Do** group Android settings by task and preserve native sheet, editor and scroll behavior.

### Don't:
- **Don't** add a permanent desktop settings wall, fixed bottom dock, or redundant app-name toolbar.
- **Don't** add desktop headings or explanatory copy when a direct control label already explains the action; Android sheet titles, section headings and necessary helper copy are native parts of the system.
- **Don't** substitute handmade glyphs, emoji, or text symbols for the shipped vector icons.
- **Don't** add ornamental animation to camera controls or preview surfaces.
- **Don't** blur, crop, or paint decorative imagery over the camera preview.
- **Don't** treat synthetic camera fixtures in review captures as the product palette or a real physical-camera feed.
- **Don't** impose desktop pixel density, fixed toolbar dimensions or manual rail offsets on Android.
