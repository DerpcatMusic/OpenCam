---
name: OpenCam
description: "A native camera-control surface centered on a direct live preview."
colors:
  primary: "#a5a0ff"
  neutral-bg: "#060606"
  neutral-rail: "#0d0d0d"
  neutral-panel: "#111111f5"
  neutral-variant: "#161616"
  neutral-menu: "#161616eb"
  neutral-divider: "#252525"
  neutral-outline: "#626262"
  neutral-text: "#e8e8e8"
  neutral-muted: "#a8a8a8"
  neutral-slider-track: "#4b4d55"
  neutral-slider-thumb: "#cdced4"
  control-field-wash: "rgba(255,255,255,0.055)"
  control-rest-wash: "rgba(255,255,255,0.035)"
  control-selected-wash: "rgba(255,255,255,0.11)"
typography:
  body:
    fontFamily: "Geist, sans-serif"
    fontSize: "12px"
  telemetry:
    fontFamily: "Geist Mono, monospace"
    fontSize: "11px"
rounded:
  sm: "6px"
  md: "12px"
spacing:
  desktop-rail: "44px"
  desktop-icon-button: "32px"
  desktop-tool-palette: "264px"
  android-content-gutter: "24dp"
  android-min-control-target: "48dp"
components:
  desktop-edit-field:
    backgroundColor: "{colors.control-field-wash}"
    textColor: "{colors.neutral-text}"
    rounded: "{rounded.sm}"
  desktop-rail-button:
    backgroundColor: "{colors.control-rest-wash}"
    textColor: "{colors.neutral-text}"
    rounded: "{rounded.sm}"
    width: "{spacing.desktop-icon-button}"
    height: "{spacing.desktop-icon-button}"
  desktop-rail-button-selected:
    backgroundColor: "{colors.control-selected-wash}"
    textColor: "{colors.primary}"
    rounded: "{rounded.sm}"
    width: "{spacing.desktop-icon-button}"
    height: "{spacing.desktop-icon-button}"
  desktop-navigation-rail:
    backgroundColor: "{colors.neutral-rail}"
    rounded: "{rounded.md}"
    width: "{spacing.desktop-rail}"
  desktop-tool-palette:
    backgroundColor: "{colors.neutral-panel}"
    textColor: "{colors.neutral-text}"
    rounded: "{rounded.md}"
    width: "{spacing.desktop-tool-palette}"
  android-primary-button:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.neutral-bg}"
  android-password-field:
    textColor: "{colors.neutral-text}"
    width: "100%"
---

# Design System: OpenCam

## Overview

**Creative North Star: "The Live Camera Instrument"**

OpenCam treats the live camera image as the main work surface. The desktop uses Zui and gpui-base in the pinned Zeron visual language: near-black planes, compact labeled controls, separate vertical action and tool rails, and one floating palette for the selected tool. Camera content stays clear while supported floating menus receive a restrained frost treatment.

The Android companion keeps the same neutral palette and Geist family through native Material 3 Views. It is a single, full-width control screen without a redundant app-name toolbar; Android text scaling and scrolling take precedence over desktop density.

**Key Characteristics:**
- Direct preview first, controls kept to the edges.
- One lavender accent for desktop selection and focus, and the Android primary action.
- Native desktop controls and native Material 3 companion controls.

## Colors

Near-black surfaces carry the interface; one lavender accent marks interaction, while white and gray carry labels and separators.

### Primary
- **Soft lavender:** The shared selection, focus, and Android primary-action color.

### Neutral
- **Night canvas:** The desktop and companion screen ground.
- **Raised rail:** The desktop action rail surface.
- **Floating panel:** The desktop tool palette surface.
- **Material variant:** Android variant surfaces and the opaque base for desktop menus.
- **Menu glass:** Selection menus and tooltips.
- **Hairline divider:** Desktop separators and the top-chrome rule.
- **Android outline:** Material 3 field outline.
- **Main ink:** Primary labels and control text.
- **Muted ink:** Secondary labels, status, and telemetry.
- **Slider track and handle:** The desktop slider's neutral rail and thumb.
- **Rest wash:** Subtle unselected desktop button and field fill.
- **Field wash:** The desktop editable-field frame.
- **Selected wash:** Hover and selected desktop control fill.

**The One-Accent Rule.** Use the lavender for desktop selection and focus, and for the Android primary action; let the live image hold the visual attention.

