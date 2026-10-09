package dev.opencam;

import android.content.Context;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.SystemClock;
import com.google.mediapipe.framework.image.ByteBufferExtractor;
import com.google.mediapipe.framework.image.ByteBufferImageBuilder;
import com.google.mediapipe.framework.image.MPImage;
import com.google.mediapipe.tasks.core.BaseOptions;
import com.google.mediapipe.tasks.core.Delegate;
import com.google.mediapipe.tasks.vision.core.RunningMode;
import com.google.mediapipe.tasks.vision.imagesegmenter.ImageSegmenter;
import com.google.mediapipe.tasks.vision.imagesegmenter.ImageSegmenterResult;
import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;
import org.json.JSONObject;

final class BackgroundSegmenter implements AutoCloseable {
    static final int WIDTH = 256, HEIGHT = 144;
    record Mask(ByteBuffer pixels, int width, int height, long sampledMs) { }
    final Context context;
    final String requested;
    final Consumer<Exception> error;
    final HandlerThread thread = new HandlerThread("opencam-segmentation");
    final Handler handler;
    final AtomicBoolean busy = new AtomicBoolean();
    volatile boolean closed;
    volatile Mask mask;
    volatile double inferenceMs, cpuMs = -1, gpuMs = -1;
    volatile String delegate = "warming up";
    ImageSegmenter segmenter;

    BackgroundSegmenter(Context context, String requested, Consumer<Exception> error) {
        this.context = context; this.requested = requested; this.error = error;
        thread.start(); handler = new Handler(thread.getLooper());
    }

    boolean idle() { return !closed && !busy.get(); }

    // One inference in flight; its mask never holds up current video frames.
    void submit(ByteBuffer rgba, long sampledMs) {
        if (closed || !busy.compareAndSet(false, true)) return;
        handler.post(() -> {
            try (MPImage image = new ByteBufferImageBuilder(rgba, WIDTH, HEIGHT, MPImage.IMAGE_FORMAT_RGBA).build()) {
                if (closed) return;
                if (segmenter == null) initialize(image);
                long start = System.nanoTime();
                ImageSegmenterResult result = segmenter.segment(image);
                inferenceMs = (System.nanoTime() - start) / 1e6;
                MPImage category = result.categoryMask().orElseThrow(() -> new IllegalStateException("Segmentation returned no person mask"));
                try {
                    ByteBuffer source = ByteBufferExtractor.extract(category);
                    ByteBuffer pixels = ByteBuffer.allocateDirect(category.getWidth() * category.getHeight());
                    while (source.hasRemaining()) pixels.put(source.get() == 1 ? (byte) 255 : 0);
                    pixels.flip();
                    if (!closed) mask = new Mask(pixels, category.getWidth(), category.getHeight(), sampledMs);
                } finally { category.close(); }
            } catch (Exception e) { if (!closed) error.accept(e); }
            finally { busy.set(false); }
        });
    }

    ImageSegmenter create(Delegate choice) {
        return ImageSegmenter.createFromOptions(context, ImageSegmenter.ImageSegmenterOptions.builder()
                .setBaseOptions(BaseOptions.builder().setModelAssetPath("selfie_segmenter_landscape.tflite").setDelegate(choice).build())
                .setRunningMode(RunningMode.IMAGE).setOutputCategoryMask(true).setOutputConfidenceMasks(false).build());
    }

    double measure(ImageSegmenter candidate, MPImage image) {
        double total = 0;
        for (int i = 0; i < 5; i++) {
            long start = System.nanoTime();
            ImageSegmenterResult result = candidate.segment(image);
            if (i >= 2) total += (System.nanoTime() - start) / 1e6;
            result.categoryMask().ifPresent(MPImage::close);
        }
        return total / 3;
    }

    void initialize(MPImage image) {
        if (!requested.equals("auto")) {
            segmenter = create(requested.equals("gpu") ? Delegate.GPU : Delegate.CPU);
            delegate = requested.toUpperCase();
            return;
        }
        ImageSegmenter cpu = null, gpu = null;
        try {
            cpu = create(Delegate.CPU); cpuMs = measure(cpu, image);
            try { gpu = create(Delegate.GPU); gpuMs = measure(gpu, image); }
            catch (RuntimeException unsupported) { if (gpu != null) { gpu.close(); gpu = null; } }
            if (gpu != null && gpuMs < cpuMs) {
                segmenter = gpu; gpu = null; delegate = "GPU";
            } else { segmenter = cpu; cpu = null; delegate = "CPU"; }
        } finally { if (cpu != null) cpu.close(); if (gpu != null) gpu.close(); }
    }

    JSONObject stats() {
        Mask latest = mask;
        return Json.object("delegate", delegate, "inferenceMs", inferenceMs,
                "cpuMs", cpuMs < 0 ? JSONObject.NULL : cpuMs, "gpuMs", gpuMs < 0 ? JSONObject.NULL : gpuMs,
                "maskAgeMs", latest == null ? JSONObject.NULL : SystemClock.uptimeMillis() - latest.sampledMs());
    }

    @Override public void close() {
        closed = true; mask = null;
        CountDownLatch done = new CountDownLatch(1);
        handler.post(() -> { try { if (segmenter != null) { segmenter.close(); segmenter = null; } } finally { done.countDown(); thread.quitSafely(); } });
        try { done.await(5, TimeUnit.SECONDS); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
    }
}
