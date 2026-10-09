package dev.opencam;

import android.content.Context;
import android.text.InputType;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.*;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.slider.Slider;
import com.google.android.material.textfield.*;
import java.util.*;
import java.util.function.*;
import org.json.*;

final class CameraControls {
    final Context context;
    final Runnable raw;
    final LinearLayout root;
    final BiConsumer<JSONObject, Boolean> apply;
    final Consumer<String> error;
    final List<Consumer<JSONObject>> bindings = new ArrayList<>();
    JSONObject settings = new JSONObject(), catalog, camera;
    boolean binding;
    int tool;

    CameraControls(Context context, LinearLayout root, BiConsumer<JSONObject, Boolean> apply, Consumer<String> error,Runnable raw) {
        this.context = context; this.root = root; this.apply = apply; this.error = error;this.raw=raw;
    }
    int dp(int value) { return Math.round(value * context.getResources().getDisplayMetrics().density); }
    void state(JSONObject catalog, JSONObject value, int tool) {
        boolean rebuild = this.catalog == null || this.tool != tool || !settings.optString("camera").equals(value.optString("camera"))
                || !settings.optString("codec").equals(value.optString("codec")) || tool == 1 && (settings.optInt("width") != value.optInt("width") || settings.optInt("height") != value.optInt("height"));
        this.catalog = catalog; settings = value; this.tool = tool;
        if (rebuild) {
            root.removeAllViews(); bindings.clear(); camera = null;
            try {
                JSONArray cameras = catalog.getJSONArray("cameras");
                for (int i = 0; i < cameras.length(); i++) {
                    JSONObject candidate = cameras.getJSONObject(i);
                    if (candidate.getString("id").equals(settings.getString("camera"))) camera = candidate;
                }
                if (camera == null) return;
                camera=WirePixels.camera(camera,settings.optString("codec"));
                switch (tool) { case 0 -> lenses(); case 1 -> format(); case 2 -> sensor(); case 3 -> effects(); }
            } catch (Exception e) { error.accept(e.getMessage()); }
        }
        binding = true;
        try { for (var update : bindings) update.accept(settings); }
        finally { binding = false; }
    }
    void patch(JSONObject values, boolean configure) { if (!binding) apply.accept(values, configure); }
    void spacing(View view) {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2); p.bottomMargin = dp(8); root.addView(view, p);
    }
    void toggle(String label, String key, boolean available) {
        if (!available) return;
        MaterialSwitch view = new MaterialSwitch(context); view.setText(label); view.setMinHeight(dp(48));
        view.setOnCheckedChangeListener((button, checked) -> {
            JSONObject values = Json.object(key, checked);
            if (checked && key.equals("ois")) values = Json.object("ois", true, "stabilization", false);
            if (checked && key.equals("stabilization")) values = Json.object("stabilization", true, "ois", false);
            patch(values, key.equals("phoneMirror"));
        });
        bindings.add(s -> view.setChecked(s.optBoolean(key, key.equals("focusAuto")))); spacing(view);
    }
    void number(String label, String key, double low, double high, boolean integer, boolean configure) {
        editor(label, List.of(), s -> s.has(key) ? s.opt(key).toString() : key.equals("outputWidth") ? Integer.toString(PhoneProcessor.outputWidth(s)) : Integer.toString(PhoneProcessor.outputHeight(s)), text -> {
            double value = Double.parseDouble(text.trim()); Limits.checked(label, value, low, high);
            if (integer && value != Math.rint(value)) throw new IllegalArgumentException(label + " requires a whole number");
            JSONObject values = Json.object(key, integer ? (Object) (long) value : value);
            if (key.equals("iso")) values.put("manual", true);
            if (key.equals("focus")) values.put("focusAuto", false);
            patch(values, configure);
        }, InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL | InputType.TYPE_NUMBER_FLAG_SIGNED);
    }
    void slider(String label, String key, float low, float high, float fallback, boolean integer) {
        if (low >= high) return;
        TextView text = new TextView(context); text.setTextAppearance(com.google.android.material.R.style.TextAppearance_Material3_LabelLarge); root.addView(text);
        Slider slider = new Slider(context); slider.setValueFrom(low); slider.setValueTo(high); slider.setStepSize(integer ? 1 : 0);
        slider.setContentDescription(label); slider.setMinimumHeight(dp(48));
        boolean[] dragging = {false}; long[] last = {0};
        Runnable submit = () -> { last[0] = android.os.SystemClock.uptimeMillis(); patch(Json.object(key, integer ? (Object) Math.round(slider.getValue()) : slider.getValue()), false); };
        slider.addOnChangeListener((v, value, user) -> {
            text.setText(label + "  " + String.format(Locale.US, integer ? "%.0f" : "%.2f", value));
            if (user) { slider.removeCallbacks(submit); slider.postDelayed(submit, Math.max(0, 33 - (android.os.SystemClock.uptimeMillis() - last[0]))); }
        });
        slider.addOnSliderTouchListener(new Slider.OnSliderTouchListener() {
            public void onStartTrackingTouch(Slider s) { dragging[0] = true; }
            public void onStopTrackingTouch(Slider s) { dragging[0] = false; slider.removeCallbacks(submit); submit.run(); }
        });
        bindings.add(s -> { if (!dragging[0]) {
            float value = (float) Math.max(low, Math.min(high, s.optDouble(key, fallback))); slider.setValue(value);
            text.setText(label + "  " + String.format(Locale.US, integer ? "%.0f" : "%.2f", value));
        } });
        spacing(slider);
    }
    interface Edit { void apply(String text) throws Exception; }
    void editor(String label, List<String> choices, Function<JSONObject, String> value, Edit edit, int type) {
        LinearLayout row = new LinearLayout(context); row.setGravity(android.view.Gravity.CENTER_VERTICAL);
        TextInputLayout field = new TextInputLayout(context, null, com.google.android.material.R.attr.textInputOutlinedExposedDropdownMenuStyle);
        field.setHint(label); field.setEndIconMode(choices.isEmpty() ? TextInputLayout.END_ICON_NONE : TextInputLayout.END_ICON_DROPDOWN_MENU);
        MaterialAutoCompleteTextView input = new MaterialAutoCompleteTextView(field.getContext()); input.setSingleLine(true); input.setInputType(type); input.setImeOptions(EditorInfo.IME_ACTION_DONE);
        input.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_dropdown_item_1line, choices)); input.setThreshold(0);
        field.addView(input); row.addView(field, new LinearLayout.LayoutParams(0, -2, 1));
        Runnable submit = () -> { try { edit.apply(input.getText().toString()); field.setError(null); input.clearFocus(); } catch (Exception e) { field.setError(e.getMessage()); } };
        input.setOnItemClickListener((parent, view, position, id) -> submit.run());
        input.setOnEditorActionListener((v, action, event) -> { if (action == EditorInfo.IME_ACTION_DONE) { submit.run(); return true; } return false; });
        MaterialButton check = icon(context, R.drawable.ic_check, "Apply " + label); check.setOnClickListener(v -> submit.run());
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(dp(48), dp(48)); p.leftMargin = dp(8); row.addView(check, p);
        bindings.add(s -> { if (!input.hasFocus()) input.setText(value.apply(s), false); }); spacing(row);
    }
    void menu(String label, List<String> labels, List<Object> values, Function<JSONObject, Object> selected, Consumer<Object> choose) {
        if (values.isEmpty()) return;
        TextInputLayout field = new TextInputLayout(context, null, com.google.android.material.R.attr.textInputOutlinedExposedDropdownMenuStyle);
        field.setHint(label); field.setEndIconMode(TextInputLayout.END_ICON_DROPDOWN_MENU);
        MaterialAutoCompleteTextView input = new MaterialAutoCompleteTextView(field.getContext()); input.setInputType(InputType.TYPE_NULL); input.setKeyListener(null); input.setSingleLine(true);
        input.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_dropdown_item_1line, labels)); field.addView(input);
        input.setOnItemClickListener((parent, view, position, id) -> { if (!binding) choose.accept(values.get(position)); });
        bindings.add(s -> { Object value = selected.apply(s); int index = values.indexOf(value); input.setText(index >= 0 ? labels.get(index) : String.valueOf(value), false); }); spacing(field);
    }
    void mode(String label, String key, String capability, String[] names, boolean auto) throws JSONException {
        JSONArray modes = camera.optJSONArray(capability); if (modes == null || modes.length() == 0) return;
        List<String> labels = new ArrayList<>(); List<Object> values = new ArrayList<>();
        if (auto) { labels.add("Default"); values.add(-1); }
        for (int i = 0; i < modes.length(); i++) { int mode = modes.getInt(i); values.add(mode); labels.add(mode < names.length ? names[mode] : Integer.toString(mode)); }
        menu(label, labels, values, s -> s.optInt(key, auto ? -1 : 1), value -> patch(Json.object(key, value), false));
    }
    void lenses() throws Exception {
        List<String> labels = new ArrayList<>(); List<Object> values = new ArrayList<>(); JSONArray cameras = catalog.getJSONArray("cameras");
        for (int i = 0; i < cameras.length(); i++) { JSONObject c = cameras.getJSONObject(i); labels.add(c.getString("label") + " · " + c.getString("id")); values.add(c.getString("id")); }
        menu("Camera", labels, values, s -> s.optString("camera"), id -> {
            try { for (int i = 0; i < cameras.length(); i++) if (cameras.getJSONObject(i).getString("id").equals(id)) apply.accept(CaptureSettings.defaults(catalog, cameras.getJSONObject(i)), true); }
            catch (Exception e) { error.accept(e.getMessage()); }
        });
        slider("Zoom", "zoom", (float) camera.getJSONArray("zoom").getDouble(0), (float) camera.getJSONArray("zoom").getDouble(1), 1, false);
        toggle("Torch", "torch", camera.optBoolean("flash"));
        toggle("Autofocus", "focusAuto", camera.optBoolean("manualFocus"));
        if (camera.optBoolean("manualFocus")) number("Focus · diopters", "focus", 0, camera.getDouble("focusMax"), false, false);
    }
    void format() throws Exception {
        List<String> resolutions = new ArrayList<>(); JSONArray sizes = camera.getJSONArray("sizes");
        for (int i = 0; i < sizes.length(); i++) { JSONArray size = sizes.getJSONArray(i); resolutions.add(size.getInt(0) + "x" + size.getInt(1)); }
        editor("Sensor resolution", resolutions, s -> s.optInt("width") + "x" + s.optInt("height"), text -> {
            String[] parts = text.toLowerCase(Locale.ROOT).replace('×', 'x').replace(" ", "").split("x");
            if (parts.length != 2) throw new IllegalArgumentException("Enter width x height");
            JSONArray size = new JSONArray(new int[]{Integer.parseInt(parts[0]), Integer.parseInt(parts[1])});
            if (!resolutions.contains(size.getInt(0) + "x" + size.getInt(1))) throw new IllegalArgumentException("Sensor does not expose this resolution");
            List<Integer> rates = CaptureSettings.fps(camera, size);
            int fps = rates.contains(settings.optInt("fps")) ? settings.optInt("fps") : rates.stream().min(Comparator.comparingInt(f -> Math.abs(f - 30))).orElseThrow(() -> new IllegalArgumentException("Select a regular sensor mode"));
            patch(Json.object("width", size.getInt(0), "height", size.getInt(1), "fps", fps), true);
        }, InputType.TYPE_CLASS_TEXT);
        List<String> rates = new ArrayList<>(); JSONArray size = new JSONArray(new int[]{settings.optInt("width"), settings.optInt("height")});
        for (int f : CaptureSettings.fps(camera, size)) rates.add(Integer.toString(f));
        JSONArray high = camera.optJSONArray("highSpeed");
        if (high != null) for (int i = 0; i < high.length(); i++) {
            JSONObject mode = high.getJSONObject(i); if (!mode.getJSONArray("size").toString().equals(size.toString())) continue;
            JSONArray ranges = mode.getJSONArray("fpsRanges");
            for (int j = 0; j < ranges.length(); j++) { JSONArray range = ranges.getJSONArray(j); if (range.getInt(0) == range.getInt(1)) rates.add(Integer.toString(range.getInt(0))); }
        }
        editor("Frame rate · fps", rates, s -> Integer.toString(s.optInt("fps")), text -> patch(Json.object("fps", Integer.parseInt(text.trim())), true), InputType.TYPE_CLASS_NUMBER);
        List<String> labels = new ArrayList<>(); List<Object> codecs = new ArrayList<>(); JSONArray encoders = catalog.getJSONArray("codecs");
        for (int i = 0; i < encoders.length(); i++) { JSONObject c = encoders.getJSONObject(i); labels.add(WirePixels.uncompressed(c.getString("name"))?c.getString("label"):c.getString("label") + " · " + c.getString("name")); codecs.add(c.getString("name")); }
        menu("Transport format",labels,codecs,s->s.optString("codec"),codec->{
            try {
                JSONObject mode=WirePixels.camera(camera,codec.toString());JSONArray modes=mode.getJSONArray("sizes"),choiceSize=null;
                for(int i=0;i<modes.length();i++)if(modes.getJSONArray(i).getInt(0)==settings.optInt("width") && modes.getJSONArray(i).getInt(1)==settings.optInt("height"))choiceSize=modes.getJSONArray(i);
                if(choiceSize==null)choiceSize=modes.getJSONArray(0);
                int fps=CaptureSettings.fps(mode,choiceSize).stream().min(Comparator.comparingInt(f->Math.abs(f-settings.optInt("fps",30)))).orElseThrow();
                patch(Json.object("codec",codec,"width",choiceSize.getInt(0),"height",choiceSize.getInt(1),"fps",fps),true);
            }catch(Exception e){error.accept(e.getMessage());}
        });
        if(!WirePixels.uncompressed(settings.optString("codec")))number("Bitrate · bit/s", "bitrate", 1, Integer.MAX_VALUE, true, true);
        JSONArray raw=camera.optJSONArray("rawSizes");
        if(camera.optBoolean("raw") && raw!=null && raw.length()>0) {
            List<String> choices=new ArrayList<>();for(int i=0;i<raw.length();i++){JSONArray rawSize=raw.getJSONArray(i);choices.add(rawSize.getInt(0)+"x"+rawSize.getInt(1));}
            choices.sort(Comparator.comparingLong(text->{String[] pair=text.split("x");return (long)Integer.parseInt(pair[0])*Integer.parseInt(pair[1]);}));
            editor("RAW resolution",choices,s->s.optInt("rawWidth")>0?s.optInt("rawWidth")+"x"+s.optInt("rawHeight"):choices.get(choices.size()-1),text->{
                String normalized=text.toLowerCase(Locale.US).replace('×','x').replace(" ","");if(!choices.contains(normalized))throw new IllegalArgumentException("Sensor does not expose this RAW resolution");
                String[] pair=normalized.split("x");patch(Json.object("rawWidth",Integer.parseInt(pair[0]),"rawHeight",Integer.parseInt(pair[1])),false);
            },InputType.TYPE_CLASS_TEXT);
            MaterialButton capture=icon(context,R.drawable.ic_camera,"Capture RAW DNG");capture.setOnClickListener(v->this.raw.run());spacing(capture);
        }
        number("Output width", "outputWidth", 2, 8192, true, true); number("Output height", "outputHeight", 2, 8192, true, true);
        menu("Aspect mapping", List.of("Fit", "Crop", "Stretch"), List.of(0, 1, 2), s -> s.optInt("outputMode"), value -> patch(Json.object("outputMode", value), true));
        menu("Rotation", List.of("0°", "90°", "180°", "270°"), List.of(0, 90, 180, 270), s -> s.optInt("phoneRotation"), value -> patch(Json.object("phoneRotation", value), true));
        toggle("Mirror", "phoneMirror", true);
    }
    void sensor() throws Exception {
        toggle("Manual exposure", "manual", camera.optBoolean("manualSensor"));
        if (camera.optBoolean("manualSensor")) {
            number("ISO", "iso", camera.getJSONArray("iso").getDouble(0), camera.getJSONArray("iso").getDouble(1), true, false);
            editor("Shutter · ms", List.of("4.167", "8.333", "16.667", "33.333"), s -> String.format(Locale.US, "%.3f", s.optLong("exposureNs") / 1e6), text -> patch(Json.object("exposureNs", Math.round(Double.parseDouble(text.trim()) * 1e6), "manual", true), false), InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL);
        }
        slider("Exposure compensation", "ev", camera.getJSONArray("ev").getInt(0), camera.getJSONArray("ev").getInt(1), 0, true);
        toggle("Exposure lock", "aeLock", camera.optBoolean("aeLock"));
        mode("White balance", "awb", "awbModes", new String[]{"Manual gains", "Auto", "Incandescent", "Fluorescent", "Warm fluorescent", "Daylight", "Cloudy", "Twilight", "Shade"}, false);
        if (has(camera.optJSONArray("awbModes"), 0)) editor("WB gains · R, G₁, G₂, B", List.of(), s -> s.optJSONArray("gains") == null ? "1, 1, 1, 1" : s.optJSONArray("gains").toString().replace("[", "").replace("]", ""), text -> {
            String[] parts = text.split(","); if (parts.length != 4) throw new IllegalArgumentException("Enter four gains separated by commas");
            JSONArray gains = new JSONArray(); for (String part : parts) gains.put(Limits.checked("Gain", Double.parseDouble(part.trim()), 1, 8)); patch(Json.object("gains", gains), false);
        }, InputType.TYPE_CLASS_TEXT);
        toggle("White balance lock", "awbLock", camera.optBoolean("awbLock"));
        toggle("Optical stabilization", "ois", has(camera.optJSONArray("oisModes"), 1)); toggle("Video stabilization", "stabilization", has(camera.optJSONArray("stabilizationModes"),1)||has(camera.optJSONArray("stabilizationModes"),2));
        String[] quality = {"Off", "Fast", "High quality", "Minimal", "Zero shutter lag"};
        mode("Noise reduction", "noiseReduction", "noiseModes", quality, true); mode("Sharpening", "edgeMode", "edgeModes", quality, true);
        mode("Chromatic correction", "aberrationMode", "aberrationModes", quality, true); mode("Lens correction", "lensCorrection", "lensCorrectionModes", quality, true);
    }
    void effects() {
        slider("Horizontal stretch", "stretchX", .25f, 4, 1, false); slider("Vertical stretch", "stretchY", .25f, 4, 1, false);
        slider("Distortion", "distortion", -.8f, .8f, 0, false); slider("Bulge", "bulge", -.8f, .8f, 0, false);
        slider("Bulge radius", "bulgeRadius", .05f, 1, .4f, false); slider("Center X", "bulgeX", 0, 1, .5f, false); slider("Center Y", "bulgeY", 0, 1, .5f, false);
        slider("Background blur", "backgroundBlur", 0, 32, 0, true); slider("Mask rate · fps", "maskFps", 2, 30, 10, true);
        menu("ML processor", List.of("Auto", "CPU", "GPU"), List.of("auto", "cpu", "gpu"), s -> s.optString("mlDelegate", "auto"), value -> patch(Json.object("mlDelegate", value), false));
    }
    static boolean has(JSONArray values, int value) { if (values != null) for (int i = 0; i < values.length(); i++) if (values.optInt(i) == value) return true; return false; }
    static MaterialButton icon(Context context, int resource, String label) {
        MaterialButton button = new MaterialButton(context, null, com.google.android.material.R.attr.materialButtonOutlinedStyle);
        button.setIconResource(resource); button.setIconSize(Math.round(24 * context.getResources().getDisplayMetrics().density)); button.setIconPadding(0); button.setIconGravity(MaterialButton.ICON_GRAVITY_TEXT_START);
        button.setContentDescription(label); button.setTooltipText(label); button.setMinWidth(0); button.setMinimumWidth(0); button.setPadding(0, 0, 0, 0); button.setInsetTop(0); button.setInsetBottom(0); return button;
    }
}
