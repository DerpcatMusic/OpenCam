package dev.opencam;

import android.graphics.Rect;
import android.util.Range;
import android.util.Size;
import android.util.SizeF;
import java.lang.reflect.Array;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

final class Json {
    static JSONObject object(Object... pairs) {
        JSONObject value = new JSONObject();
        try {
            for (int i = 0; i < pairs.length; i += 2) value.put((String) pairs[i], encode(pairs[i + 1]));
        } catch (JSONException e) { throw new IllegalArgumentException(e); }
        return value;
    }

    static Object encode(Object value) throws JSONException {
        if (value == null || value == JSONObject.NULL) return JSONObject.NULL;
        if (value instanceof android.util.Rational r) return r.isFinite() ? r.doubleValue() : JSONObject.NULL;
        if (value instanceof Float f && !Float.isFinite(f)) return JSONObject.NULL;
        if (value instanceof Double d && !Double.isFinite(d)) return JSONObject.NULL;
        if (value instanceof JSONObject || value instanceof JSONArray || value instanceof String
                || value instanceof Boolean || value instanceof Number) return value;
        if (value instanceof Range<?> r) return new JSONArray().put(encode(r.getLower())).put(encode(r.getUpper()));
        if (value instanceof Size s) return new JSONArray().put(s.getWidth()).put(s.getHeight());
        if (value instanceof SizeF s) return new JSONArray().put(s.getWidth()).put(s.getHeight());
        if (value instanceof Rect r) return new JSONArray().put(r.left).put(r.top).put(r.right).put(r.bottom);
        if (value instanceof Iterable<?> items) {
            JSONArray array = new JSONArray();
            for (Object item : items) array.put(encode(item));
            return array;
        }
        if (value.getClass().isArray()) {
            JSONArray array = new JSONArray();
            for (int i = 0; i < Array.getLength(value); i++) array.put(encode(Array.get(value, i)));
            return array;
        }
        return value.toString();
    }
}
