OpenCam is a native, MIT-licensed Android-camera studio with USB and encrypted local Wi-Fi, live Camera2 controls, GPU effects, person background blur and direct virtual-camera output.

OpenCam 0.1.2 brings an adaptive Android camera layout: labeled Camera, Video, Exposure and Effects controls, intrinsic preview space, native settings sheets, system light/dark appearance and dynamic color. Compact phones keep controls within reach; wide windows put them beside the preview. Mounted and handheld use are equal priorities, and large text, keyboard errors and RTL layouts are covered by emulator verification.

The desktop Connection panel now offers labeled Wi-Fi/USB actions, keyboard password submission, protected-phone guidance and cancel/recovery controls. Network and host interruptions participate in automatic reconnect; stale previews clear during interruption or disconnect. Android shows persistent listener errors with a Retry connection action, refreshes the current address before copying a pairing link, and keeps copied credentials out of clipboard previews. Local preview, synchronized camera controls, uncompressed YUV420/RGBA and RAW DNG transfer remain available.

- **Linux x86_64:** extract the `.tar.gz`. To install an Applications-menu entry and Desktop shortcut, run `python3 install-desktop.py --desktop` inside the extracted folder. It installs for your user and retains older installed versions. You can also launch `opencam` directly. Ubuntu 24.04 or a compatible distribution; Wayland/X11 libraries and a GPU driver are required.
- **macOS 14+:** extract the archive for Apple Silicon (`aarch64`) or Intel (`x86_64`) and open `OpenCam.app`. These builds are ad-hoc signed, without Apple notarization.
- **Windows 10/11 x86_64:** install the [Microsoft Visual C++ x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist), extract the entire `.zip` and launch `opencam.exe` with its DLLs beside it. No Windows code-signing certificate is configured.
- **Android 11+:** install `opencam-android-release.apk`. Version code 3 and the persistent project signing key allow this APK to update 0.1.0 or 0.1.1.

FFmpeg decoding libraries and the CPU ONNX Runtime are included in desktop packages. GPU inference requires a provider-enabled runtime. Camera output needs v4l2loopback and the system `ffmpeg` command on Linux, Unity Capture on Windows, or the OBS Camera Extension on macOS; see the README. iOS is not implemented.

Release builds and protocol/processing checks do not verify your particular phone, system camera driver, GPU inference SDK or real-world latency. This remains an early version. See `SHA256SUMS` for download verification.
