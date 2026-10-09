package dev.opencam;

import android.graphics.ImageFormat;
import android.hardware.camera2.*;
import android.hardware.camera2.params.StreamConfigurationMap;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaCodecList;
import android.os.Build;
import android.util.Range;
import android.util.Size;
import java.util.*;
import org.json.*;

final class Catalog {
    record Lens(String id, String openId, String physicalId, CameraCharacteristics chars) {}
    final CameraManager manager;
    private final Map<String, JSONObject> descriptions = new HashMap<>();
    private JSONObject snapshot;
    final LinkedHashMap<String, Lens> lenses = new LinkedHashMap<>();

    Catalog(CameraManager manager) throws CameraAccessException {
        this.manager = manager;
        Set<String> publicIds = new HashSet<>(Arrays.asList(manager.getCameraIdList()));
        for (String id : publicIds) {
            CameraCharacteristics c = manager.getCameraCharacteristics(id);
            lenses.put(id, new Lens(id, id, null, c));
            for (String physical : c.getPhysicalCameraIds()) {
                if (publicIds.contains(physical)) continue;
                String key = id + "/" + physical;
                lenses.put(key, new Lens(key, id, physical, manager.getCameraCharacteristics(physical)));
            }
        }
    }

    static boolean has(CameraCharacteristics c, int capability) {
        int[] caps = c.get(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES);
        return caps != null && Arrays.stream(caps).anyMatch(x -> x == capability);
    }

    static boolean mode(int[] values, int value) {
        return values != null && Arrays.stream(values).anyMatch(x -> x == value);
    }

    boolean writable(Lens lens, CaptureRequest.Key<?> key) throws CameraAccessException {
        if (lens.physicalId == null) return lens.chars.getAvailableCaptureRequestKeys().contains(key);
        List<CaptureRequest.Key<?>> keys = manager.getCameraCharacteristics(lens.openId).getAvailablePhysicalCameraRequestKeys();
        return keys != null && keys.contains(key);
    }

    Size[] sizes(Lens lens) throws CameraAccessException { return sizes(lens, ""); }
    Size[] sizes(Lens lens,String codec) throws CameraAccessException {
        StreamConfigurationMap map=lens.chars.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        Size[] values=outputSizes(map,codec);
        if (values==null) return new Size[0];
        if (lens.physicalId!=null) {
            StreamConfigurationMap parent=manager.getCameraCharacteristics(lens.openId).get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
            Size[] parentSizes=outputSizes(parent,codec);
            Set<Size> allowed=parentSizes==null?Set.of():new HashSet<>(Arrays.asList(parentSizes));
            values=Arrays.stream(values).filter(allowed::contains).toArray(Size[]::new);
        }
        Arrays.sort(values,Comparator.comparingLong((Size size)->(long)size.getWidth()*size.getHeight()));return values;
    }
    static Size[] outputSizes(StreamConfigurationMap map,String codec) {
        if(map==null)return null;
        return codec.equals(WirePixels.YUV)?map.getOutputSizes(ImageFormat.YUV_420_888):codec.equals(WirePixels.RGBA)?map.getOutputSizes(android.graphics.SurfaceTexture.class):map.getOutputSizes(MediaCodec.class);
    }
    static long minFrameNs(StreamConfigurationMap map,String codec,Size size) {
        return codec.equals(WirePixels.YUV)?map.getOutputMinFrameDuration(ImageFormat.YUV_420_888,size):codec.equals(WirePixels.RGBA)?map.getOutputMinFrameDuration(android.graphics.SurfaceTexture.class,size):map.getOutputMinFrameDuration(MediaCodec.class,size);
    }

