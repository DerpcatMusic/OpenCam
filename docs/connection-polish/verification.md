# OpenCam 0.1.2 connection finishing

The phone keeps a contained live preview in portrait and landscape, with naturally measured native controls and scrollable sheets. Desktop preserves the existing camera-first studio. Connection polish makes the next action clear on both sides:

- Wi-Fi and USB actions are named directly; Enter connects and an in-flight attempt can be cancelled.
- Desktop validates complete clipboard pairing links, separates drafts from the active connection, and supplies recovery guidance for credentials, certificates, discovery and transport failures.
- Phone listener errors persist with Retry connection while camera preview continues. Camera errors and pairing guidance are distinct.
- Copy refreshes the current address and credentials and marks the clipboard item sensitive. Android 13+ uses system clipboard confirmation; older versions receive one app snackbar, following [Android clipboard guidance](https://developer.android.com/develop/ui/views/touch-and-input/copy-paste).
- Password drafts pause discovery until save or discard, including an edited draft that has been emptied.
- The Linux release includes a user installer with Applications and optional Desktop launchers, versioned binaries and a current-version link.

Validation on the final source: 46 Rust binary tests, debug desktop build, full synthetic desktop protocol probe, Android debug/test build, lint with zero errors, and the native API 36 instrumentation suite all passed. Native checks include a deliberately occupied listener port followed by recovery, fresh/sensitive clipboard content, permission resume, saved/emptied draft restoration, TLS/authentication, camera revisions, preview orientation, raw transfer and encoder bypass. Protocol and native result JSON are committed here.

The 18 captures are actual native output, with embedded origin metadata and SHA-256 provenance. Android uses synthetic emulator camera input. Matrix data records phone/tablet, portrait/landscape, dark/light, large text, keyboard and connection states. Linux capture shows the real window in the camera-off empty state. No failed render or mockup is presented as a working application.

Independent Impeccable finish disposition: **ship**, with no material fixes for the captured scope. Persistence and all reviewed fidelity axes passed; native behavior and distinct desktop/phone visual worlds were preserved. The documenter merged only the shared functional desktop error token and source-backed connection behavior into the incumbent design system.

Limits: physical handset and TalkBack comfort, interactive minimum-size desktop behavior, and Windows/macOS visual inspection were not verified by these captures. Hosted CI builds and packages all four desktop targets and Android. Synthetic rendering timings are verification data, not performance claims.