## Typography

**Display Font:** None; the interface has no display headline.
**Body Font:** Geist (bundled on desktop and Android).
**Label/Mono Font:** Geist Mono for changing stream measurements.

**Character:** Compact, direct labels keep camera settings scannable. The mono face is reserved for values that change continuously.

### Hierarchy
- **Body and labels** (12px desktop base): Shared by desktop control labels and ordinary interface text.
- **Telemetry** (11px): Geist Mono for frame rate, bitrate, and frame age.
- **Android roles:** Material 3 text appearances use Geist and follow Android font scaling; the layout scrolls when larger text needs room.

**The Label-First Rule.** Keep desktop labels at the inherited 12px scale and reserve mono type for live measurements.

## Layout

Desktop uses a 38px top bar, a flexible contained preview, two 44px vertical rails, and a 28px status strip. Icon targets are 32px square. The selected tool opens one 264px palette over the preview, inset from the top and tool rail; the preview keeps the remaining width. The initial logical window is 1280×800 and the minimum is 800×560. These are dense desktop logical pixels, not phone touch-target dimensions.

Android uses one vertical scroll view with 24dp content insets. The switch and both actions have 48dp minimum touch targets; fields and buttons span the content width. Insets account for system bars, cutouts, and the keyboard. Some desktop review images show a labeled synthetic video fixture, so vivid bars in those captures are not part of the interface palette or a camera sample. The available captures document these UI states and do not establish physical-phone, Windows, or macOS visual verification.

**The Open-Frame Rule.** Keep the preview flexible, with two slim rails and only the selected tool floating over it.

## Elevation & Depth

The desktop shell relies on tonal surfaces and a thin divider. Floating palettes and selection menus use gpui-base `shadow_lg()` and the shared 16px backdrop-blur sigma inside one scene layer; on opaque platforms the surface passes through without blur. The preview itself is never blurred. The Android companion keeps Material 3 surface and elevation behavior without a custom app shadow token.

## Shapes

Desktop buttons and input frames use gently rounded 6px corners. The action rail, floating tool palettes, and option menus use 12px corners. The Android companion keeps Material 3 control shapes rather than imposing desktop radii.

## Components

### Buttons
- **Desktop icon buttons:** Compact 32px squares with 16px supplied SVG artwork. Resting controls use a faint white wash; hover and selection increase that wash, selected icons use the accent, and keyboard focus adds a 1px accent border. Disabled controls fade to 40% opacity.
- **Desktop sliders:** A 2px neutral track with an accent progress segment and a small light thumb; the entire 20px control is keyboard focusable.
- **Android actions:** Native Material 3 contained and outlined buttons, full width, with a 48dp minimum target. Labels state the action directly.

### Inputs / Fields
- **Desktop:** gpui-base input frames use a faint surface wash, 6px corners, and an accent border on focus. Resolution and frame-rate controls pair editable text with a suggestion menu and a separate apply action.
- **Android:** The password uses Material 3's outlined text field with a persistent label, helper text, and the native password-visibility control.

### Navigation
- **Desktop:** The connection/output action rail and camera-tool rail stay separate and icon-led. Selection is visible in the rail while its detailed controls live in a floating palette.
- **Android:** One screen has no app toolbar or navigation rail.

### Floating Tool Palette

The selected tool is a compact surface over the preview, sized to its controls and vertically scrollable when needed. Selection menus use the same radius and blur treatment. Closing the palette returns the full preview.

## Do's and Don'ts

### Do:
- **Do** keep the live preview at the center and let it fill the available desktop space.
- **Do** use the supplied Zeron and Solar SVG assets for their applicable controls.
- **Do** keep Android controls at their native 48dp minimum while allowing large text to scroll.
- **Do** keep labels concise and make each icon action accessible by name.

### Don't:
- **Don't** add a permanent settings wall, fixed bottom dock, or redundant app-name toolbar.
- **Don't** add headings or explanatory copy when a direct control label already explains the action.
- **Don't** substitute handmade glyphs, emoji, or text symbols for the shipped vector icons.
- **Don't** add ornamental animation to camera controls or preview surfaces.
- **Don't** blur, crop, or paint decorative imagery over the camera preview.
- **Don't** treat synthetic fixture bars in review captures as the product palette or a real camera feed.
