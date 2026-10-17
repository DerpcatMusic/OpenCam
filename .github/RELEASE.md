OpenCam is a native, MIT-licensed Android-camera studio with USB and encrypted local Wi-Fi, live Camera2 controls, GPU effects, person background blur and direct virtual-camera output.

OpenCam 0.1.3 adds QR pairing that works over mobile data. Choose **Pair with QR code** in the desktop Connection panel, then **Scan code from computer** in the phone's Connection settings. The phone sends its encrypted pairing link back and the desktop asks you to Accept before connecting. When both devices run [Tailscale](https://tailscale.com), the phone advertises its Tailscale address, so the connection works from mobile data or any other network with no relay or cloud account. The existing certificate pin, token and password checks still apply.

Also new: a **Quick** tab on the phone (torch, autofocus, mirror, follow-orientation, background blur, saturation, sharpen, zoom), hue, saturation, contrast and sharpen controls on phone and desktop, a stronger and steadier background blur (higher-resolution blur chain, colour-aware matte edges, temporal smoothing, no more whole-frame blur flicker when the model stalls), a stream that follows phone rotation (Quick → Follow phone orientation; portrait phone gives a portrait stream), and a new app icon.

Fixes: with desktop processing on, ISO, shutter, lens and effect changes failed with "Stream changes require configure". Live controls no longer carry stream settings, and the phone restarts the stream instead of rejecting a changed one. Resolution and frame-rate fields search anywhere in the list (type `1080` to find `1920x1080`), and changing resolution keeps the nearest frame rate to your current one instead of snapping to 30 fps.

OpenCam 0.1.2 brought an adaptive Android camera layout: labeled Camera, Video, Exposure and Effects controls, native settings sheets, system light/dark appearance and dynamic color.

The desktop Connection panel now offers labeled Wi-Fi/USB actions, keyboard password submission, protected-phone guidance and cancel/recovery controls. Network and host interruptions participate in automatic reconnect; stale previews clear during interruption or disconnect. Android shows persistent listener errors with a Retry connection action, refreshes the current address before copying a pairing link, and keeps copied credentials out of clipboard previews. Local preview, synchronized camera controls, uncompressed YUV420/RGBA and RAW DNG transfer remain available.

- **Linux x86_64:** extract the `.tar.gz`. To install an Applications-menu entry and Desktop shortcut, run `python3 install-desktop.py --desktop` inside the extracted folder. It installs for your user and retains older installed versions. You can also launch `opencam` directly. Ubuntu 24.04 or a compatible distribution; Wayland/X11 libraries and a GPU driver are required.
- **macOS 14+:** extract the archive for Apple Silicon (`aarch64`) or Intel (`x86_64`) and open `OpenCam.app`. These builds are ad-hoc signed, without Apple notarization.
- **Windows 10/11 x86_64:** install the [Microsoft Visual C++ x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist), extract the entire `.zip` and launch `opencam.exe` with its DLLs beside it. No Windows code-signing certificate is configured.
- **Android 11+:** install `opencam-android-release.apk`. Version code 4 and the persistent project signing key allow this APK to update 0.1.0 or 0.1.1.

FFmpeg decoding libraries and the CPU ONNX Runtime are included in desktop packages. GPU inference requires a provider-enabled runtime. Camera output needs v4l2loopback and the system `ffmpeg` command on Linux, Unity Capture on Windows, or the OBS Camera Extension on macOS; see the README. iOS is not implemented.

Release builds and protocol/processing checks do not verify your particular phone, system camera driver, GPU inference SDK or real-world latency. This remains an early version. See `SHA256SUMS` for download verification.
