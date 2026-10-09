package dev.opencam;

import android.app.Activity;
import android.app.Instrumentation;
import android.graphics.*;
import android.media.Image;
import android.media.ImageReader;
import android.os.Bundle;
import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

public final class ProcessingTest extends Instrumentation {
    final JSONObject checks=Json.object("device",android.os.Build.MODEL,"syntheticInput",true);
    @Override public void onCreate(Bundle args) { super.onCreate(args); start(); }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            checkShaders(); checkModel(); checkValidation(); checkSettings(); checkCameraSync();
            result.putString("stream","GLES3 encoder/preview parity, blur, segmentation, native lens switching, phone/desktop sync, revisions and validation passed");
            result.putString("checks",checks.toString());
            finish(Activity.RESULT_OK,result);
        } catch (Throwable e) {
            result.putString("stream","FAIL: "+android.util.Log.getStackTraceString(e));
            finish(Activity.RESULT_CANCELED,result);
        }
    }
    void require(boolean condition,String message) { if (!condition) throw new AssertionError(message); }
    void pattern(PhoneProcessor processor) {
        Canvas canvas=processor.surface().lockCanvas(null);
        try {
            canvas.drawColor(Color.DKGRAY); Paint paint=new Paint();
            for (int x=0;x<320;x+=20) {
                paint.setColor((x/20)%2==0 ? Color.WHITE:Color.BLACK); canvas.drawRect(x,0,x+10,240,paint);
            }
            paint.setColor(Color.RED); canvas.drawCircle(130,100,38,paint);
            paint.setColor(Color.GREEN); canvas.drawRect(15,15,70,60,paint);
        } finally { processor.surface().unlockCanvasAndPost(canvas); }
    }
    byte[] frame(PhoneProcessor processor,ImageReader reader,AtomicReference<Exception> error) throws Exception {
        try (Image stale=reader.acquireLatestImage()) { }
        pattern(processor);
        for (int i=0;i<80;i++) {
            if (error.get()!=null) throw error.get();
            try (Image image=reader.acquireLatestImage()) {
                if (image!=null) {
                    require(image.getWidth()==160 && image.getHeight()==120,"GPU output dimensions");
                    Image.Plane plane=image.getPlanes()[0]; ByteBuffer pixels=plane.getBuffer();
                    byte[] rgba=new byte[160*120*4];
                    for (int y=0;y<120;y++) for (int x=0;x<160;x++) for (int c=0;c<4;c++)
                        rgba[(y*160+x)*4+c]=pixels.get(y*plane.getRowStride()+x*plane.getPixelStride()+c);
                    return rgba;
                }
            }
            Thread.sleep(25);
        }
        throw new AssertionError("GPU did not deliver a frame");
    }
    void checkShaders() throws Exception {
        AtomicReference<Exception> error=new AtomicReference<>();
        JSONObject settings=Json.object("width",320,"height",240,"outputWidth",160,"outputHeight",120,"mlDelegate","cpu");
        try (ImageReader reader=ImageReader.newInstance(160,120,PixelFormat.RGBA_8888,2);
                ImageReader preview=ImageReader.newInstance(160,120,PixelFormat.RGBA_8888,2);
                PhoneProcessor processor=new PhoneProcessor(getTargetContext(),reader.getSurface(),320,240,160,120,settings,error::set)) {
            processor.preview(preview.getSurface());
            byte[] original=frame(processor,reader,error);
            byte[] displayed=frame(processor,preview,error);
            require(java.util.Arrays.equals(original,displayed), "Preview and encoder GPU pixels differ");
            settings.put("stretchX",1.8).put("distortion",.35).put("bulge",.4);
            processor.update(settings); byte[] warped=frame(processor,reader,error);
            int changed=0, nonBlack=0;
            for (int i=0;i<original.length;i+=4) {
                if (original[i]!=warped[i] || original[i+1]!=warped[i+1] || original[i+2]!=warped[i+2]) changed++;
                if ((original[i]&255)+(original[i+1]&255)+(original[i+2]&255)>20) nonBlack++;
            }
            require(nonBlack>1000,"Texture shader rendered black"); require(changed>1500,"Warp controls did not change pixels");
            checks.put("warpChangedPixels",changed).put("outputWidth",160).put("outputHeight",120);
            settings.put("backgroundBlur",12); processor.update(settings);
            byte[] blurred=frame(processor,reader,error);
            require(!java.util.Arrays.equals(warped,blurred),"Blur passes did not change pixels");
            for (int i=3;i<blurred.length;i+=4) require((blurred[i]&255)==255,"Output alpha must be opaque");
            checks.put("gpu",processor.stats());
        }
    }
    void checkModel() throws Exception {
        AtomicReference<Exception> error=new AtomicReference<>();
        try (BackgroundSegmenter segmenter=new BackgroundSegmenter(getTargetContext(),"auto",error::set)) {
            ByteBuffer rgba=ByteBuffer.allocateDirect(256*144*4);
            for (int i=0;i<256*144;i++) rgba.putInt(0x000000ff);
            rgba.flip(); segmenter.submit(rgba,android.os.SystemClock.uptimeMillis());
            for (int i=0;i<200 && segmenter.mask==null && error.get()==null;i++) Thread.sleep(25);
            if (error.get()!=null) throw error.get();
            require(segmenter.mask!=null,"Bundled segmentation model did not return a mask");
            require(segmenter.mask.width()>0 && segmenter.mask.height()>0,"Invalid mask shape");
            require(segmenter.inferenceMs>0,"Missing inference measurement");
            require(segmenter.cpuMs>0,"CPU delegate benchmark missing");
            require(segmenter.delegate.equals("CPU") || segmenter.delegate.equals("GPU"),"Auto did not choose an available delegate");
            checks.put("segmentation",segmenter.stats());
        }
    }
    void checkValidation() throws Exception {
        JSONObject settings=Json.object("width",320,"height",240);
        require(!PhoneProcessor.needed(settings),"Effects-off path must bypass GPU");
        settings.put("stretchX",1.5); require(PhoneProcessor.needed(settings),"Stretch must enable GPU");
        for (JSONObject bad : new JSONObject[]{Json.object("stretchX",0),Json.object("distortion",2),Json.object("phoneRotation",45),Json.object("mlDelegate","cloud")}) {
            boolean rejected=false;
            try { PhoneProcessor.Options.read(bad); } catch (IllegalArgumentException expected) { rejected=true; }
            require(rejected,"Invalid processing controls were accepted");
        }
        settings.put("outputWidth",721);
        boolean rejected=false;
        try { PhoneProcessor.outputWidth(settings); } catch (IllegalArgumentException expected) { rejected=true; }
        require(rejected,"Odd output dimensions were accepted");
    }
    void checkSettings() throws Exception {
        JSONObject camera = new JSONObject("{\"fpsRanges\":[[10,20],[30,60]],\"frameDurations\":[{\"size\":[1280,720],\"minFrameNs\":25000000}]}");
        var fps = CaptureSettings.fps(camera, new org.json.JSONArray(new int[]{1280,720}));
        require(fps.contains(17) && fps.contains(40) && !fps.contains(25) && !fps.contains(41), "FPS must intersect sensor ranges and frame durations");
        require(!CaptureSettings.stale(Json.object("baseRevision", 7), 7), "Current revision rejected");
        require(CaptureSettings.stale(Json.object("baseRevision", 6), 7), "Stale desktop command accepted");
        require(!CaptureSettings.stale(Json.object("settings", new JSONObject()), 7), "Version 1 compatibility failed");
    }
    void await(java.util.function.BooleanSupplier condition, String failure) throws Exception {
        for (int i=0;i<200;i++) { if (condition.getAsBoolean()) return; Thread.sleep(25); }
        throw new AssertionError(failure);
    }
    void checkCameraSync() throws Exception {
        MainActivity activity = (MainActivity) startActivitySync(new android.content.Intent(getTargetContext(), MainActivity.class).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK));
        try {
            await(() -> activity.settings != null && activity.controller != null && activity.controller.session != null, "Native preview did not open");
            require(activity.controller.encoder == null, "Local preview must not run a video encoder");
            checkBridgeSync(activity);
            long first = activity.revision;
            int ev = activity.settings.optInt("ev");
            runOnMainSync(() -> activity.apply(Json.object("ev", ev), false));
            await(() -> activity.revision > first, "Phone change was not acknowledged");
            long second = activity.revision;
            activity.command(Json.object("type", "controls", "baseRevision", second, "settings", Json.object("ev", ev)));
            await(() -> activity.revision > second, "Desktop change did not reach the phone UI");
            long third = activity.revision;
            activity.command(Json.object("type", "controls", "baseRevision", first, "settings", Json.object("ev", ev + 1)));
            await(() -> activity.status.getText().toString().contains("other device"), "Stale command conflict was not shown");
            require(activity.revision == third && activity.settings.optInt("ev") == ev, "Stale command overwrote phone settings");
            activity.command(Json.object("type", "controls", "baseRevision", third, "settings", Json.object("zoom", 100000)));
            await(() -> activity.status.getText().toString().contains("Zoom"), "Invalid remote sensor control was not rejected");
            require(activity.revision == third, "Rejected control advanced the revision");
            String original = activity.settings.optString("camera");
            org.json.JSONArray cameras = activity.catalog.getJSONArray("cameras");
            if (cameras.length() > 1) {
                JSONObject next = cameras.getJSONObject(0);
                if (next.getString("id").equals(original)) next = cameras.getJSONObject(1);
                JSONObject selected = CaptureSettings.defaults(activity.catalog, next);
                runOnMainSync(() -> activity.apply(selected, true));
                await(() -> activity.settings.optString("camera").equals(selected.optString("camera")) && activity.controller.session != null, "Front/rear lens switch failed");
            }
            checks.put("cameraSync", Json.object("localRevision", first, "remoteRevision", third, "staleRejected", true, "previewWithoutEncoder", true, "cameraPaths", cameras.length()));
        } finally { runOnMainSync(activity::finish); }
    }

    void checkBridgeSync(MainActivity activity) throws Exception {
        runOnMainSync(activity::enable);
        require(activity.bridge != null, "Native connection did not open");
        javax.net.ssl.SSLContext tls = javax.net.ssl.SSLContext.getInstance("TLS");
        tls.init(null, new javax.net.ssl.TrustManager[]{new javax.net.ssl.X509TrustManager() {
            public java.security.cert.X509Certificate[] getAcceptedIssuers() { return new java.security.cert.X509Certificate[0]; }
            public void checkClientTrusted(java.security.cert.X509Certificate[] certs, String auth) { }
            public void checkServerTrusted(java.security.cert.X509Certificate[] certs, String auth) { }
        }}, null);
        try (javax.net.ssl.SSLSocket socket = (javax.net.ssl.SSLSocket) tls.getSocketFactory().createSocket("127.0.0.1", Bridge.PORT)) {
            socket.setSoTimeout(5000); socket.startHandshake();
            require(Bridge.hex(java.security.MessageDigest.getInstance("SHA-256").digest(socket.getSession().getPeerCertificates()[0].getEncoded())).equals(activity.bridge.fingerprint), "Test peer pin mismatch");
            var output = new java.io.DataOutputStream(socket.getOutputStream()); var input = new java.io.DataInputStream(socket.getInputStream());
            write(output, Json.object("type", "hello", "protocol", 1, "token", activity.bridge.token));
            JSONObject catalog = event(input, "capabilities");
            require(catalog.getJSONObject("settings").getString("camera").equals(activity.settings.getString("camera")), "Pairing discarded phone camera selection");
            long revision = catalog.getLong("revision");
            write(output, Json.object("type", "controls", "baseRevision", revision, "settings", Json.object("ev", 0)));
            JSONObject remote = event(input, "controls");
            await(() -> activity.revision == remote.optLong("revision"), "Wire controls did not sync phone UI");
            runOnMainSync(() -> activity.apply(Json.object("ev", 0), false));
            JSONObject local = event(input, "controls");
            require(local.getString("origin").equals("phone") && local.getLong("revision") > remote.getLong("revision"), "Phone change was not broadcast to desktop");
            write(output, Json.object("type", "controls", "baseRevision", revision, "settings", Json.object("ev", 1)));
            JSONObject stale = event(input, "state");
            require(stale.getBoolean("conflict") && stale.getJSONObject("settings").getInt("ev") == 0, "Stale wire command overwrote phone");
        } finally { runOnMainSync(activity::disable); }
        await(() -> activity.controller.session != null, "Disconnect did not restore local preview");
        checks.put("tlsControlSync", true);
    }
    void write(java.io.DataOutputStream output, JSONObject command) throws Exception {
        byte[] bytes = command.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
        output.writeByte(1); output.writeInt(bytes.length); output.write(bytes); output.flush();
    }
    JSONObject event(java.io.DataInputStream input, String type) throws Exception {
        for (int i=0;i<100;i++) {
            int kind=input.readUnsignedByte(), size=input.readInt(); require(size>0 && size<=Bridge.MAX_PACKET, "Invalid native packet size");
            byte[] bytes=new byte[size]; input.readFully(bytes); if (kind!=1) continue;
            JSONObject event=new JSONObject(new String(bytes,java.nio.charset.StandardCharsets.UTF_8));
            if (event.optString("type").equals(type)) return event;
            if (event.optString("type").equals("error")) throw new AssertionError(event.optString("message"));
        }
        throw new AssertionError("Missing event: " + type);
    }

}
