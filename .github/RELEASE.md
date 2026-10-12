OpenCam is a native, MIT-licensed Android-camera studio with USB and encrypted local Wi-Fi, live Camera2 controls, GPU effects, person background blur and direct virtual-camera output.

Android 0.1.1 opens directly into live camera preview. Four vertical tool icons expose camera/focus, resolution/FPS, sensor and phone effects. Tap the small connection indicator for availability and password settings; local discovery starts automatically while the app is open. The version code is increased so this APK updates the original 0.1.0 release.

This release also includes the mobile fixes merged after 0.1.0: native preview with corrected orientation, synchronized phone/desktop camera controls, bounded uncompressed YUV420/RGBA streaming, authenticated connection recovery, and RAW DNG transfer with preview recovery after capture failures. Password protection uses an explicit Save password action; desktop availability can be disabled while keeping local preview open.

- **Linux x86_64:** extract the `.tar.gz` and launch `opencam`. Ubuntu 24.04 or a compatible distribution; Wayland/X11 libraries and a GPU driver are required.
- **macOS 14+:** extract the archive for Apple Silicon (`aarch64`) or Intel (`x86_64`) and open `OpenCam.app`. These builds are ad-hoc signed, without Apple notarization.
- **Windows 10/11 x86_64:** install the [Microsoft Visual C++ x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist), extract the entire `.zip` and launch `opencam.exe` with its DLLs beside it. No Windows code-signing certificate is configured.
- **Android 11+:** install `opencam-android-release.apk`. It uses the project's persistent release signing key, so subsequent releases can update it.

FFmpeg decoding libraries and the CPU ONNX Runtime are included in desktop packages. GPU inference requires a provider-enabled runtime. Camera output needs v4l2loopback and the system `ffmpeg` command on Linux, Unity Capture on Windows, or the OBS Camera Extension on macOS; see the README. iOS is not implemented.

Release builds and protocol/processing checks do not verify your particular phone, system camera driver, GPU inference SDK or real-world latency. This remains an early version. See `SHA256SUMS` for download verification.