    JSONObject describe(Lens l) throws Exception {
        JSONObject cached = descriptions.get(l.id);
        if (cached != null) return cached;
        CameraCharacteristics c = l.chars;
        StreamConfigurationMap map = c.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        JSONObject all = new JSONObject();
        for (CameraCharacteristics.Key<?> key : c.getKeys()) {
            try { all.put(key.getName(), Json.encode(c.get(key))); }
            catch (Exception e) { all.put(key.getName(), "unavailable: " + e.getMessage()); }
        }
        Integer facing = c.get(CameraCharacteristics.LENS_FACING);
        float[] focal = c.get(CameraCharacteristics.LENS_INFO_AVAILABLE_FOCAL_LENGTHS);
        String label = Objects.equals(facing, CameraMetadata.LENS_FACING_FRONT) ? "Front" : "Rear";
        if (!c.getPhysicalCameraIds().isEmpty()) label += " · logical";
        if (focal != null && focal.length > 0) label += String.format(Locale.US, " · %.1f mm", focal[0]);
        Range<Float> zoom = Build.VERSION.SDK_INT >= 30 ? c.get(CameraCharacteristics.CONTROL_ZOOM_RATIO_RANGE) : null;
        Float maximum = c.get(CameraCharacteristics.SCALER_AVAILABLE_MAX_DIGITAL_ZOOM);
        Float focus = c.get(CameraCharacteristics.LENS_INFO_MINIMUM_FOCUS_DISTANCE);
        List<JSONObject> durations = new ArrayList<>();
        for (Size s : sizes(l)) durations.add(Json.object("size", s, "minFrameNs", map.getOutputMinFrameDuration(MediaCodec.class, s)));
        List<JSONObject> yuvDurations=new ArrayList<>(),rgbaDurations=new ArrayList<>();
        for(Size size:sizes(l,WirePixels.YUV))yuvDurations.add(Json.object("size",size,"minFrameNs",minFrameNs(map,WirePixels.YUV,size)));
        for(Size size:sizes(l,WirePixels.RGBA))rgbaDurations.add(Json.object("size",size,"minFrameNs",minFrameNs(map,WirePixels.RGBA,size)));
        List<JSONObject> highSpeed = new ArrayList<>();
        if (l.physicalId == null && has(c, CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_CONSTRAINED_HIGH_SPEED_VIDEO)) {
            for (Size s : map.getHighSpeedVideoSizes()) highSpeed.add(Json.object("size", s, "fpsRanges", map.getHighSpeedVideoFpsRangesFor(s)));
        }
        boolean manual = has(c, CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_MANUAL_SENSOR)
                && writable(l, CaptureRequest.SENSOR_SENSITIVITY) && writable(l, CaptureRequest.SENSOR_EXPOSURE_TIME)
                && writable(l, CaptureRequest.CONTROL_AE_MODE);
        JSONObject result = Json.object("id", l.id, "openId", l.openId, "physicalId", l.physicalId,
                "label", label, "facing", facing, "sizes", sizes(l), "frameDurations", durations,
                "encodedSizes",sizes(l),"encodedFrameDurations",durations,"yuvSizes",sizes(l,WirePixels.YUV),"rgbaSizes",sizes(l,WirePixels.RGBA),"yuvFrameDurations",yuvDurations,"rgbaFrameDurations",rgbaDurations,
                "fpsRanges", c.get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES),
                "highSpeed", highSpeed, "encodedHighSpeed",highSpeed,"focals", focal, "apertures", c.get(CameraCharacteristics.LENS_INFO_AVAILABLE_APERTURES),
                "iso", c.get(CameraCharacteristics.SENSOR_INFO_SENSITIVITY_RANGE),
                "exposureNs", c.get(CameraCharacteristics.SENSOR_INFO_EXPOSURE_TIME_RANGE),
                "manualSensor", manual, "focusMax", focus == null ? 0 : focus,
                "manualFocus", focus != null && focus > 0 && writable(l, CaptureRequest.LENS_FOCUS_DISTANCE) && writable(l, CaptureRequest.CONTROL_AF_MODE),
                "zoom", zoom == null ? new float[]{1, maximum == null ? 1 : maximum} : zoom,
                "ev", c.get(CameraCharacteristics.CONTROL_AE_COMPENSATION_RANGE),
                "evStep", c.get(CameraCharacteristics.CONTROL_AE_COMPENSATION_STEP),
                "awbModes", c.get(CameraCharacteristics.CONTROL_AWB_AVAILABLE_MODES),
                "noiseModes", c.get(CameraCharacteristics.NOISE_REDUCTION_AVAILABLE_NOISE_REDUCTION_MODES),
                "edgeModes", c.get(CameraCharacteristics.EDGE_AVAILABLE_EDGE_MODES),
                "aberrationModes", c.get(CameraCharacteristics.COLOR_CORRECTION_AVAILABLE_ABERRATION_MODES),
                "lensCorrectionModes", c.get(CameraCharacteristics.DISTORTION_CORRECTION_AVAILABLE_MODES),
                "afModes", c.get(CameraCharacteristics.CONTROL_AF_AVAILABLE_MODES),
                "oisModes", c.get(CameraCharacteristics.LENS_INFO_AVAILABLE_OPTICAL_STABILIZATION),
                "stabilizationModes", c.get(CameraCharacteristics.CONTROL_AVAILABLE_VIDEO_STABILIZATION_MODES),
                "flash", c.get(CameraCharacteristics.FLASH_INFO_AVAILABLE),
                "aeLock", c.get(CameraCharacteristics.CONTROL_AE_LOCK_AVAILABLE),
                "awbLock", c.get(CameraCharacteristics.CONTROL_AWB_LOCK_AVAILABLE),
                "raw", has(c, CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_RAW),
                "rawSizes", map == null ? null : map.getOutputSizes(ImageFormat.RAW_SENSOR),
                "orientation", c.get(CameraCharacteristics.SENSOR_ORIENTATION), "characteristics", all);
        descriptions.put(l.id, result);
        return result;
    }

    JSONObject export() throws Exception {
        if (snapshot != null) return snapshot;
        JSONArray cameras = new JSONArray();
        for (Lens lens : lenses.values()) cameras.put(describe(lens));
        JSONArray codecs = new JSONArray();
        for (MediaCodecInfo info : new MediaCodecList(MediaCodecList.REGULAR_CODECS).getCodecInfos()) {
            if (!info.isEncoder()) continue;
            if (Build.VERSION.SDK_INT >= 29 && info.isAlias()) continue;
            boolean hardware = Build.VERSION.SDK_INT >= 29 ? info.isHardwareAccelerated()
                    : !info.getName().startsWith("OMX.google.") && !info.getName().startsWith("c2.android.");
            if (!hardware) continue;
            for (String type : info.getSupportedTypes()) {
                if (!type.equals("video/avc") && !type.equals("video/hevc")) continue;
                var cap = info.getCapabilitiesForType(type);
                if (!Arrays.stream(cap.colorFormats).anyMatch(x -> x == MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)) continue;
                var v = cap.getVideoCapabilities();
                codecs.put(Json.object("name", info.getName(), "mime", type, "label", type.equals("video/avc") ? "H.264" : "HEVC",
                        "widths", v.getSupportedWidths(), "heights", v.getSupportedHeights(),
                        "bitrate", v.getBitrateRange(), "frameRates", v.getSupportedFrameRates()));
            }
        }
        codecs.put(Json.object("name",WirePixels.YUV,"mime",WirePixels.mime(WirePixels.YUV),"label","Uncompressed YUV420","bitrate",new int[]{1,Integer.MAX_VALUE}));
        codecs.put(Json.object("name",WirePixels.RGBA,"mime",WirePixels.mime(WirePixels.RGBA),"label","Uncompressed RGBA","bitrate",new int[]{1,Integer.MAX_VALUE}));
        snapshot = Json.object("type", "capabilities", "protocol", 1,
                "device", Build.MANUFACTURER + " " + Build.MODEL, "cameras", cameras, "codecs", codecs);
        return snapshot;
    }
}
