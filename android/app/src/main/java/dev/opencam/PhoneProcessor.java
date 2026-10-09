package dev.opencam;

import android.content.Context;
import android.graphics.SurfaceTexture;
import android.opengl.*;
import android.os.*;
import android.view.Surface;
import java.nio.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;
import org.json.JSONObject;

final class PhoneProcessor implements AutoCloseable {
    record Options(float stretchX, float stretchY, float distortion, float bulge, float radius,
                   float centerX, float centerY, float blur, int maskFps, String delegate,
                   int rotation, boolean mirror, int fit) {
        static Options read(JSONObject s) {
            float sx = value(s,"stretchX",1,.25,4), sy = value(s,"stretchY",1,.25,4);
            float distortion = value(s,"distortion",0,-.8,.8), bulge = value(s,"bulge",0,-.8,.8);
            float radius = value(s,"bulgeRadius",.4,.05,1), x = value(s,"bulgeX",.5,0,1), y = value(s,"bulgeY",.5,0,1);
            float blur = value(s,"backgroundBlur",0,0,32);
            int fps = (int) Limits.checked("Mask rate",s.optInt("maskFps",10),2,30);
            int rotation = s.optInt("phoneRotation",0), fit = s.optInt("outputMode",0);
            if (rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270) throw new IllegalArgumentException("Rotation must be 0, 90, 180 or 270");
            if (fit < 0 || fit > 2) throw new IllegalArgumentException("Use Fit, Crop or Stretch");
            String delegate = s.optString("mlDelegate","auto");
            if (!delegate.equals("auto") && !delegate.equals("cpu") && !delegate.equals("gpu")) throw new IllegalArgumentException("Unknown ML delegate");
            return new Options(sx,sy,distortion,bulge,radius,x,y,blur,fps,delegate,rotation,s.optBoolean("phoneMirror"),fit);
        }
        static float value(JSONObject s,String key,double fallback,double low,double high) {
            return (float) Limits.checked(key,s.optDouble(key,fallback),low,high);
        }
        boolean effects() { return stretchX != 1 || stretchY != 1 || distortion != 0 || bulge != 0 || blur > 0 || rotation != 0 || mirror; }
    }
    static int outputWidth(JSONObject s) { return dimension(s,"outputWidth",s.optInt(s.optInt("phoneRotation")%180 == 0 ? "width" : "height")); }
    static int outputHeight(JSONObject s) { return dimension(s,"outputHeight",s.optInt(s.optInt("phoneRotation")%180 == 0 ? "height" : "width")); }
    static int dimension(JSONObject s,String key,int fallback) {
        int v = s.optInt(key,0); if (v == 0) v = fallback;
        Limits.checked(key,v,2,8192);
        if ((v&1) != 0) throw new IllegalArgumentException("Output dimensions must be even");
        return v;
    }
    static boolean needed(JSONObject s) {
        Options o = Options.read(s);
        return o.effects() || outputWidth(s) != s.optInt("width") || outputHeight(s) != s.optInt("height");
    }

    final Context context;
    final int inputWidth,inputHeight,width,height;
    final Consumer<Exception> error;
    final HandlerThread thread = new HandlerThread("opencam-gpu");
    final Handler handler;
    final AtomicBoolean pending = new AtomicBoolean();
    final FloatBuffer quad = ByteBuffer.allocateDirect(32).order(ByteOrder.nativeOrder()).asFloatBuffer();
    final float[] textureMatrix = new float[16];
    final int[] buffers = new int[3], images = new int[3];
    final java.util.Map<String,Integer> uniforms = new java.util.HashMap<>();
    volatile Options options;
    volatile boolean closed;
    volatile double submitMs;
    volatile long renderedFrames;
    EGLDisplay display = EGL14.EGL_NO_DISPLAY;
    EGLContext eglContext = EGL14.EGL_NO_CONTEXT;
    EGLSurface eglSurface = EGL14.EGL_NO_SURFACE;
    EGLSurface previewSurface = EGL14.EGL_NO_SURFACE;
    EGLConfig configuration;
    SurfaceTexture cameraTexture;
    Surface cameraSurface;
    int cameraImage,maskImage,program,blurProgram;
    volatile BackgroundSegmenter segmenter;
    BackgroundSegmenter.Mask uploaded;
    long sampledMs;

