# OpenCam

<!-- impeccable:product-schema 1 -->

## Platform
adaptive

## Stack
Rust with Zui (Zeron’s GPUI fork) and gpui-base on Linux, Windows and macOS; native Java/Camera2 on Android.
The user explicitly chose Zeron’s toolkit and visual reference. Native Android Views use Material3 controls, themed to the same neutral palette.

## Users
The user wants to use a Nothing phone as a desktop camera, with less lag than Iriun and access to individual lenses and camera settings.

## Product Purpose
Send camera video over USB and encrypted Wi-Fi and expose the camera capabilities and capture metadata Android makes available to third-party apps.

## Operating Context
All three desktop platforms were explicitly requested. Low latency and throughput are the priority. The exact phone model remains unconfirmed.

## Capabilities and Constraints
Camera2 enumeration, logical and physical lenses, manual exposure/focus/white balance/stabilization and ISP modes where supported, RAW stills and full capability export. Phone-side native EGL/GLES stretch, fit/crop, radial/local distortion, rotate/mirror and optional bounded person segmentation/background blur. Prefer direct sensor/ISP controls; bypass GPU and ML when effects are off.

Heavy effects can run on the desktop instead. Rust wgpu compute supports Vulkan, DirectX 12 and Metal with explicit GPU selection, measured Auto or parallel CPU fallback. ONNX Runtime segmentation offers CPU, CUDA, ROCm/MIGraphX, OpenVINO, DirectML and CoreML when the installed runtime supports them. Video, processing and ML use bounded latest-frame work; both preview and virtual camera receive the same effects. Hardware availability and timings are reported, never assumed.
Use discovered Android hardware H.264/HEVC codecs, with user selection and an automatic benchmark; USB ADB forwarding and encrypted, certificate-pinned Wi-Fi. Native desktop preview and direct virtual-camera frames: Linux V4L2 loopback, Windows Unity Capture and macOS OBS Camera Extension. No app-window capture. Installed system drivers required; their runtime integration remains hardware-dependent.
Firmware can hide sensors or processing features. The app must describe exposed capabilities rather than promise undocumented access.
No attached phone is currently available for hardware verification. No comparative latency measurement is available.

## Product Principles
- Actual device capabilities drive every camera option.
- Recent frames take priority over accumulating a preview backlog.
- Camera access starts explicitly and stops with the companion's lifecycle.
- No cloud, subscriptions, analytics or hardcoded Nothing camera IDs.

## Open Decisions
The product is OpenCam, with MIT-licensed source published on GitHub. Custom Windows/macOS camera drivers are outside this first version.
