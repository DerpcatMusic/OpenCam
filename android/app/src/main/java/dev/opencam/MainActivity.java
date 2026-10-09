package dev.opencam;

import android.Manifest;
import androidx.appcompat.app.AppCompatActivity;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.textfield.TextInputLayout;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.pm.PackageManager;
import android.hardware.camera2.CameraManager;
import android.os.Bundle;
import android.view.*;
import android.widget.*;
import java.net.*;
import java.util.*;
import org.json.JSONObject;

public final class MainActivity extends AppCompatActivity implements Bridge.Listener {
    Bridge bridge;
    CameraController controller;
    TextView status, pairing;
    Button start, copy;
    MaterialSwitch protect;
    TextInputLayout passwordField;
    EditText password;
    TextureView preview;
    ScrollView panel;
    CameraControls controls;
    volatile JSONObject settings, catalog;
    volatile long revision;
    int selectedTool = -1;
    boolean resumed;
    final java.util.List<com.google.android.material.button.MaterialButton> tools = new ArrayList<>();

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setContentView(R.layout.main);
        View content = findViewById(R.id.content);
        int pad = Math.round(8 * getResources().getDisplayMetrics().density);
        content.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            android.graphics.Insets ime = insets.getInsets(WindowInsets.Type.ime());
            view.setPadding(pad + bars.left, pad + bars.top, pad + bars.right, pad + Math.max(bars.bottom, ime.bottom));
            return insets;
        });
        panel = findViewById(R.id.panel);
        preview = findViewById(R.id.preview);
        controls = new CameraControls(this, findViewById(R.id.camera_controls), this::apply, this::status);
        LinearLayout rail = findViewById(R.id.tools);
        int[] icons = {R.drawable.ic_camera, R.drawable.ic_video, R.drawable.ic_settings, R.drawable.ic_effects, R.drawable.ic_wifi};
        String[] labels = {"Camera and focus", "Resolution and frame rate", "Sensor controls", "Phone effects", "Connection"};
        for (int i = 0; i < icons.length; i++) {
            final int index = i;
            var button = CameraControls.icon(this, icons[i], labels[i]);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(controls.dp(48), controls.dp(48)); params.bottomMargin = controls.dp(8);
            button.setCheckable(true); rail.addView(button, params); tools.add(button); button.setOnClickListener(v -> select(selectedTool == index ? -1 : index));
        }
        content.addOnLayoutChangeListener((v,l,t,r,b,ol,ot,or,ob) -> { sizePanel(); transform(); });
        preview.setSurfaceTextureListener(new TextureView.SurfaceTextureListener() {
            public void onSurfaceTextureAvailable(android.graphics.SurfaceTexture texture, int w, int h) { if (controller != null) controller.preview(texture); }
            public void onSurfaceTextureSizeChanged(android.graphics.SurfaceTexture texture, int w, int h) { transform(); }
            public boolean onSurfaceTextureDestroyed(android.graphics.SurfaceTexture texture) {
                if (controller != null) { controller.preview(null); return false; }
                return true;
            }
            public void onSurfaceTextureUpdated(android.graphics.SurfaceTexture texture) { }
        });
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            public void handleOnBackPressed() { if (selectedTool >= 0) select(-1); else { setEnabled(false); getOnBackPressedDispatcher().onBackPressed(); } }
        });
        if (state != null && state.containsKey("camera-settings")) try { settings = new JSONObject(state.getString("camera-settings")); } catch (Exception ignored) { }
        status = findViewById(R.id.status);
        pairing = findViewById(R.id.pairing);
        protect = findViewById(R.id.protect);
        password = findViewById(R.id.password);
        passwordField = findViewById(R.id.password_field);
        start = findViewById(R.id.start);
        copy = findViewById(R.id.copy);
        protect.setChecked(getPreferences(MODE_PRIVATE).getBoolean("protect",false));
        passwordField.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        if (getPreferences(MODE_PRIVATE).contains("password-key")) passwordField.setPlaceholderText("Saved password");
        protect.setOnCheckedChangeListener((button,checked) -> {
            disable(); getPreferences(MODE_PRIVATE).edit().putBoolean("protect",checked).apply();
            passwordField.setVisibility(checked ? View.VISIBLE : View.GONE);
        });
        password.addTextChangedListener(new android.text.TextWatcher() {
            public void beforeTextChanged(CharSequence s,int start,int count,int after) { }
            public void onTextChanged(CharSequence s,int start,int before,int count) { if (bridge != null) disable(); }
            public void afterTextChanged(android.text.Editable s) { }
        });
        start.setOnClickListener(v -> {
            if (bridge != null) disable();
            else if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED)
                requestPermissions(new String[]{Manifest.permission.CAMERA}, 1);
            else enable();
        });
        copy.setOnClickListener(v -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            clipboard.setPrimaryClip(ClipData.newPlainText("OpenCam pairing", pairing.getText()));
            status.setText("Pairing link copied");
        });
    }

    String address() throws SocketException {
        for (NetworkInterface iface : Collections.list(NetworkInterface.getNetworkInterfaces())) {
            if (!iface.isUp() || iface.isLoopback()) continue;
            for (InetAddress ip : Collections.list(iface.getInetAddresses()))
                if (ip instanceof Inet4Address && ip.isSiteLocalAddress()) return ip.getHostAddress();
        }
        return "127.0.0.1";
    }

    void enable() {
        try {
            camera();
            if (controller == null) throw new IllegalStateException("Enable camera permission first");
            Auth.Credential credential = null;
            if (protect.isChecked()) {
                String value = password.getText().toString();
                var prefs = getPreferences(MODE_PRIVATE);
                if (!value.isEmpty()) {
                    byte[] salt = Auth.random(16), key = Auth.derive(value,salt);
                    prefs.edit().putString("password-salt",Bridge.hex(salt)).putString("password-key",Bridge.hex(key)).apply();
                    credential = new Auth.Credential(salt,key);
                } else if (prefs.contains("password-key")) {
                    credential = new Auth.Credential(Auth.unhex(prefs.getString("password-salt",""),16),Auth.unhex(prefs.getString("password-key",""),32));
                } else throw new IllegalArgumentException("Set a password before enabling the connection");
            }
            bridge = new Bridge(this, this, credential);
            controller.attach(bridge);
            pairing.setText("opencam://" + address() + ":" + Bridge.PORT + "?token=" + bridge.token + "&pin=" + bridge.fingerprint);
            copy.setEnabled(true); start.setText("Disable connection");
            status.setText("Ready · " + (protect.isChecked() ? "password required" : "open on local network") + " · " + catalog.getJSONArray("cameras").length() + " camera paths");
        } catch (Exception e) { disable(); status.setText("Could not enable camera: " + e.getMessage()); }
    }

    void disable() {
        if (bridge != null) { bridge.close(); bridge = null; }
        if (controller != null) controller.attach(null);
        if (start != null) { start.setText("Enable connection"); copy.setEnabled(false); pairing.setText(""); status.setText("Local preview · connection off"); }
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request == 1 && results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) camera();
        else status.setText("Camera permission is needed. Enable it in Android app settings, then try again.");
    }
    @Override public void command(JSONObject command) { CameraController c = controller; if (c != null) c.command(command); }
    @Override public void disconnected() { CameraController c = controller; if (c != null) c.handler.post(() -> { try { if (c.current != null && c.streaming) c.configure(c.current, false); } catch (Exception e) { c.fail(e); } }); }
    @Override public void status(String value) { runOnUiThread(() -> { if (status != null) status.setText(value); }); }
    @Override public void onResume() {
        super.onResume(); resumed = true;
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) camera();
        else if (!getPreferences(MODE_PRIVATE).getBoolean("asked-camera", false)) {
            getPreferences(MODE_PRIVATE).edit().putBoolean("asked-camera", true).apply();
            requestPermissions(new String[]{Manifest.permission.CAMERA}, 1);
        }
    }
    @Override public void onPause() {
        resumed = false;
        if (controller != null) { controller.close(); controller = null; }
        disable(); super.onPause();
    }
    @Override public void onSaveInstanceState(Bundle out) {
        if (settings != null) out.putString("camera-settings", settings.toString()); super.onSaveInstanceState(out);
    }

    void camera() {
        if (!resumed || controller != null) return;
        try {
            Catalog capabilities = new Catalog(getSystemService(CameraManager.class)); catalog = capabilities.export();
            controller = new CameraController(this, capabilities, this::cameraEvent);
            if (settings != null && capabilities.lenses.containsKey(settings.optString("camera"))) controller.current = CaptureSettings.copy(settings);
            if (preview.isAvailable()) controller.preview(preview.getSurfaceTexture());
        } catch (Exception e) { status("Camera could not start: " + e.getMessage()); }
    }
    void cameraEvent(JSONObject event) {
        runOnUiThread(() -> {
            if (!resumed) return;
            try {
                if (event.optJSONObject("settings") != null) {
                    settings = event.getJSONObject("settings"); revision = event.optLong("revision", revision);
                    controls.state(catalog, settings, selectedTool); transform();
                }
                switch (event.optString("type")) {
                    case "preview", "configured", "controls", "state" -> {
                        status.setText(event.optBoolean("conflict") ? "Settings changed on the other device · try again" :
                            (event.optBoolean("streaming") ? "Streaming" : "Local preview") + " · " + settings.optInt("width") + " × " + settings.optInt("height") + " · " + settings.optInt("fps") + " fps");
                    }
                    case "error" -> { status.setText(event.optString("message")); if (settings != null) controls.state(catalog, settings, selectedTool); }
                    case "raw_saved" -> status.setText(event.optString("message"));
                }
                sizePanel();
            } catch (Exception e) { status.setText(e.getMessage()); }
        });
    }
    void apply(JSONObject values, boolean configure) {
        if (controller == null || settings == null) return;
        try {
            JSONObject next = values;
            if (configure) {
                next = CaptureSettings.copy(settings);
                for (Iterator<String> keys = values.keys(); keys.hasNext();) { String key = keys.next(); next.put(key, values.get(key)); }
            }
            JSONObject command = Json.object("type", configure ? "configure" : "controls", "settings", next);
            if (configure) command.put("baseRevision", revision);
            controller.local(command);
        } catch (Exception e) { status(e.getMessage()); }
    }
    void select(int tool) {
        selectedTool = tool;
        for (int i = 0; i < tools.size(); i++) tools.get(i).setChecked(i == tool);
        panel.setVisibility(tool < 0 ? View.GONE : View.VISIBLE);
        findViewById(R.id.connection_controls).setVisibility(tool == 4 ? View.VISIBLE : View.GONE);
        findViewById(R.id.camera_controls).setVisibility(tool < 4 ? View.VISIBLE : View.GONE);
        if (settings != null) controls.state(catalog, settings, tool);
        if (tool < 4 && tool >= 0 && checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED)
            requestPermissions(new String[]{Manifest.permission.CAMERA}, 1);
        panel.scrollTo(0, 0); sizePanel();
    }
    void sizePanel() {
        if (panel == null || findViewById(R.id.content).getHeight() == 0) return;
        View child = panel.getChildAt(0);
        child.measure(View.MeasureSpec.makeMeasureSpec(Math.max(0, panel.getWidth()), View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        int height = Math.min(child.getMeasuredHeight(), (findViewById(R.id.content).getHeight() - findViewById(R.id.content).getPaddingTop() - findViewById(R.id.content).getPaddingBottom()) / 2);
        if (panel.getLayoutParams().height != height) { panel.getLayoutParams().height = height; panel.requestLayout(); }
    }
    void transform() {
        if (settings == null || preview.getWidth() == 0 || preview.getHeight() == 0) return;
        float w = PhoneProcessor.outputWidth(settings), h = PhoneProcessor.outputHeight(settings);
        float vw = preview.getWidth(), vh = preview.getHeight(), scale = Math.min(vw / w, vh / h);
        android.graphics.Matrix matrix = new android.graphics.Matrix();
        matrix.setScale(w * scale / vw, h * scale / vh, vw / 2, vh / 2); preview.setTransform(matrix);
    }
    @Override public void onDestroy() { disable(); super.onDestroy(); }
}
