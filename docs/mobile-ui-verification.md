# Native mobile UI verification

The Android redesign uses native Material 3 Views. Content measures the summary, labeled actions and settings sheets. ConstraintLayout allocates the remaining preview space; compact controls wrap and scroll when enlarged, while wide windows put a naturally measured scrollable column beside the preview. Containers have no fixed screen coordinates or Java-assigned panel heights. Spacing, artwork sizes and accessibility minimum targets are shared resource tokens.

The baseline critique and competitor evidence are in [mobile-ui-critique.md](mobile-ui-critique.md). This is a design assessment informed by official product documentation, not a comparative usability study.

## Verification

- `./android/gradlew -p android assembleDebug assembleDebugAndroidTest lintDebug --console=plain` passed; lint has no errors.
- The complete native instrumentation suite passed on API 36, including camera permission/resume, camera selection and synchronization, stale revisions, native preview, GLES processing, segmentation, uncompressed transport, heartbeat, RAW transfer and recovery.
- New assertions check labeled minimum touch targets, preview/control separation, compact bottom reach, native sheets, explicit password discard, password-draft restoration without reconnecting, and focused unapplied camera-editor restoration after activity recreation.
- One earlier invocation stopped at the existing automatic connection-resume assertion before the recreation check. A complete rerun passed. The timing failure remains a local test observation; it is not evidence of a production fix.

The final instrumentation results are [native-checks.json](mobile-ui/native-checks.json). Capture configurations and observed view bounds are in [matrix.json](mobile-ui/matrix.json).

The independent finish review scored both requested material fixes resolved with disposition **ship**; its scope and evidence are recorded in [mobile-ui-finish-review.md](mobile-ui-finish-review.md).

## Device captures

All images in `mobile-ui/` are unedited `adb screencap` output from the Android API 36 Google APIs x86_64 emulator. The colorful house is the emulator's synthetic camera fixture. Aspect-containment bars are part of the preview. These images do not establish physical-camera quality or sensor-orientation behavior on a real phone.

The matrix covers a compact phone in dark and light appearances, all four settings sheets, phone landscape, a 320 × 568 dp phone at 200% text scaling, invalid-password recovery with the keyboard visible, an 800 × 1280 dp tablet and camera sheet, camera-permission recovery, and forced RTL with English labels. Each shipping PNG has a provenance sidecar recording its origin, configuration and checksum.

At 200% text scale, the tool area scrolls so the preview retains space. Native sheets resize for the keyboard and scroll fields and errors into view. Password drafts stay in a ViewModel for configuration changes and are cleared with that ViewModel; only a saved derived credential belongs in preferences.

Physical-phone checks remain: mounted and handheld reach, TalkBack traversal, camera lens/rotation across hardware, and keyboard behavior on manufacturer variants. Emulator and source review verify the implemented layout and state behavior, not measured comfort across users.
