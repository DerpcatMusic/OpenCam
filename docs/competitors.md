# Iriun and Camo: evidence and engineering targets

Reviewed 2026-10-09. OpenCam has no measured latency advantage over either product yet.

## Iriun Linux 2.9.3

Official package: https://iriun.com/iriunwebcam-2.9.3.deb (linked by https://iriun.com/).
Package SHA-256: `3635363ec642788aca1e5a3fd45ee978ab6f69ac0a0d2ab5d68ae11bc4a80cc9`.
Executable `usr/local/bin/iriunwebcam`, x86-64 ELF, SHA-256:
`7797be44331eb71079affa4af40ff78bbcfbd46e1e0692362717dc74454a1c65`.

Analyzed with MCP REA, Ghidra 12.1.4, default complete static analysis. The package was extracted as data; its executable and installation scripts were not run. The local evidence bundle is kept in the ignored review directory, without redistributing the proprietary binary or decompiled code.

| Finding | Evidence | What it establishes |
|---|---|---|
| Qt desktop controls | Imported Qt symbols; strings at `0x34b011`, `0x34b035`, `0x34b06f` | Camera switching, torch and focus-distance UI/control handlers exist. |
| USB via ADB forwarding | Strings `0x34b60f`, `0x34b61d`, `0x34b627` | ADB forward command construction targets phone TCP ports 4697 and 4699. |
| Network endpoint | User-facing string `0x34a198`; connect function `0x15c930` | Port 4699 is named for connection. The connector calls POSIX connect, waits for writability when needed, then enters `0x15afe0`. |
| Framed receive loop | Function `0x15afe0`, caller of recv thunk `0x123b10` | Accumulates a four-byte header and packet body in a bounded 4,000,000-byte receive allocation. Low 24 header bits represent packet length; the high byte is dispatched with the packet to a callback. |
| Receive readiness | `0x15afe0` calls poll with a 100 ms timeout | This is a maximum idle wait, **not a mandatory 100 ms delay per frame**: readable sockets wake immediately. It does not prove the cause of lag. |
| Discovery candidate | recvfrom loop `0x159540`; service-name string `0x34ba66` | Network discovery machinery and an Iriun service identifier are present. A repeated five-by-100-ms sleep appears in this loop; interpreting it as discovery cadence is an inference, not stream latency. |
| Decoder code | Embedded `libavcodec/h264dec.c` / H.264 decoder strings | FFmpeg H.264 decoding code is bundled. This alone does not prove every session uses H.264 or identify buffering behavior. |
| Linux video/audio integration | v4l2loopback error string `0x34a220`, packaged module configuration, PipeWire SPA audio plugin | Video uses a v4l2loopback device; a PipeWire audio integration is shipped. |

REA evidence IDs:

- Receive dossier: `ev_8aa2f20ef2262882392ce2ecf8d9066795f227cf798728f41248a780a3c04d1d`.
- Connector dossier: `ev_88cacf0b9d923bc75c89bf4fe7a131301f8215486ee5357f7b46293819eea7fd`.
- Discovery candidate dossier: `ev_7fa7ff2e216b92a19f526e73539676258b62243b060d4d61340a4c380d1a85dd`.
- String inventory: `ev_e9fb00f62143a2bbf446f4ab4c1b736dbe0d7f4760e98371f098f6d21d1c0052`.

Limits: stripped names, indirect callback targets unresolved; no live packets, phone APK, hardware run, codec negotiation, encryption assessment, or end-to-end timing measurement. Findings apply only to this Linux build. OpenCam uses independently written Camera2/MediaCodec, TLS and native output code; Iriun's protocol/code was not copied.

## Camo's documented baseline

Official sources: https://camo.com/camera and https://camo.com/studio.

Camo documents wired/wireless Android and iOS capture; remote lens, focus, zoom, white balance and exposure/ISO controls; phone-side effects; background treatment, LUTs, framing, profiles, recording, scenes/overlays and virtual-camera integration. Studio is offered for Windows and macOS, with additional iPad functionality. These are documented capabilities, not independently benchmarked results.

## What OpenCam implements in this change

- Native Android live preview without an encoder until streaming is requested.
- Enumerated front/rear/logical/physical camera paths, editable resolution/FPS, sensor exposure/focus/white-balance/ISP controls, torch and stabilization where exposed.
- Native phone geometry/blur/output controls with GLES processing; a second GPU presentation surface shows the same processed pixels sent to the encoder.
- Camera settings applied by one phone handler, acknowledged to both screens. Revisions reject stale commands. Pairing carries the phone's current selection instead of resetting it.
- Desktop retains its independent processing settings when sensor controls arrive from the phone. Editing phone effects while desktop processing is active transfers effects to the phone to avoid two competing processing stages.

## What must be measured or built before superiority claims

Use the same phone/lens, resolution, FPS, bitrate, codec, scene and network for all products. Measure glass-to-glass p50/p95 latency with a visible clock or LED, sustained decoded/output FPS, stalls/dropped frames, CPU/GPU use, thermal throttling, battery, reconnect and hours-long stability. Compare Wi-Fi, PC hotspot, desktop Ethernet and USB separately. Decoder age estimates and CI throughput are not glass-to-glass latency measurements.

Audio, iOS capture, easy signed driver installation, desktop hardware decoding, zero-copy output, recording, HDR, LUTs and scenes remain gaps. No promise to outperform Camo or Iriun on hardware is made until equivalent measurements exist.
