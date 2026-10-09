package dev.opencam;

import android.media.Image;
import java.nio.ByteBuffer;
import org.json.*;

final class WirePixels {
    static final String YUV = "opencam.i420", RGBA = "opencam.rgba";
    static boolean uncompressed(String codec) { return codec.equals(YUV) || codec.equals(RGBA); }
    static String mime(String codec) { return codec.equals(YUV) ? "video/x-opencam-i420" : "video/x-opencam-rgba"; }
    static int bytes(int w,int h,boolean rgba) {
        if (w<2 || h<2 || w>8192 || h>8192 || (w&1)!=0 || (h&1)!=0) throw new IllegalArgumentException("Invalid uncompressed dimensions");
        return (int) Limits.checked("Uncompressed frame allocation", (double)w*h*(rgba ? 4 : 1.5), 1, 160*1024*1024);
    }
    static void plane(ByteBuffer source,int rowStride,int pixelStride,int width,int height,int sample,byte[] target,int offset) {
        ByteBuffer input=source.duplicate(); int start=input.position();
        if (rowStride<width*pixelStride || pixelStride<sample || start+(long)(height-1)*rowStride+(long)(width-1)*pixelStride+sample>input.limit()) throw new IllegalArgumentException("Camera plane stride/buffer mismatch");
        for (int y=0;y<height;y++) {
            int row=start+y*rowStride;
            if (pixelStride==sample) { input.position(row);input.get(target,offset,width*sample);offset+=width*sample; }
            else for(int x=0;x<width;x++) for(int b=0;b<sample;b++) target[offset++]=input.get(row+x*pixelStride+b);
        }
    }
    static byte[] pack(Image image,boolean rgba) {
        int w=image.getWidth(),h=image.getHeight(); byte[] data=new byte[bytes(w,h,rgba)]; Image.Plane[] planes=image.getPlanes();
        if (rgba) { if(planes.length!=1)throw new IllegalArgumentException("RGBA surface returned unexpected planes");plane(planes[0].getBuffer(),planes[0].getRowStride(),planes[0].getPixelStride(),w,h,4,data,0); }
        else {
            if(planes.length!=3)throw new IllegalArgumentException("YUV surface returned unexpected planes");
            int offset=0;for(int i=0;i<3;i++){int pw=i==0?w:w/2,ph=i==0?h:h/2;plane(planes[i].getBuffer(),planes[i].getRowStride(),planes[i].getPixelStride(),pw,ph,1,data,offset);offset+=pw*ph;}
        }
        return data;
    }
    static JSONObject camera(JSONObject camera,String codec) throws JSONException {
        if (!uncompressed(codec) && !camera.has("encodedSizes")) return camera;
        JSONObject mode=CaptureSettings.copy(camera);String prefix=codec.equals(YUV)?"yuv":codec.equals(RGBA)?"rgba":"encoded";
        mode.put("sizes",camera.optJSONArray(prefix+"Sizes")).put("frameDurations",camera.optJSONArray(prefix+"FrameDurations"));mode.put("highSpeed",uncompressed(codec)?new JSONArray():camera.optJSONArray("encodedHighSpeed"));return mode;
    }
}
