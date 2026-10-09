package dev.opencam;

final class Limits {
    static double checked(String name, double value, double min, double max) {
        if (!Double.isFinite(value) || value < min || value > max)
            throw new IllegalArgumentException(name + " must be between " + min + " and " + max);
        return value;
    }

    static int previewFps(int[][] ranges) {
        int best = 0;
        for (int[] range : ranges) {
            if (range[0] < 1 || range[0] > range[1]) continue;
            int fps = Math.max(range[0], Math.min(30, range[1]));
            if (best == 0 || Math.abs(fps - 30) < Math.abs(best - 30)) best = fps;
        }
        if (best == 0) throw new IllegalArgumentException("No regular frame rates advertised");
        return best;
    }

    static int[] fpsRange(int[][] ranges, int fps) {
        int[] best = null;
        for (int[] range : ranges) {
            if (range[0] <= fps && range[1] >= fps
                    && (best == null || range[1] - range[0] < best[1] - best[0])) best = range;
        }
        if (best == null) throw new IllegalArgumentException("Frame rate is not exposed in a regular camera session");
        return best;
    }
}
