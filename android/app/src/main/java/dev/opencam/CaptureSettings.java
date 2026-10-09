package dev.opencam;

import java.util.*;
import org.json.*;

final class CaptureSettings {
    static JSONObject copy(JSONObject value) throws JSONException { return new JSONObject(value.toString()); }

    static List<Integer> fps(JSONObject camera, JSONArray size) throws JSONException {
        long duration = 0;
        JSONArray durations = camera.optJSONArray("frameDurations");
        if (durations != null) for (int i = 0; i < durations.length(); i++) {
            JSONObject entry = durations.getJSONObject(i);
            if (entry.getJSONArray("size").toString().equals(size.toString())) duration = entry.optLong("minFrameNs");
        }
        int maximum = duration > 0 ? (int) (1_000_000_000L / duration) : 240;
        TreeSet<Integer> values = new TreeSet<>();
        JSONArray ranges = camera.getJSONArray("fpsRanges");
        for (int i = 0; i < ranges.length(); i++) {
            JSONArray range = ranges.getJSONArray(i);
            for (int rate = Math.max(1, range.getInt(0)); rate <= Math.min(240, Math.min(maximum, range.getInt(1))); rate++) values.add(rate);
        }
        return new ArrayList<>(values);
    }

    static JSONObject defaults(JSONObject catalog, JSONObject camera) throws Exception {
        JSONArray codecs = catalog.getJSONArray("codecs");
        JSONObject codec = codecs.optJSONObject(0);
        for (int i = 0; i < codecs.length(); i++) if (codecs.getJSONObject(i).getString("mime").equals("video/avc")) { codec = codecs.getJSONObject(i); break; }
        camera=WirePixels.camera(camera,codec==null?"":codec.getString("name"));
        JSONArray sizes = camera.getJSONArray("sizes"),size=null;
        long best=Long.MAX_VALUE;
        for(int i=0;i<sizes.length();i++) {
            JSONArray candidate=sizes.getJSONArray(i);if(fps(camera,candidate).isEmpty())continue;
            long distance=Math.abs(candidate.getInt(0)-1280L)+Math.abs(candidate.getInt(1)-720L);
            if(distance<best){best=distance;size=candidate;}
        }
        if(size==null)throw new IllegalArgumentException("This camera has no regular preview mode");
        int rate = fps(camera, size).stream().min(Comparator.comparingInt(f -> Math.abs(f - 30))).orElseThrow();
        JSONArray iso = camera.optJSONArray("iso"), exposure = camera.optJSONArray("exposureNs");
        return Json.object("camera", camera.getString("id"), "width", size.getInt(0), "height", size.getInt(1), "fps", rate,
                "codec", codec == null ? "" : codec.getString("name"),
                "bitrate", codec == null ? 8_000_000 : Math.max(codec.getJSONArray("bitrate").getInt(0), Math.min(8_000_000, codec.getJSONArray("bitrate").getInt(1))),
                "manual", false, "iso", iso == null ? 100 : Math.min(iso.getInt(1), Math.max(100, iso.getInt(0))),
                "exposureNs", exposure == null ? 8_333_333 : Math.min(exposure.getLong(1), Math.max(exposure.getLong(0), 8_333_333)),
                "focusAuto", true, "focus", 0, "zoom", 1.0, "ev", 0, "awb", 1,
                "ois", false, "stabilization", false, "torch", false, "aeLock", false, "awbLock", false,
                "gains", new JSONArray(new double[]{1, 1, 1, 1}));
    }

    static boolean stale(JSONObject command, long revision) {
        return command.has("baseRevision") && command.optLong("baseRevision", -1) != revision;
    }
}
