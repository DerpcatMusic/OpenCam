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
Send camera video over USB and encrypted Wi-Fi, provide a local Android preview, and expose the camera capabilities and capture metadata Android makes available to third-party apps.

## Operating Context
All three desktop platforms were explicitly requested. Low latency and throughput are the priority. The exact phone model remains unconfirmed.

## Capabilities and Constraints
Camera2 enumeration includes front, rear, logical and physical lenses, editable sensor resolution and FPS, manual exposure/focus/white balance/stabilization and ISP modes where supported, RAW stills and full capability export. Phone-side native EGL/GLES stretch, fit/crop, radial/local distortion, rotate/mirror and optional bounded person segmentation/background blur. Prefer direct sensor/ISP controls; bypass GPU and ML when effects are off.

Heavy effects can run on the desktop instead. Rust wgpu compute supports Vulkan, DirectX 12 and Metal with explicit GPU selection, measured Auto or parallel CPU fallback. ONNX Runtime segmentation offers CPU, CUDA, ROCm/MIGraphX, OpenVINO, DirectML and CoreML when the installed runtime supports them. Video, processing and ML use bounded latest-frame work; both preview and virtual camera receive the same effects. Hardware availability and timings are reported, never assumed.
The phone can preview without starting the stream encoder; the encoder runs for desktop streaming. When phone processing is selected, the same GLES-processed frames feed local preview and encoder. Desktop-side effects do not return video to the phone; editing phone effects switches processing to the phone. Phone and desktop control changes synchronize in both directions, with acknowledgements updating the phone UI.
Use discovered Android hardware H.264/HEVC codecs or uncompressed YUV420/RGBA, with user selection and an automatic benchmark; USB ADB forwarding and encrypted, certificate-pinned Wi-Fi. Native desktop preview and direct virtual-camera frames: Linux V4L2 loopback, Windows Unity Capture and macOS OBS Camera Extension. No app-window capture. Installed system drivers required; their runtime integration remains hardware-dependent.
Firmware can hide sensors or processing features. The app must describe exposed capabilities rather than promise undocumented access.
No attached phone is currently available for hardware verification. No comparative latency measurement is available.

## Product Principles
- Actual device capabilities drive every camera option.
- Recent frames take priority over accumulating a preview backlog.
- Camera access starts explicitly and stops with the companion's lifecycle.
- No cloud, subscriptions, analytics or hardcoded Nothing camera IDs.

## Open Decisions
The product is OpenCam, with MIT-licensed source published on GitHub. Custom Windows/macOS camera drivers are outside this first version.

The architecture extension separates socket reading from decoding/color conversion, reuses conversion buffers, bounds pending video, and rejects late frames from canceled captures. Controls retain priority between 64 KiB video/file chunks. Transient connections retry automatically with 100 ms to 2 s delay, reauthenticate and restore capture. Desktop and phone heartbeat watchdogs expire silent connections. Sensor sizes and frame-duration limits follow the selected transport format; uncompressed formats avoid MediaCodec but require CPU packing and high network bandwidth.

RAW-capable lenses expose a RAW size selector and DNG capture action on phone and desktop. DngCreator still capture preserves sensor metadata; a checksum-verified file download is separate from uncompressed processed-pixel video. Capture resumes after success or failure; completed phone DNGs survive transfer failure. Continuous Bayer RAW video, hardware desktop decoding, zero-copy transport, physical RAW fidelity and competitor latency remain unverified/unimplemented. `docs/transport.md` records the wire contract; `docs/architecture-smoke.json` contains explicitly synthetic verification, including a 5.18 MB DngCreator capture/download from the emulator’s actual RAW_SENSOR surface and restored preview.
