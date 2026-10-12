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
    TextView connectionDetail;
    Button status, copy, format, cameraAccess, savePassword;
    MaterialSwitch protect, available;
    TextInputLayout passwordField;
    EditText password;
    TextureView preview;
    ScrollView panel;
    CameraControls controls;
    volatile JSONObject settings, catalog;
    volatile long revision;
    int selectedTool = -1;
    boolean resumed, streaming;
    String pairing = "";
    JSONObject previewInfo;
    final android.hardware.display.DisplayManager.DisplayListener displays = new android.hardware.display.DisplayManager.DisplayListener() {
        public void onDisplayAdded(int id) { }
        public void onDisplayRemoved(int id) { }
        public void onDisplayChanged(int id) { if (preview.getDisplay() != null && preview.getDisplay().getDisplayId() == id) transform(); }
    };
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
        getSystemService(android.hardware.display.DisplayManager.class).registerDisplayListener(displays, new android.os.Handler(getMainLooper()));
        controls = new CameraControls(this, findViewById(R.id.camera_controls), this::apply, this::status,()->{CameraController active=controller;if(active!=null)active.local(Json.object("type","raw"));});
        LinearLayout rail = findViewById(R.id.tools);
        int[] icons = {R.drawable.ic_camera, R.drawable.ic_video, R.drawable.ic_settings, R.drawable.ic_effects};
        String[] labels = {"Camera and focus", "Resolution and frame rate", "Sensor controls", "Phone effects"};
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
        connectionDetail = findViewById(R.id.connection_detail);
        format = findViewById(R.id.format);
        cameraAccess = findViewById(R.id.camera_access);
        savePassword = findViewById(R.id.save_password);
        available = findViewById(R.id.available);
        protect = findViewById(R.id.protect);
        password = findViewById(R.id.password);
        passwordField = findViewById(R.id.password_field);
        copy = findViewById(R.id.copy);
        available.setChecked(getPreferences(MODE_PRIVATE).getBoolean("available",true));
        protect.setChecked(getPreferences(MODE_PRIVATE).getBoolean("protect",false));
        passwordField.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        savePassword.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        if (getPreferences(MODE_PRIVATE).contains("password-key")) passwordField.setPlaceholderText("Saved password");
        available.setOnCheckedChangeListener((button,checked) -> {
            disable(); getPreferences(MODE_PRIVATE).edit().putBoolean("available",checked).apply();
            if (checked) resumeCamera();
            refreshStatus();
        });
        protect.setOnCheckedChangeListener((button,checked) -> {
            disable(); getPreferences(MODE_PRIVATE).edit().putBoolean("protect",checked).apply();
            passwordField.setVisibility(checked ? View.VISIBLE : View.GONE);
            savePassword.setVisibility(checked ? View.VISIBLE : View.GONE);
            if (!checked) password.setText("");
            enable(); refreshStatus(); sizePanel();
        });
        password.addTextChangedListener(new android.text.TextWatcher() {
            public void beforeTextChanged(CharSequence s,int start,int count,int after) { }
            public void onTextChanged(CharSequence s,int start,int before,int count) { if (bridge != null) disable(); passwordField.setError(null); refreshStatus(); }
            public void afterTextChanged(android.text.Editable s) { }
        });
        status.setOnClickListener(v -> select(selectedTool == 4 ? -1 : 4));
        format.setOnClickListener(v -> select(selectedTool == 1 ? -1 : 1));
        cameraAccess.setOnClickListener(v -> requestCamera());
        savePassword.setOnClickListener(v -> {
            try {
                byte[] salt = Auth.random(16), key = Auth.derive(password.getText().toString(),salt);
                disable();
                getPreferences(MODE_PRIVATE).edit().putString("password-salt",Bridge.hex(salt)).putString("password-key",Bridge.hex(key)).apply();
                password.setText(""); passwordField.setPlaceholderText("Saved password");
                ((android.view.inputmethod.InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(password.getWindowToken(),0);
                enable();
            } catch (Exception e) { passwordField.setError(e.getMessage()); sizePanel(); }
        });
        copy.setOnClickListener(v -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            clipboard.setPrimaryClip(ClipData.newPlainText("OpenCam pairing", pairing));
            com.google.android.material.snackbar.Snackbar.make(content,"Pairing link copied",com.google.android.material.snackbar.Snackbar.LENGTH_SHORT).show();
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
        if (!resumed || bridge != null || !available.isChecked() || checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) return;
        try {
            camera();
            if (controller == null) throw new IllegalStateException("Enable camera permission first");
            Auth.Credential credential = null;
            if (protect.isChecked()) {
                var prefs = getPreferences(MODE_PRIVATE);
                if (password.length() > 0 || !prefs.contains("password-key")) { refreshStatus(); return; }
                credential = new Auth.Credential(Auth.unhex(prefs.getString("password-salt",""),16),Auth.unhex(prefs.getString("password-key",""),32));
            }
            bridge = new Bridge(this, this, credential);
            controller.attach(bridge);
            String endpoint = address() + ":" + Bridge.PORT;
            pairing = "opencam://" + endpoint + "?token=" + bridge.token + "&pin=" + bridge.fingerprint;
            connectionDetail.setText(endpoint); refreshStatus();
        } catch (Exception e) { disable(); status("Connection failed: " + e.getMessage()); }
    }

    void disable() {
        if (bridge != null) { bridge.close(); bridge = null; }
        if (controller != null) controller.attach(null);
        pairing = ""; streaming = false;
        if (copy != null) { connectionDetail.setText(""); refreshStatus(); }
    }

    void refreshStatus() {
        if (status == null) return;
        boolean cameraAllowed = checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED;
        boolean paired = bridge != null && bridge.peer != null && bridge.peer.alive;
        String label = !cameraAllowed || controller == null ? "No camera" : !available.isChecked() ? "Offline" :
            bridge == null ? (protect.isChecked() ? "Set password" : "Offline") : paired ? (streaming ? "Live" : "Connected") : "No desktop";
        status.setText(label); status.setContentDescription(label + ", connection settings");
        copy.setEnabled(bridge != null);
        cameraAccess.setVisibility(controller == null ? View.VISIBLE : View.GONE);
        cameraAccess.setText(cameraAllowed ? "Retry camera" : "Allow camera");
        format.setVisibility(controller != null && settings != null && selectedTool < 0 ? View.VISIBLE : View.GONE);
    }

    void requestCamera() {
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) resumeCamera();
        else if (getPreferences(MODE_PRIVATE).getBoolean("asked-camera", false) && !shouldShowRequestPermissionRationale(Manifest.permission.CAMERA))
            startActivity(new android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,android.net.Uri.parse("package:" + getPackageName())));
        else { getPreferences(MODE_PRIVATE).edit().putBoolean("asked-camera",true).apply(); requestPermissions(new String[]{Manifest.permission.CAMERA},1); }
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request != 1) return;
        if (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) resumeCamera();
        else { disable(); refreshStatus(); }
    }
    @Override public void command(JSONObject command) { CameraController c = controller; if (c != null) c.command(command); }
    @Override public void disconnected() { runOnUiThread(() -> { streaming = false; refreshStatus(); }); CameraController c = controller; if (c != null) c.handler.post(() -> { try { if (c.current != null && c.streaming) c.configure(c.current, false); } catch (Exception e) { c.fail(e); } }); }
    @Override public void status(String value) { runOnUiThread(() -> {
        if (!resumed || connectionDetail == null) return;
        connectionDetail.setText(value); refreshStatus();
        if (selectedTool != 4) com.google.android.material.snackbar.Snackbar.make(findViewById(R.id.content),value,com.google.android.material.snackbar.Snackbar.LENGTH_LONG).show();
    }); }
    @Override public void onResume() {
        super.onResume(); resumed = true;
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) resumeCamera();
        else if (!getPreferences(MODE_PRIVATE).getBoolean("asked-camera", false)) {
            getPreferences(MODE_PRIVATE).edit().putBoolean("asked-camera", true).apply();
            requestPermissions(new String[]{Manifest.permission.CAMERA}, 1);
        }
        refreshStatus();
    }
    @Override public void onPause() {
        resumed = false;
        if (controller != null) { controller.close(); controller = null; }
        disable(); super.onPause();
    }
    @Override public void onSaveInstanceState(Bundle out) {
        if (settings != null) out.putString("camera-settings", settings.toString()); super.onSaveInstanceState(out);
    }

    void resumeCamera() {
        if (!resumed) return;
        camera();
        enable(); refreshStatus();
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
                    if (event.optJSONObject("preview") != null) previewInfo = event.getJSONObject("preview");
                    controls.state(catalog, settings, selectedTool); transform();
                }
                switch (event.optString("type")) {
                    case "preview", "configured", "controls", "state" -> {
                        streaming = event.optBoolean("streaming");
                        format.setText(settings.optInt("width") + " × " + settings.optInt("height") + " · " + settings.optInt("fps") + " fps");
                        format.setContentDescription(format.getText() + ", resolution and frame rate");
                        refreshStatus();
                        if (event.optBoolean("conflict")) status("Settings changed on the other device · try again");
                    }
                    case "error" -> { status(event.optString("message")); if (settings != null) controls.state(catalog, settings, selectedTool); }
                    case "raw_saved", "raw_transfer_error" -> status(event.optString("message"));
                }
                sizePanel();
            } catch (Exception e) { status(e.getMessage()); }
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
            requestCamera();
        refreshStatus(); panel.scrollTo(0, 0); sizePanel();
    }
    void sizePanel() {
        if (panel == null || findViewById(R.id.content).getHeight() == 0) return;
        View child = panel.getChildAt(0);
        child.measure(View.MeasureSpec.makeMeasureSpec(Math.max(0, panel.getWidth()), View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        int height = Math.min(child.getMeasuredHeight(), (findViewById(R.id.content).getHeight() - findViewById(R.id.content).getPaddingTop() - findViewById(R.id.content).getPaddingBottom()) / 2);
        if (panel.getLayoutParams().height != height) { panel.getLayoutParams().height = height; panel.requestLayout(); }
    }
    void transform() {
        if (previewInfo == null || previewInfo.optInt("width") <= 0 || previewInfo.optInt("height") <= 0 || preview.getDisplay() == null || preview.getWidth() == 0 || preview.getHeight() == 0) return;
        preview.setTransform(previewMatrix(preview.getWidth(), preview.getHeight(), previewInfo.optInt("width"), previewInfo.optInt("height"),
            previewInfo.optInt("orientation"), preview.getDisplay().getRotation() * 90, previewInfo.optBoolean("processed")));
    }
    static android.graphics.Matrix previewMatrix(float vw, float vh, float w, float h, int sensor, int display, boolean processed) {
        // Camera2 already rotates direct TextureView buffers for the sensor; EGL output has explicit dimensions.
        if (!processed && sensor % 180 != 0) { float swap = w; w = h; h = swap; }
        float scale = display % 180 == 0 ? Math.min(vw / w, vh / h) : Math.min(vw / h, vh / w);
        android.graphics.Matrix matrix = new android.graphics.Matrix();
        matrix.setScale(w * scale / vw, h * scale / vh, vw / 2, vh / 2);
        matrix.postRotate(-display, vw / 2, vh / 2);
        return matrix;
    }
    @Override public void onDestroy() { getSystemService(android.hardware.display.DisplayManager.class).unregisterDisplayListener(displays); disable(); super.onDestroy(); }
}