    PhoneProcessor(Context context,Surface encoderSurface,int inputWidth,int inputHeight,int width,int height,
                   JSONObject settings,Consumer<Exception> error) throws Exception {
        this.context=context; this.inputWidth=inputWidth; this.inputHeight=inputHeight;
        this.width=width; this.height=height; this.error=error; options=Options.read(settings);
        quad.put(new float[]{-1,-1,1,-1,-1,1,1,1}).position(0);
        thread.start(); handler=new Handler(thread.getLooper());
        FutureTask<Void> init = new FutureTask<>(() -> { initialize(encoderSurface); return null; });
        handler.post(init);
        try { init.get(5,TimeUnit.SECONDS); }
        catch (Exception e) { close(); throw new IllegalStateException("Phone GPU could not start",e); }
    }
    Surface surface() { return cameraSurface; }
    void update(JSONObject settings) { options=Options.read(settings); }
    void preview(Surface target) {
        handler.post(() -> {
            try {
                if (previewSurface != EGL14.EGL_NO_SURFACE) EGL14.eglDestroySurface(display, previewSurface);
                previewSurface = EGL14.eglCreateWindowSurface(display, configuration, target, new int[]{EGL14.EGL_NONE}, 0);
                check(previewSurface != EGL14.EGL_NO_SURFACE, "Create preview surface");
            } catch (Exception e) { error.accept(e); }
        });
    }

