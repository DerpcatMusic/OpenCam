package dev.opencam;

import android.content.ContentValues;
import android.content.Context;
import android.graphics.ImageFormat;
import android.graphics.Rect;
import android.hardware.camera2.*;
import android.hardware.camera2.params.*;
import android.media.*;
import android.os.*;
import android.provider.MediaStore;
import android.util.Range;
import android.util.Size;
import android.view.Surface;
import java.io.*;
import java.nio.ByteBuffer;
import java.util.*;
import org.json.*;

final class CameraController implements AutoCloseable {
    final Context context;
    final Catalog catalog;
    final Bridge bridge;
    final HandlerThread thread = new HandlerThread("opencam-camera");
    final Handler handler;
    CameraDevice camera;
    CameraCaptureSession session;
    MediaCodec encoder;
    Surface surface;
    PhoneProcessor processor;
    ImageReader rawReader;
    volatile int generation;
    long lastMetadata;
    JSONObject current;
    Catalog.Lens lens;
    TotalCaptureResult rawResult;
    android.media.Image rawImage;
    ColorSpaceTransform autoColorTransform;
    String colorTransformLens;

    CameraController(Context context, Catalog catalog, Bridge bridge) {
        this.context = context;
        this.catalog = catalog;
        this.bridge = bridge;
        thread.start();
        handler = new Handler(thread.getLooper());
        bridge.syncFrame = () -> handler.post(this::requestSync);
    }

    void command(JSONObject command) {
        handler.post(() -> {
            try {
                switch (command.getString("type")) {
                    case "hello", "capabilities" -> bridge.send(catalog.export());
                    case "ping" -> bridge.send(Json.object("type", "pong", "sent", command.optDouble("sent"), "phoneUs", SystemClock.elapsedRealtimeNanos() / 1000));
                    case "configure" -> configure(command.getJSONObject("settings"));
                    case "controls" -> controls(command.getJSONObject("settings"));
                    case "stop" -> { stop(); bridge.send(Json.object("type", "stopped")); }
                    case "raw" -> raw();
                    default -> throw new IllegalArgumentException("Unknown command");
                }
            } catch (Exception e) { fail(e); }
        });
    }

    void fail(Exception e) {
        bridge.send(Json.object("type", "error", "message", e.getClass().getSimpleName() + ": " + e.getMessage()));
        bridge.listener.status("Camera: " + e.getMessage());
    }

