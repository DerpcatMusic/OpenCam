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
            checkShaders(); checkModel(); checkValidation();
            result.putString("stream","GLES3 surface transforms, blur passes, CPU segmentation and validation passed");
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
                PhoneProcessor processor=new PhoneProcessor(getTargetContext(),reader.getSurface(),320,240,160,120,settings,error::set)) {
            byte[] original=frame(processor,reader,error);
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
}
