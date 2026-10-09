OpenCam is a native, MIT-licensed Android-camera studio with USB and encrypted local Wi-Fi, live Camera2 controls, GPU effects, person background blur and direct virtual-camera output.

- **Linux x86_64:** extract the `.tar.gz` and launch `opencam`. Ubuntu 24.04 or a compatible distribution; Wayland/X11 libraries and a GPU driver are required.
- **macOS:** extract the archive for Apple Silicon (`aarch64`) or Intel (`x86_64`) and open `OpenCam.app`. These builds are ad-hoc signed, without Apple notarization.
- **Windows x86_64:** extract the entire `.zip` and launch `opencam.exe` with its DLLs beside it. No Windows code-signing certificate is configured.
- **Android 11+:** install `opencam-android-release.apk`. It uses the project's persistent release signing key, so subsequent releases can update it.

FFmpeg and the CPU ONNX Runtime are included in desktop packages. GPU inference requires a provider-enabled runtime. Camera output needs v4l2loopback on Linux, Unity Capture on Windows, or the OBS Camera Extension on macOS; see the README. iOS is not implemented.

Release builds and protocol/processing checks do not verify your particular phone, system camera driver, GPU inference SDK or real-world latency. This remains an early version. See `SHA256SUMS` for download verification.