    boolean highSpeed(JSONObject s, Catalog.Lens l) {
        Range<Integer>[] regular = l.chars().get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES);
        int fps = s.optInt("fps");
        return Arrays.stream(regular).noneMatch(r -> r.contains(fps));
    }

    Range<Integer> fpsRange(JSONObject s, Catalog.Lens l) {
        int fps = s.optInt("fps");
        if (!highSpeed(s, l)) {
            var ranges = l.chars().get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES);
            int[] range = Limits.fpsRange(Arrays.stream(ranges).map(r -> new int[]{r.getLower(), r.getUpper()}).toArray(int[][]::new), fps);
            return new Range<>(range[0], range[1]);
        }
        var map = l.chars().get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        Size size = new Size(s.optInt("width"), s.optInt("height"));
        if (l.physicalId() == null && Catalog.has(l.chars(), CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_CONSTRAINED_HIGH_SPEED_VIDEO)
                && Arrays.asList(map.getHighSpeedVideoSizes()).contains(size)) {
            for (Range<Integer> range : map.getHighSpeedVideoFpsRangesFor(size))
                if (range.getLower() == fps && range.getUpper() == fps) return range;
        }
        throw new IllegalArgumentException("Frame rate is unavailable for this resolution/lens");
    }

    void validate(JSONObject s, Catalog.Lens l) throws Exception {
        var c = l.chars();
        Set<String> allowed = Set.of("camera", "codec", "width", "height", "fps", "bitrate", "manual", "iso", "exposureNs",
                "focusAuto", "focus", "zoom", "ev", "awb", "ois", "stabilization", "torch", "aeLock", "awbLock", "gains",
                "outputWidth", "outputHeight", "outputMode", "phoneRotation", "phoneMirror", "stretchX", "stretchY",
                "distortion", "bulge", "bulgeRadius", "bulgeX", "bulgeY", "backgroundBlur", "maskFps", "mlDelegate",
                "noiseReduction", "edgeMode", "aberrationMode", "lensCorrection");
        for (Iterator<String> keys = s.keys(); keys.hasNext();) {
            if (!allowed.contains(keys.next())) throw new IllegalArgumentException("Unknown camera control");
        }
        int w = s.getInt("width"), h = s.getInt("height"), fps = s.getInt("fps");
        if (!Arrays.asList(catalog.sizes(l)).contains(new Size(w, h))) throw new IllegalArgumentException("Resolution is not exposed by this lens");
        fpsRange(s, l);
        PhoneProcessor.Options.read(s);
        int encodedWidth = PhoneProcessor.outputWidth(s), encodedHeight = PhoneProcessor.outputHeight(s);
        Limits.checked("Output frame allocation", (double) encodedWidth * encodedHeight * 4, 0, 160 * 1024 * 1024);
        if (PhoneProcessor.needed(s)) {
            if (highSpeed(s,l)) throw new IllegalArgumentException("Phone GPU effects require a regular camera session; disable effects for high-speed capture");
            Size[] sizes = c.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP).getOutputSizes(android.graphics.SurfaceTexture.class);
            if (sizes == null || !Arrays.asList(sizes).contains(new Size(w,h))) throw new IllegalArgumentException("This sensor mode does not expose a GPU texture surface");
        }
        StreamConfigurationMap map = c.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        long duration = map.getOutputMinFrameDuration(MediaCodec.class, new Size(w, h));
        if (!highSpeed(s, l) && duration > 0 && fps > 1_000_000_000.0 / duration + 0.01) throw new IllegalArgumentException("This resolution cannot sustain the requested frame rate");
        if (highSpeed(s, l) && (s.optBoolean("manual") || !s.optBoolean("focusAuto", true) || s.optInt("awb", 1) != 1))
            throw new IllegalArgumentException("High-speed sessions require automatic exposure, focus and white balance");
        String name = s.getString("codec");
        JSONObject codec = null;
        JSONArray codecs = catalog.export().getJSONArray("codecs");
        for (int i = 0; i < codecs.length(); i++) if (codecs.getJSONObject(i).getString("name").equals(name)) codec = codecs.getJSONObject(i);
        if (codec == null) throw new IllegalArgumentException("Select an exposed hardware encoder");
        MediaCodecInfo info = Arrays.stream(new MediaCodecList(MediaCodecList.REGULAR_CODECS).getCodecInfos())
                .filter(x -> x.getName().equals(name)).findFirst().orElseThrow();
        var caps = info.getCapabilitiesForType(codec.getString("mime")).getVideoCapabilities();
        if (!caps.areSizeAndRateSupported(encodedWidth, encodedHeight, fps)) throw new IllegalArgumentException("Codec does not support this output size/frame rate combination");
        Limits.checked("Bitrate", s.getInt("bitrate"), caps.getBitrateRange().getLower(), caps.getBitrateRange().getUpper());
        boolean manual = s.optBoolean("manual");
        if (manual) {
            if (!catalog.describe(l).getBoolean("manualSensor")) throw new IllegalArgumentException("Manual sensor controls are unavailable");
            var iso = c.get(CameraCharacteristics.SENSOR_INFO_SENSITIVITY_RANGE);
            var exposure = c.get(CameraCharacteristics.SENSOR_INFO_EXPOSURE_TIME_RANGE);
            Limits.checked("ISO", s.getInt("iso"), iso.getLower(), iso.getUpper());
            long maxFrame = c.get(CameraCharacteristics.SENSOR_INFO_MAX_FRAME_DURATION);
            Limits.checked("Exposure", s.getLong("exposureNs"), exposure.getLower(), Math.min(exposure.getUpper(), Math.min(maxFrame, 1_000_000_000L / fps)));
        }
        if (!s.optBoolean("focusAuto", true)) {
            if (!catalog.describe(l).getBoolean("manualFocus")) throw new IllegalArgumentException("Manual focus is unavailable");
            Limits.checked("Focus", s.getDouble("focus"), 0, c.get(CameraCharacteristics.LENS_INFO_MINIMUM_FOCUS_DISTANCE));
        }
        Range<Float> z = Build.VERSION.SDK_INT >= 30 ? c.get(CameraCharacteristics.CONTROL_ZOOM_RATIO_RANGE) : null;
        Limits.checked("Zoom", s.optDouble("zoom", 1), z == null ? 1 : z.getLower(),
                z == null ? c.get(CameraCharacteristics.SCALER_AVAILABLE_MAX_DIGITAL_ZOOM) : z.getUpper());
        var ev = c.get(CameraCharacteristics.CONTROL_AE_COMPENSATION_RANGE);
        Limits.checked("Exposure compensation", s.optInt("ev", 0), ev.getLower(), ev.getUpper());
        int awb = s.optInt("awb", CameraMetadata.CONTROL_AWB_MODE_AUTO);
        if (!Catalog.mode(c.get(CameraCharacteristics.CONTROL_AWB_AVAILABLE_MODES), awb)) throw new IllegalArgumentException("White balance mode is unavailable");
        if (awb == CameraMetadata.CONTROL_AWB_MODE_OFF) {
            if (!Catalog.has(c, CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_MANUAL_POST_PROCESSING)) throw new IllegalArgumentException("Manual white balance is unavailable");
            JSONArray gains = s.getJSONArray("gains");
            if (gains.length() != 4) throw new IllegalArgumentException("White balance needs R, G-even, G-odd and B gains");
            for (int i = 0; i < 4; i++) Limits.checked("Color gain", gains.getDouble(i), 1, 8);
            if (autoColorTransform == null || !l.id().equals(colorTransformLens))
                throw new IllegalArgumentException("Run an automatic white-balance preview on this lens before setting manual gains");
        }
        int ois = s.optBoolean("ois") ? CameraMetadata.LENS_OPTICAL_STABILIZATION_MODE_ON : CameraMetadata.LENS_OPTICAL_STABILIZATION_MODE_OFF;
        int stab = s.optBoolean("stabilization") ? CameraMetadata.CONTROL_VIDEO_STABILIZATION_MODE_ON : CameraMetadata.CONTROL_VIDEO_STABILIZATION_MODE_OFF;
        if (ois != 0 && !Catalog.mode(c.get(CameraCharacteristics.LENS_INFO_AVAILABLE_OPTICAL_STABILIZATION), ois)) throw new IllegalArgumentException("OIS is unavailable");
        if (stab != 0 && !Catalog.mode(c.get(CameraCharacteristics.CONTROL_AVAILABLE_VIDEO_STABILIZATION_MODES), stab)) throw new IllegalArgumentException("Video stabilization is unavailable");
        if (ois != 0 && stab != 0) throw new IllegalArgumentException("Use optical OR video stabilization");
        if (s.optBoolean("torch") && !Boolean.TRUE.equals(c.get(CameraCharacteristics.FLASH_INFO_AVAILABLE))) throw new IllegalArgumentException("Flash is unavailable");
        if (s.optBoolean("aeLock") && !Boolean.TRUE.equals(c.get(CameraCharacteristics.CONTROL_AE_LOCK_AVAILABLE))) throw new IllegalArgumentException("AE lock is unavailable");
        if (s.optBoolean("awbLock") && !Boolean.TRUE.equals(c.get(CameraCharacteristics.CONTROL_AWB_LOCK_AVAILABLE))) throw new IllegalArgumentException("AWB lock is unavailable");
        validateMode(s,"noiseReduction",c.get(CameraCharacteristics.NOISE_REDUCTION_AVAILABLE_NOISE_REDUCTION_MODES));
        validateMode(s,"edgeMode",c.get(CameraCharacteristics.EDGE_AVAILABLE_EDGE_MODES));
        validateMode(s,"aberrationMode",c.get(CameraCharacteristics.COLOR_CORRECTION_AVAILABLE_ABERRATION_MODES));
        validateMode(s,"lensCorrection",c.get(CameraCharacteristics.DISTORTION_CORRECTION_AVAILABLE_MODES));
    }

    void validateMode(JSONObject s,String key,int[] modes) {
        if (s.has(key) && s.optInt(key,-1) != -1 && !Catalog.mode(modes,s.optInt(key))) throw new IllegalArgumentException(key+" is not exposed by this camera");
    }

    void configure(JSONObject settings) throws Exception {
        Catalog.Lens selected = catalog.lenses.get(settings.getString("camera"));
        if (selected == null) throw new IllegalArgumentException("Camera ID is unavailable");
        validate(settings, selected);
        stop();
        current = new JSONObject(settings.toString());
        lens = selected;
        int epoch = generation;
        try {
        int sensorW = current.getInt("width"), sensorH = current.getInt("height"), fps = current.getInt("fps");
        int w = PhoneProcessor.outputWidth(current), h = PhoneProcessor.outputHeight(current);
        encoder = MediaCodec.createByCodecName(current.getString("codec"));
        String mime = encoder.getCodecInfo().getSupportedTypes()[0];
        for (String type : encoder.getCodecInfo().getSupportedTypes()) if (type.equals("video/avc") || type.equals("video/hevc")) mime = type;
        MediaFormat format = MediaFormat.createVideoFormat(mime, w, h);
        format.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
        format.setInteger(MediaFormat.KEY_BIT_RATE, current.getInt("bitrate"));
        format.setInteger(MediaFormat.KEY_FRAME_RATE, fps);
        format.setFloat(MediaFormat.KEY_MAX_FPS_TO_ENCODER, fps);
        format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1);
        if (Build.VERSION.SDK_INT >= 29) format.setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0);
        // ponytail: codec-specific encoder latency knobs need per-device benchmarks; zero B-frames is the portable baseline.
        if (mime.equals("video/avc")) format.setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline);
        encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
        surface = encoder.createInputSurface();
        encoder.start();
        if (PhoneProcessor.needed(current)) {
            processor = new PhoneProcessor(context,surface,sensorW,sensorH,w,h,current,
                    e -> handler.post(() -> { if (epoch == generation) { stop(); fail(e); } }));
        }
        MediaCodec activeEncoder = encoder;
        String codecMime = mime;
        catalog.manager.openCamera(lens.openId(), new CameraDevice.StateCallback() {
            @Override public void onOpened(CameraDevice device) {
                if (epoch != generation) { device.close(); return; }
                camera = device;
                try {
                    OutputConfiguration output = new OutputConfiguration(processor == null ? surface : processor.surface());
                    if (lens.physicalId() != null) output.setPhysicalCameraId(lens.physicalId());
                    camera.createCaptureSession(new SessionConfiguration(highSpeed(current, lens) ? SessionConfiguration.SESSION_HIGH_SPEED : SessionConfiguration.SESSION_REGULAR, List.of(output),
                            handler::post, new CameraCaptureSession.StateCallback() {
                        @Override public void onConfigured(CameraCaptureSession value) {
                            if (epoch != generation) { value.close(); return; }
                            session = value;
                            try {
                                applyControls();
                                bridge.send(Json.object("type", "configured", "width", w, "height", h, "fps", fps, "mime", codecMime,
                                        "timestampsRealtime", Objects.equals(lens.chars().get(CameraCharacteristics.SENSOR_INFO_TIMESTAMP_SOURCE), CameraMetadata.SENSOR_INFO_TIMESTAMP_SOURCE_REALTIME), "phoneProcessed",processor != null,"settings", current));
                                bridge.send(processor == null ? Json.object("type","processing","mode","Camera → encoder") : processingStats());
                                new Thread(() -> drain(activeEncoder, epoch), "opencam-encoder").start();
                                bridge.listener.status("Streaming " + w + " × " + h + " · " + fps + " fps");
                            } catch (Exception e) { stop(); fail(e); }
                        }
                        @Override public void onConfigureFailed(CameraCaptureSession value) {
                            if (epoch == generation) { stop(); fail(new IOException("Camera rejected the stream combination; try a smaller resolution")); }
                        }
                    }));
                } catch (Exception e) { stop(); fail(e); }
            }
            @Override public void onDisconnected(CameraDevice device) { device.close(); if (epoch == generation) { stop(); fail(new IOException("Camera disconnected")); } }
            @Override public void onError(CameraDevice device, int error) { device.close(); if (epoch == generation) { stop(); fail(new IOException("Camera open failed (" + error + ")")); } }
        }, handler);
        } catch (Exception e) { stop(); throw e; }
    }

    void controls(JSONObject settings) throws Exception {
        if (current == null) throw new IllegalStateException("Start a stream before adjusting controls");
        JSONObject merged = new JSONObject(current.toString());
        Set<String> streamKeys = Set.of("camera", "codec", "width", "height", "fps", "bitrate", "outputWidth", "outputHeight", "phoneRotation", "phoneMirror", "outputMode");
        for (Iterator<String> keys = settings.keys(); keys.hasNext();) {
            String key = keys.next();
            if (streamKeys.contains(key)) throw new IllegalArgumentException("Stream changes require configure");
            merged.put(key, settings.get(key));
        }
        validate(merged, lens);
        if (PhoneProcessor.needed(merged) != (processor != null) || !merged.optString("mlDelegate","auto").equals(current.optString("mlDelegate","auto"))) {
            configure(merged); return;
        }
        JSONObject previous = current;
        current = merged;
        try { if (session != null) applyControls(); if (processor != null) processor.update(current); }
        catch (Exception e) { current = previous; throw e; }
        bridge.send(Json.object("type", "controls", "settings", current));
    }

    <T> void set(CaptureRequest.Builder request, CaptureRequest.Key<T> key, T value) throws CameraAccessException {
        if (lens.physicalId() != null && catalog.writable(lens, key)) request.setPhysicalCameraKey(key, value, lens.physicalId());
        else if (catalog.manager.getCameraCharacteristics(lens.openId()).getAvailableCaptureRequestKeys().contains(key)) request.set(key, value);
    }

    void fill(CaptureRequest.Builder b) throws Exception {
        JSONObject s = current;
        var c = lens.chars();
        boolean manual = s.optBoolean("manual");
        set(b, CaptureRequest.CONTROL_MODE, CameraMetadata.CONTROL_MODE_AUTO);
        set(b, CaptureRequest.CONTROL_AE_MODE, manual ? CameraMetadata.CONTROL_AE_MODE_OFF : CameraMetadata.CONTROL_AE_MODE_ON);
        int fps = s.getInt("fps");
        set(b, CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, fpsRange(s, lens));
        if (manual) {
            set(b, CaptureRequest.SENSOR_SENSITIVITY, s.getInt("iso"));
            set(b, CaptureRequest.SENSOR_EXPOSURE_TIME, s.getLong("exposureNs"));
            set(b, CaptureRequest.SENSOR_FRAME_DURATION, 1_000_000_000L / fps);
        }
        set(b, CaptureRequest.CONTROL_AE_EXPOSURE_COMPENSATION, s.optInt("ev", 0));
        set(b, CaptureRequest.CONTROL_AE_LOCK, s.optBoolean("aeLock"));
        set(b, CaptureRequest.CONTROL_AWB_LOCK, s.optBoolean("awbLock"));
        boolean autofocus = s.optBoolean("focusAuto", true);
        int af = Catalog.mode(c.get(CameraCharacteristics.CONTROL_AF_AVAILABLE_MODES), CameraMetadata.CONTROL_AF_MODE_CONTINUOUS_VIDEO)
                ? CameraMetadata.CONTROL_AF_MODE_CONTINUOUS_VIDEO : CameraMetadata.CONTROL_AF_MODE_OFF;
        set(b, CaptureRequest.CONTROL_AF_MODE, autofocus ? af : CameraMetadata.CONTROL_AF_MODE_OFF);
        if (!autofocus) set(b, CaptureRequest.LENS_FOCUS_DISTANCE, (float) s.getDouble("focus"));
        int awb = s.optInt("awb", CameraMetadata.CONTROL_AWB_MODE_AUTO);
        set(b, CaptureRequest.CONTROL_AWB_MODE, awb);
        if (awb == CameraMetadata.CONTROL_AWB_MODE_OFF) {
            JSONArray gains = s.getJSONArray("gains");
            set(b, CaptureRequest.COLOR_CORRECTION_MODE, CameraMetadata.COLOR_CORRECTION_MODE_TRANSFORM_MATRIX);
            set(b, CaptureRequest.COLOR_CORRECTION_GAINS, new RggbChannelVector((float) gains.getDouble(0), (float) gains.getDouble(1), (float) gains.getDouble(2), (float) gains.getDouble(3)));
            set(b, CaptureRequest.COLOR_CORRECTION_TRANSFORM, autoColorTransform);
        }
        if (Build.VERSION.SDK_INT >= 30 && c.get(CameraCharacteristics.CONTROL_ZOOM_RATIO_RANGE) != null)
            set(b, CaptureRequest.CONTROL_ZOOM_RATIO, (float) s.optDouble("zoom", 1));
        else {
            Rect rect = c.get(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE);
            float zoom = (float) s.optDouble("zoom", 1);
            int w = Math.round(rect.width() / zoom), h = Math.round(rect.height() / zoom);
            set(b, CaptureRequest.SCALER_CROP_REGION, new Rect(rect.centerX() - w/2, rect.centerY() - h/2, rect.centerX() + w/2, rect.centerY() + h/2));
        }
        set(b, CaptureRequest.LENS_OPTICAL_STABILIZATION_MODE, s.optBoolean("ois") ? CameraMetadata.LENS_OPTICAL_STABILIZATION_MODE_ON : CameraMetadata.LENS_OPTICAL_STABILIZATION_MODE_OFF);
        set(b, CaptureRequest.CONTROL_VIDEO_STABILIZATION_MODE, s.optBoolean("stabilization") ? CameraMetadata.CONTROL_VIDEO_STABILIZATION_MODE_ON : CameraMetadata.CONTROL_VIDEO_STABILIZATION_MODE_OFF);
        set(b, CaptureRequest.FLASH_MODE, s.optBoolean("torch") ? CameraMetadata.FLASH_MODE_TORCH : CameraMetadata.FLASH_MODE_OFF);
        if (s.optInt("noiseReduction",-1)>=0) set(b,CaptureRequest.NOISE_REDUCTION_MODE,s.optInt("noiseReduction"));
        if (s.optInt("edgeMode",-1)>=0) set(b,CaptureRequest.EDGE_MODE,s.optInt("edgeMode"));
        if (s.optInt("aberrationMode",-1)>=0) set(b,CaptureRequest.COLOR_CORRECTION_ABERRATION_MODE,s.optInt("aberrationMode"));
        if (s.optInt("lensCorrection",-1)>=0) set(b,CaptureRequest.DISTORTION_CORRECTION_MODE,s.optInt("lensCorrection"));
    }

    void applyControls() throws Exception {
        CaptureRequest.Builder b = camera.createCaptureRequest(CameraDevice.TEMPLATE_RECORD);
        b.addTarget(processor == null ? surface : processor.surface());
        fill(b);
        CameraCaptureSession.CaptureCallback callback = new CameraCaptureSession.CaptureCallback() {
            @Override public void onCaptureCompleted(CameraCaptureSession s, CaptureRequest r, TotalCaptureResult result) {
                if (current.optInt("awb", 1) != 0) {
                    CaptureResult color = lens.physicalId() == null ? result : result.getPhysicalCameraResults().get(lens.physicalId());
                    if (color != null && color.get(CaptureResult.COLOR_CORRECTION_TRANSFORM) != null) {
                        autoColorTransform = color.get(CaptureResult.COLOR_CORRECTION_TRANSFORM);
                        colorTransformLens = lens.id();
                    }
                }
                if (SystemClock.elapsedRealtime() - lastMetadata < 1000) return;
                lastMetadata = SystemClock.elapsedRealtime();
                JSONObject metadata = new JSONObject();
                try {
                    for (CaptureResult.Key<?> key : result.getKeys()) metadata.put(key.getName(), Json.encode(result.get(key)));
                    JSONObject physical = new JSONObject();
                    for (var entry : result.getPhysicalCameraResults().entrySet()) {
                        JSONObject values = new JSONObject();
                        for (CaptureResult.Key<?> key : entry.getValue().getKeys()) values.put(key.getName(), Json.encode(entry.getValue().get(key)));
                        physical.put(entry.getKey(), values);
                    }
                    metadata.put("physicalCameras", physical);
                    bridge.send(Json.object("type", "metadata", "values", metadata));
                    if (processor != null) bridge.send(processingStats());
                } catch (Exception e) { fail(e); }
            }
        };
        if (session instanceof CameraConstrainedHighSpeedCaptureSession fast) session.setRepeatingBurst(fast.createHighSpeedRequestList(b.build()), callback, handler);
        else session.setRepeatingRequest(b.build(), callback, handler);
    }

    JSONObject processingStats() {
        JSONObject stats = processor.stats();
        try { stats.put("type","processing"); } catch (JSONException impossible) { throw new IllegalStateException(impossible); }
        return stats;
    }

    void drain(MediaCodec codec, int epoch) {
        MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
        byte[] config = null;
        try {
            while (generation == epoch) {
                int index = codec.dequeueOutputBuffer(info, 10_000);
                if (index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                    MediaFormat format = codec.getOutputFormat();
                    for (String key : new String[]{"csd-0", "csd-1", "csd-2"}) {
                        ByteBuffer buffer = format.getByteBuffer(key);
                        if (buffer == null) continue;
                        ByteBuffer copy = buffer.duplicate();
                        byte[] data = new byte[copy.remaining()]; copy.get(data); bytes.write(data);
                    }
                    config = bytes.toByteArray();
                } else if (index >= 0) {
                    ByteBuffer buffer = codec.getOutputBuffer(index);
                    if (buffer != null && info.size > 0) {
                        buffer.position(info.offset); buffer.limit(info.offset + info.size);
                        byte[] data = new byte[info.size]; buffer.get(data);
                        if ((info.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0) config = data;
                        else bridge.video(data, info.presentationTimeUs, info.flags, config);
                    }
                    codec.releaseOutputBuffer(index, false);
                }
            }
        } catch (Exception e) { if (generation == epoch) handler.post(() -> { stop(); fail(e); }); }
    }

    void requestSync() {
        if (encoder == null) return;
        try { Bundle parameters = new Bundle(); parameters.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0); encoder.setParameters(parameters); }
        catch (IllegalStateException ignored) { }
    }

    void raw() throws Exception {
        if (current == null || lens == null) throw new IllegalStateException("Select a lens and start the stream first");
        if (!Catalog.has(lens.chars(), CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_RAW)) throw new IllegalArgumentException("RAW_SENSOR is unavailable for this lens");
        if (Build.VERSION.SDK_INT < 29) throw new IllegalArgumentException("RAW saving requires Android 10 or later");
        stop();
        var regular = lens.chars().get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES);
        current.put("fps", Limits.previewFps(Arrays.stream(regular).map(r -> new int[]{r.getLower(), r.getUpper()}).toArray(int[][]::new)));
        int epoch = generation;
        Size[] sizes = lens.chars().get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP).getOutputSizes(ImageFormat.RAW_SENSOR);
        if (sizes == null || sizes.length == 0) throw new IllegalArgumentException("No RAW sizes exposed");
        Size size = Arrays.stream(sizes).max(Comparator.comparingLong(s -> (long) s.getWidth() * s.getHeight())).orElseThrow();
        rawReader = ImageReader.newInstance(size.getWidth(), size.getHeight(), ImageFormat.RAW_SENSOR, 2);
        rawReader.setOnImageAvailableListener(reader -> {
            android.media.Image image = reader.acquireNextImage();
            if (epoch != generation) { if (image != null) image.close(); return; }
            if (rawImage != null) rawImage.close();
            rawImage = image;
            saveRaw(epoch);
        }, handler);
        bridge.send(Json.object("type", "stopped", "message", "RAW capture pauses the stream; start again when complete"));
        catalog.manager.openCamera(lens.openId(), new CameraDevice.StateCallback() {
            @Override public void onOpened(CameraDevice device) {
                if (epoch != generation) { device.close(); return; }
                camera = device;
                try {
                    OutputConfiguration out = new OutputConfiguration(rawReader.getSurface());
                    if (lens.physicalId() != null) out.setPhysicalCameraId(lens.physicalId());
                    camera.createCaptureSession(new SessionConfiguration(SessionConfiguration.SESSION_REGULAR, List.of(out), handler::post, new CameraCaptureSession.StateCallback() {
                        @Override public void onConfigured(CameraCaptureSession value) {
                            if (epoch != generation) { value.close(); return; }
                            session = value;
                            try {
                                CaptureRequest.Builder b = camera.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE);
                                fill(b); b.addTarget(rawReader.getSurface());
                                session.capture(b.build(), new CameraCaptureSession.CaptureCallback() {
                                    @Override public void onCaptureCompleted(CameraCaptureSession s, CaptureRequest r, TotalCaptureResult result) { rawResult = result; saveRaw(epoch); }
                                    @Override public void onCaptureFailed(CameraCaptureSession s, CaptureRequest r, CaptureFailure failure) { stop(); fail(new IOException("RAW capture failed")); }
                                }, handler);
                                handler.postDelayed(() -> { if (epoch == generation) { stop(); fail(new IOException("RAW capture timed out")); } }, 15_000);
                            } catch (Exception e) { stop(); fail(e); }
                        }
                        @Override public void onConfigureFailed(CameraCaptureSession value) { if (epoch == generation) { stop(); fail(new IOException("This camera rejects RAW-only capture")); } }
                    }));
                } catch (Exception e) { stop(); fail(e); }
            }
            @Override public void onDisconnected(CameraDevice device) { device.close(); if (epoch == generation) stop(); }
            @Override public void onError(CameraDevice device, int error) { device.close(); if (epoch == generation) { stop(); fail(new IOException("RAW camera failed: " + error)); } }
        }, handler);
    }

    void saveRaw(int epoch) {
        if (epoch != generation || rawImage == null || rawResult == null) return;
        android.net.Uri uri = null;
        try {
            CaptureResult result = rawResult;
            if (lens.physicalId() != null) {
                result = rawResult.getPhysicalCameraResults().get(lens.physicalId());
                if (result == null) throw new IOException("Physical RAW metadata was not returned");
            }
            ContentValues values = new ContentValues();
            values.put(MediaStore.Images.Media.DISPLAY_NAME, "OpenCam-" + System.currentTimeMillis() + ".dng");
            values.put(MediaStore.Images.Media.MIME_TYPE, "image/x-adobe-dng");
            values.put(MediaStore.Images.Media.RELATIVE_PATH, "DCIM/OpenCam");
            values.put(MediaStore.Images.Media.IS_PENDING, 1);
            uri = context.getContentResolver().insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values);
            if (uri == null) throw new IOException("Could not create DNG file");
            try (OutputStream out = context.getContentResolver().openOutputStream(uri);
                    DngCreator dng = new DngCreator(lens.chars(), result)) {
                if (out == null) throw new IOException("Could not open DNG file");
                dng.writeImage(out, rawImage);
            }
            values.clear(); values.put(MediaStore.Images.Media.IS_PENDING, 0);
            context.getContentResolver().update(uri, values, null, null);
            bridge.send(Json.object("type", "raw_saved", "uri", uri.toString(), "message", "DNG saved to DCIM/OpenCam on your phone"));
        } catch (Exception e) {
            if (uri != null) context.getContentResolver().delete(uri, null, null);
            fail(e);
        } finally { stop(); }
    }

    void stop() {
        generation++;
        bridge.resetVideo();
        if (session != null) { session.close(); session = null; }
        if (camera != null) { camera.close(); camera = null; }
        if (processor != null) { processor.close(); processor = null; }
        if (encoder != null) {
            try { encoder.stop(); } catch (IllegalStateException ignored) { }
            encoder.release(); encoder = null;
        }
        if (surface != null) { surface.release(); surface = null; }
        if (rawImage != null) { rawImage.close(); rawImage = null; }
        rawResult = null;
        if (rawReader != null) { rawReader.close(); rawReader = null; }
    }

    @Override public void close() { handler.post(() -> { stop(); thread.quitSafely(); }); }
}
