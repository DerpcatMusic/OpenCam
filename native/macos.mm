// Adapted from pyvirtualcam's MIT-licensed macOS Camera Extension backend.
// Copyright (C) 2025 Sebastian Beckmann; (C) 2021 Jannik Vogel.
// Full license: vendor/mac-virtualcam/LICENSE.
#import <Foundation/Foundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreMediaIO/CoreMediaIO.h>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <vector>

struct Camera {
    CMIOObjectID device{};
    CMIOStreamID stream{};
    CMSimpleQueueRef queue{};
    CVPixelBufferPoolRef pool{};
    CMVideoFormatDescriptionRef format{};
    uint32_t width, height;
    bool running{};
    Camera(uint32_t w, uint32_t h) : width(w), height(h) {}
    ~Camera() {
        if (running) CMIODeviceStopStream(device, stream);
        if (format) CFRelease(format);
        if (queue) CFRelease(queue);
        if (pool) CVPixelBufferPoolRelease(pool);
    }
};
extern "C" void *opencam_camera_create(uint32_t w, uint32_t h, char *error, size_t cap) {
    @autoreleasepool {
        auto c = std::make_unique<Camera>(w, h);
        CMIOObjectPropertyAddress a{kCMIOHardwarePropertyDevices,
            kCMIOObjectPropertyScopeGlobal, kCMIOObjectPropertyElementMain};
        UInt32 size{}, used{};
        if (CMIOObjectGetPropertyDataSize(kCMIOObjectSystemObject, &a, 0, nullptr, &size) != noErr) {
            snprintf(error, cap, "Unable to enumerate virtual cameras."); return nullptr;
        }
        std::vector<CMIOObjectID> devices(size/sizeof(CMIOObjectID));
        if (CMIOObjectGetPropertyData(kCMIOObjectSystemObject, &a, 0, nullptr, size, &used, devices.data()) != noErr) return nullptr;
        a.mSelector = kCMIODevicePropertyDeviceUID;
        for (auto device : devices) {
            CFStringRef uid{}; size=sizeof(uid);
            if (CMIOObjectGetPropertyData(device, &a, 0, nullptr, size, &used, &uid) == noErr && uid) {
                bool match=CFEqual(uid, CFSTR("7626645E-4425-469E-9D8B-97E0FA59AC75"));
                CFRelease(uid);
                if (match) { c->device=device; break; }
            }
        }
        if (!c->device) {
            snprintf(error, cap, "Install OBS 30+ Camera Extension, activate it in macOS Settings, then close OBS. Select OBS Virtual Camera in your video app."); return nullptr;
        }
        a.mSelector=kCMIODevicePropertyStreams;
        if (CMIOObjectGetPropertyDataSize(c->device, &a, 0, nullptr, &size)!=noErr) return nullptr;
        std::vector<CMIOStreamID> streams(size/sizeof(CMIOStreamID));
        if (CMIOObjectGetPropertyData(c->device, &a, 0, nullptr, size, &used, streams.data())!=noErr || streams.size()<2) {
            snprintf(error, cap, "Camera Extension input stream unavailable."); return nullptr;
        }
        c->stream=streams[1];
        NSDictionary *attributes=@{
            (id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_422YpCbCr8),
            (id)kCVPixelBufferWidthKey:@(w), (id)kCVPixelBufferHeightKey:@(h),
            (id)kCVPixelBufferIOSurfacePropertiesKey:@{}
        };
        if (CVPixelBufferPoolCreate(kCFAllocatorDefault, nullptr, (__bridge CFDictionaryRef)attributes, &c->pool)!=kCVReturnSuccess ||
            CMVideoFormatDescriptionCreate(kCFAllocatorDefault, kCVPixelFormatType_422YpCbCr8, w, h, nullptr, &c->format)!=noErr ||
            CMIOStreamCopyBufferQueue(c->stream, [](CMIOStreamID, void *, void *){}, nullptr, &c->queue)!=noErr ||
            CMIODeviceStartStream(c->device, c->stream)!=noErr) {
            snprintf(error, cap, "Unable to start the Camera Extension; close other camera senders."); return nullptr;
        }
        c->running=true;
        return c.release();
    }
}
extern "C" bool opencam_camera_send(void *handle, const uint8_t *uyvy, size_t len) {
    auto &c=*static_cast<Camera *>(handle);
    if (len!=size_t(c.width)*c.height*2) return false;
    // Drop rather than queue an older frame when the camera consumer is behind.
    if (CMSimpleQueueGetFullness(c.queue)>=1.0f) return true;
    CVPixelBufferRef buffer{};
    if (CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault,c.pool,&buffer)!=kCVReturnSuccess) return false;
    if (CVPixelBufferLockBaseAddress(buffer,0)!=kCVReturnSuccess) { CFRelease(buffer); return false; }
    auto *dst=static_cast<uint8_t *>(CVPixelBufferGetBaseAddress(buffer));
    auto stride=CVPixelBufferGetBytesPerRow(buffer);
    for (uint32_t y=0;y<c.height;++y) memcpy(dst+y*stride,uyvy+size_t(y)*c.width*2,c.width*2);
    CVPixelBufferUnlockBaseAddress(buffer,0);
    CMSampleTimingInfo timing{kCMTimeInvalid,
        CMTimeMake(clock_gettime_nsec_np(CLOCK_UPTIME_RAW),1000000000),kCMTimeInvalid};
    CMSampleBufferRef sample{};
    bool ok=CMSampleBufferCreateForImageBuffer(kCFAllocatorDefault,buffer,true,nullptr,nullptr,c.format,&timing,&sample)==noErr;
    if (ok && CMSimpleQueueEnqueue(c.queue,sample)!=noErr) { CFRelease(sample); ok=false; }
    CFRelease(buffer);
    return ok;
}
extern "C" void opencam_camera_destroy(void *handle) { delete static_cast<Camera *>(handle); }