    void initialize(Surface target) throws Exception {
        display=EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY);
        int[] version=new int[2];
        check(EGL14.eglInitialize(display,version,0,version,1),"Initialize EGL");
        int[] attributes={EGL14.EGL_RED_SIZE,8,EGL14.EGL_GREEN_SIZE,8,EGL14.EGL_BLUE_SIZE,8,EGL14.EGL_ALPHA_SIZE,8,
            EGL14.EGL_RENDERABLE_TYPE,0x0040,0x3142,1,EGL14.EGL_NONE};
        EGLConfig[] configs=new EGLConfig[1]; int[] count=new int[1];
        check(EGL14.eglChooseConfig(display,attributes,0,configs,0,1,count,0) && count[0]>0,"Find recordable GLES3 configuration");
        configuration = configs[0];
        eglContext=EGL14.eglCreateContext(display,configs[0],EGL14.EGL_NO_CONTEXT,new int[]{EGL14.EGL_CONTEXT_CLIENT_VERSION,3,EGL14.EGL_NONE},0);
        eglSurface=EGL14.eglCreateWindowSurface(display,configs[0],target,new int[]{EGL14.EGL_NONE},0);
        check(EGL14.eglMakeCurrent(display,eglSurface,eglSurface,eglContext),"Bind encoder surface");
        int[] maximum=new int[1]; GLES30.glGetIntegerv(GLES30.GL_MAX_TEXTURE_SIZE,maximum,0);
        if (width>maximum[0] || height>maximum[0]) throw new IllegalArgumentException("Output exceeds this phone GPU's texture limit");
        program=program("camera.frag"); blurProgram=program("blur.frag");
        for (String name : new String[]{"cameraTexture","blurredTexture","personMask","textureMatrix","aspectScale","stretch","center","distortion","bulge","radius","blurEnabled","maskReady","rotation","mirror","analysis"}) uniforms.put(name,GLES30.glGetUniformLocation(program,name));
        for (String name : new String[]{"image","stepSize"}) uniforms.put(name,GLES30.glGetUniformLocation(blurProgram,name));
        cameraImage=texture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES);
        maskImage=texture(GLES30.GL_TEXTURE_2D);
        GLES30.glTexImage2D(GLES30.GL_TEXTURE_2D,0,GLES30.GL_R8,1,1,0,GLES30.GL_RED,GLES30.GL_UNSIGNED_BYTE,ByteBuffer.allocateDirect(1));
        cameraTexture=new SurfaceTexture(cameraImage); cameraTexture.setDefaultBufferSize(inputWidth,inputHeight);
        cameraSurface=new Surface(cameraTexture);
        cameraTexture.setOnFrameAvailableListener(value -> {
            if (!closed && pending.compareAndSet(false,true)) handler.post(() -> {
                pending.set(false);
                if (!closed) try { render(); } catch (Exception e) { error.accept(e); }
            });
        },handler);
    }

    int texture(int target) {
        int[] id=new int[1]; GLES30.glGenTextures(1,id,0); GLES30.glBindTexture(target,id[0]);
        GLES30.glTexParameteri(target,GLES30.GL_TEXTURE_MIN_FILTER,GLES30.GL_LINEAR);
        GLES30.glTexParameteri(target,GLES30.GL_TEXTURE_MAG_FILTER,GLES30.GL_LINEAR);
        GLES30.glTexParameteri(target,GLES30.GL_TEXTURE_WRAP_S,GLES30.GL_CLAMP_TO_EDGE);
        GLES30.glTexParameteri(target,GLES30.GL_TEXTURE_WRAP_T,GLES30.GL_CLAMP_TO_EDGE);
        return id[0];
    }
    int program(String fragment) throws Exception {
        String vertex="#version 300 es\nlayout(location=0) in vec2 position; out vec2 uv; void main(){gl_Position=vec4(position,0,1);uv=(position+1.0)*0.5;}";
        String source;
        try (var stream=context.getAssets().open(fragment)) { source=new String(stream.readAllBytes(),java.nio.charset.StandardCharsets.UTF_8); }
        int v=shader(GLES30.GL_VERTEX_SHADER,vertex), f=shader(GLES30.GL_FRAGMENT_SHADER,source), p=GLES30.glCreateProgram();
        GLES30.glAttachShader(p,v); GLES30.glAttachShader(p,f); GLES30.glLinkProgram(p);
        GLES30.glDeleteShader(v); GLES30.glDeleteShader(f);
        int[] ok=new int[1]; GLES30.glGetProgramiv(p,GLES30.GL_LINK_STATUS,ok,0);
        if (ok[0]==0) { String message=GLES30.glGetProgramInfoLog(p); GLES30.glDeleteProgram(p); throw new IllegalStateException(message); }
        return p;
    }
    static int shader(int kind,String text) {
        int id=GLES30.glCreateShader(kind); GLES30.glShaderSource(id,text); GLES30.glCompileShader(id);
        int[] ok=new int[1]; GLES30.glGetShaderiv(id,GLES30.GL_COMPILE_STATUS,ok,0);
        if (ok[0]==0) { String message=GLES30.glGetShaderInfoLog(id); GLES30.glDeleteShader(id); throw new IllegalStateException(message); }
        return id;
    }
    void bind(int unit,int target,int image,int p,String name) {
        GLES30.glActiveTexture(GLES30.GL_TEXTURE0+unit); GLES30.glBindTexture(target,image);
        GLES30.glUniform1i(uniforms.get(name),unit);
    }
    void draw(int p) {
        quad.position(0); GLES30.glVertexAttribPointer(0,2,GLES30.GL_FLOAT,false,0,quad);
        GLES30.glEnableVertexAttribArray(0); GLES30.glDrawArrays(GLES30.GL_TRIANGLE_STRIP,0,4);
    }
    void camera(Options o,boolean analysis,boolean maskReady) {
        GLES30.glUseProgram(program); bind(0,GLES11Ext.GL_TEXTURE_EXTERNAL_OES,cameraImage,program,"cameraTexture");
        bind(1,GLES30.GL_TEXTURE_2D,images[2]==0 ? maskImage : images[2],program,"blurredTexture"); bind(2,GLES30.GL_TEXTURE_2D,maskImage,program,"personMask");
        GLES30.glUniformMatrix4fv(uniforms.get("textureMatrix"),1,false,textureMatrix,0);
        float source=(float)inputWidth/inputHeight; if (o.rotation()%180!=0) source=1/source;
        float target=(float)width/height, x=1,y=1;
        if (o.fit()==0) { if (target>source) x=target/source; else y=source/target; }
        if (o.fit()==1) { if (target>source) y=source/target; else x=target/source; }
        GLES30.glUniform2f(uniforms.get("aspectScale"),x,y);
        GLES30.glUniform2f(uniforms.get("stretch"),o.stretchX(),o.stretchY());
        GLES30.glUniform2f(uniforms.get("center"),o.centerX(),o.centerY());
        GLES30.glUniform1f(uniforms.get("distortion"),o.distortion());
        GLES30.glUniform1f(uniforms.get("bulge"),o.bulge());
        GLES30.glUniform1f(uniforms.get("radius"),o.radius());
        GLES30.glUniform1f(uniforms.get("blurEnabled"),o.blur()>0 ? 1:0);
        GLES30.glUniform1f(uniforms.get("maskReady"),maskReady ? 1:0);
        GLES30.glUniform1i(uniforms.get("analysis"),analysis ? 1:0);
        GLES30.glUniform1i(uniforms.get("rotation"),o.rotation());
        GLES30.glUniform1i(uniforms.get("mirror"),o.mirror() ? 1:0);
        draw(program);
    }
    void createBlurBuffers() {
        GLES30.glGenFramebuffers(3,buffers,0);
        for (int i=0;i<3;i++) {
            images[i]=texture(GLES30.GL_TEXTURE_2D);
            GLES30.glTexImage2D(GLES30.GL_TEXTURE_2D,0,GLES30.GL_RGBA8,BackgroundSegmenter.WIDTH,BackgroundSegmenter.HEIGHT,0,GLES30.GL_RGBA,GLES30.GL_UNSIGNED_BYTE,null);
            GLES30.glBindFramebuffer(GLES30.GL_FRAMEBUFFER,buffers[i]);
            GLES30.glFramebufferTexture2D(GLES30.GL_FRAMEBUFFER,GLES30.GL_COLOR_ATTACHMENT0,GLES30.GL_TEXTURE_2D,images[i],0);
            check(GLES30.glCheckFramebufferStatus(GLES30.GL_FRAMEBUFFER)==GLES30.GL_FRAMEBUFFER_COMPLETE,"Create blur framebuffer");
        }
    }
    void blur(Options o,long now) {
        if (buffers[0]==0) createBlurBuffers();
        if (segmenter==null) segmenter=new BackgroundSegmenter(context,o.delegate(),error);
        GLES30.glBindFramebuffer(GLES30.GL_FRAMEBUFFER,buffers[0]);
        GLES30.glViewport(0,0,BackgroundSegmenter.WIDTH,BackgroundSegmenter.HEIGHT); camera(o,true,false);
        if (segmenter.idle() && now-sampledMs >= 1000/o.maskFps()) {
            sampledMs=now;
            ByteBuffer rgba=ByteBuffer.allocateDirect(BackgroundSegmenter.WIDTH*BackgroundSegmenter.HEIGHT*4);
            GLES30.glReadPixels(0,0,BackgroundSegmenter.WIDTH,BackgroundSegmenter.HEIGHT,GLES30.GL_RGBA,GLES30.GL_UNSIGNED_BYTE,rgba);
            rgba.position(0); segmenter.submit(rgba,now);
        }
        for (int pass=0;pass<2;pass++) {
            GLES30.glBindFramebuffer(GLES30.GL_FRAMEBUFFER,buffers[pass+1]); GLES30.glUseProgram(blurProgram);
            bind(0,GLES30.GL_TEXTURE_2D,images[pass],blurProgram,"image");
            GLES30.glUniform2f(uniforms.get("stepSize"),pass==0 ? o.blur()/BackgroundSegmenter.WIDTH:0,pass==1 ? o.blur()/BackgroundSegmenter.HEIGHT:0);
            draw(blurProgram);
        }
        BackgroundSegmenter.Mask mask=segmenter.mask;
        if (mask!=null && mask!=uploaded) {
            GLES30.glActiveTexture(GLES30.GL_TEXTURE2); GLES30.glBindTexture(GLES30.GL_TEXTURE_2D,maskImage);
            GLES30.glPixelStorei(GLES30.GL_UNPACK_ALIGNMENT,1); mask.pixels().position(0);
            GLES30.glTexImage2D(GLES30.GL_TEXTURE_2D,0,GLES30.GL_R8,mask.width(),mask.height(),0,GLES30.GL_RED,GLES30.GL_UNSIGNED_BYTE,mask.pixels());
            uploaded=mask;
        }
    }
    void render() {
        long start=System.nanoTime(), now=SystemClock.uptimeMillis(); Options o=options;
        cameraTexture.updateTexImage(); cameraTexture.getTransformMatrix(textureMatrix);
        if (o.blur()>0) blur(o,now);
        else if (segmenter!=null) { segmenter.close(); segmenter=null; uploaded=null; }
        GLES30.glBindFramebuffer(GLES30.GL_FRAMEBUFFER,0); GLES30.glViewport(0,0,width,height);
        camera(o,false,uploaded!=null && now-uploaded.sampledMs()<=500);
        EGLExt.eglPresentationTimeANDROID(display,eglSurface,cameraTexture.getTimestamp());
        check(EGL14.eglSwapBuffers(display,eglSurface),"Submit processed frame");
        if (previewSurface != EGL14.EGL_NO_SURFACE) {
            check(EGL14.eglMakeCurrent(display, previewSurface, previewSurface, eglContext), "Bind preview");
            GLES30.glViewport(0, 0, width, height);
            camera(o, false, uploaded != null && now-uploaded.sampledMs()<=500);
            check(EGL14.eglSwapBuffers(display, previewSurface), "Submit preview frame");
            check(EGL14.eglMakeCurrent(display, eglSurface, eglSurface, eglContext), "Restore encoder surface");
        }
        submitMs=(System.nanoTime()-start)/1e6; renderedFrames++;
    }
    JSONObject stats() {
        return Json.object("mode","phone GPU","frameSubmitMs",submitMs,"frames",renderedFrames,
                "ml",segmenter==null ? JSONObject.NULL : segmenter.stats());
    }
    static void check(boolean valid,String action) { if (!valid) throw new IllegalStateException(action+" failed (EGL "+EGL14.eglGetError()+")"); }

    @Override public void close() {
        closed=true;
        FutureTask<Void> release=new FutureTask<>(() -> {
            if (segmenter!=null) { segmenter.close(); segmenter=null; }
            if (cameraSurface!=null) cameraSurface.release();
            if (cameraTexture!=null) cameraTexture.release();
            if (eglContext!=EGL14.EGL_NO_CONTEXT) {
                GLES30.glDeleteTextures(3,images,0); GLES30.glDeleteTextures(2,new int[]{cameraImage,maskImage},0);
                GLES30.glDeleteFramebuffers(3,buffers,0); GLES30.glDeleteProgram(program); GLES30.glDeleteProgram(blurProgram);
            }
            if (display!=EGL14.EGL_NO_DISPLAY) {
                EGL14.eglMakeCurrent(display,EGL14.EGL_NO_SURFACE,EGL14.EGL_NO_SURFACE,EGL14.EGL_NO_CONTEXT);
                if (previewSurface != EGL14.EGL_NO_SURFACE) EGL14.eglDestroySurface(display, previewSurface);
                EGL14.eglDestroySurface(display,eglSurface); EGL14.eglDestroyContext(display,eglContext); EGL14.eglTerminate(display);
            }
            thread.quitSafely(); return null;
        });
        handler.post(release);
        try { release.get(5,TimeUnit.SECONDS); } catch (Exception ignored) { thread.quitSafely(); }
    }
}
