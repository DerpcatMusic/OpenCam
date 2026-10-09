#define NOMINMAX
#include <windows.h>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <vector>
#include "../vendor/unity-capture/shared.inl"

struct Camera {
    SharedImageMemory memory{0};
    uint32_t width, height;
    std::vector<uint8_t> pixels;
    Camera(uint32_t w, uint32_t h) : width(w), height(h), pixels(size_t(w)*h*4) {}
};
extern "C" void *opencam_camera_create(uint32_t w, uint32_t h, char *error, size_t cap) {
    HKEY key{};
    // The first 64-bit Unity Capture filter, registered by its open-source installer.
    if (RegOpenKeyExW(HKEY_CLASSES_ROOT, L"CLSID\\{5C2CD55C-92AD-4999-8666-912BD3E70010}",
                      0, KEY_READ, &key) != ERROR_SUCCESS) {
        snprintf(error, cap, "Install the Unity Capture virtual-camera driver; select Unity Video Capture in OBS/Zoom.");
        return nullptr;
    }
    RegCloseKey(key);
    if (size_t(w)*h*4 > MAX_SHARED_IMAGE_SIZE || !w || !h) {
        snprintf(error, cap, "Output exceeds the virtual-camera driver's frame limit.");
        return nullptr;
    }
    try { return new Camera(w, h); }
    catch (...) { snprintf(error, cap, "Virtual-camera buffer allocation failed."); return nullptr; }
}
extern "C" bool opencam_camera_send(void *handle, const uint8_t *bgra, size_t len) {
    auto &c = *static_cast<Camera *>(handle);
    if (len != c.pixels.size()) return false;
    if (!c.memory.SendIsReady()) return true;
    // Unity Capture expects bottom-up RGBA; decoded/processed frames are top-down BGRA.
    for (uint32_t y=0; y<c.height; ++y) {
        const auto *src = bgra + size_t(y)*c.width*4;
        auto *dst = c.pixels.data() + size_t(c.height-y-1)*c.width*4;
        for (uint32_t x=0; x<c.width; ++x) {
            dst[4*x]=src[4*x+2]; dst[4*x+1]=src[4*x+1];
            dst[4*x+2]=src[4*x]; dst[4*x+3]=255;
        }
    }
    auto result = c.memory.Send(c.width, c.height, c.width, DWORD(len),
        SharedImageMemory::FORMAT_UINT8, SharedImageMemory::RESIZEMODE_DISABLED,
        SharedImageMemory::MIRRORMODE_DISABLED, 500, c.pixels.data());
    return result != SharedImageMemory::SENDRES_TOOLARGE;
}
extern "C" void opencam_camera_destroy(void *handle) {
    delete static_cast<Camera *>(handle);
}
