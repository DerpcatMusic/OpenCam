package dev.opencam;

public class LimitsTest {
    public static void main(String[] args) {
        assert Limits.checked("ISO", 100, 50, 800) == 100;
        for (double bad : new double[]{49, 801, Double.NaN, Double.POSITIVE_INFINITY}) {
            try { Limits.checked("ISO", bad, 50, 800); throw new AssertionError("Accepted " + bad); }
            catch (IllegalArgumentException expected) { }
        }
        assert Limits.previewFps(new int[][]{{15,30},{60,60}}) == 30;
        assert Limits.previewFps(new int[][]{{60,60}}) == 60;
        assert Limits.previewFps(new int[][]{{10,20}}) == 20;
        int[] fixed = Limits.fpsRange(new int[][]{{15, 30}, {30, 30}, {30, 60}}, 30);
        assert fixed[0] == 30 && fixed[1] == 30;
        try { Limits.fpsRange(new int[][]{{15, 30}}, 60); throw new AssertionError("Accepted unsupported fps"); }
        catch (IllegalArgumentException expected) { }
        System.out.println("Camera range validation passed");
    }
}
