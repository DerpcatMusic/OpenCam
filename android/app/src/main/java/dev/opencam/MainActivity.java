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
import androidx.core.widget.NestedScrollView;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import com.google.android.material.bottomsheet.BottomSheetBehavior;
import com.google.android.material.color.DynamicColors;
import java.net.*;
import java.util.*;
import org.json.JSONObject;

public final class MainActivity extends AppCompatActivity implements Bridge.Listener {
    Bridge bridge;
    CameraController controller;
    TextView connectionDetail, connectionState, cameraState, sheetTitle, emptyTitle, emptyDetail, videoSummary;
    View cameraEmpty, sheetRoot, passwordHelp, connectionControlsView, cameraControlsView;
    BottomSheetDialog sheet;
    Button discardPassword;
    int restoredTool = -1, restoredScroll;
    String cameraFailure;
    PasswordDraft passwordDraft;
    public static final class PasswordDraft extends androidx.lifecycle.ViewModel {
        String value = "";
        @Override protected void onCleared() { value = ""; }
    }
    Button status, copy, format, cameraAccess, savePassword;
    MaterialSwitch protect, available;
    TextInputLayout passwordField;
    EditText password;
    TextureView preview;
    NestedScrollView panel;
    CameraControls controls;
    volatile JSONObject settings, catalog;
    volatile long revision;
    int selectedTool = -1;
    boolean resumed, streaming, previewReady;
    String pairing = "";
    JSONObject previewInfo;
    final android.hardware.display.DisplayManager.DisplayListener displays = new android.hardware.display.DisplayManager.DisplayListener() {
        public void onDisplayAdded(int id) { }
        public void onDisplayRemoved(int id) { }
        public void onDisplayChanged(int id) { if (preview.getDisplay() != null && preview.getDisplay().getDisplayId() == id) transform(); }
    };
    final java.util.List<com.google.android.material.button.MaterialButton> tools = new ArrayList<>();

    @Override public void onCreate(Bundle state) {
        DynamicColors.applyToActivityIfAvailable(this);
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setContentView(R.layout.main);
        View content = findViewById(R.id.content);
        passwordDraft = new androidx.lifecycle.ViewModelProvider(this).get(PasswordDraft.class);
        boolean lightSurface = com.google.android.material.color.MaterialColors.isColorLight(
            com.google.android.material.color.MaterialColors.getColor(content,com.google.android.material.R.attr.colorSurface));
        WindowCompat.getInsetsController(getWindow(),content).setAppearanceLightStatusBars(lightSurface);
        WindowCompat.getInsetsController(getWindow(),content).setAppearanceLightNavigationBars(lightSurface);
        int pad = getResources().getDimensionPixelSize(R.dimen.content_gutter);
        ViewCompat.setOnApplyWindowInsetsListener(content, (view, insets) -> {
            androidx.core.graphics.Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() | WindowInsetsCompat.Type.displayCutout());
            androidx.core.graphics.Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());
            view.setPadding(pad + bars.left, pad + bars.top, pad + bars.right, pad + Math.max(bars.bottom, ime.bottom));
            return insets;
        });
        ViewCompat.requestApplyInsets(content);
        preview = findViewById(R.id.preview);
        cameraEmpty = findViewById(R.id.camera_empty);
        emptyTitle = findViewById(R.id.camera_empty_title);
        emptyDetail = findViewById(R.id.camera_empty_detail);
        connectionState = findViewById(R.id.connection_state);
        cameraState = findViewById(R.id.camera_state);
        getSystemService(android.hardware.display.DisplayManager.class).registerDisplayListener(displays, new android.os.Handler(getMainLooper()));
        sheet = new BottomSheetDialog(this);
        sheetRoot = getLayoutInflater().cloneInContext(sheet.getContext()).inflate(R.layout.settings_sheet, new FrameLayout(sheet.getContext()), false);
        sheet.setContentView(sheetRoot);
        ViewCompat.setOnApplyWindowInsetsListener(sheetRoot,(view,insets) -> {
            sheetRoot.findViewById(R.id.drag_handle).setVisibility(insets.isVisible(WindowInsetsCompat.Type.ime())?View.GONE:View.VISIBLE);
            return insets;
        });
        sheet.getBehavior().setFitToContents(true);
        sheet.getBehavior().setSkipCollapsed(true);
        sheet.setOnDismissListener(dialog -> {
            selectedTool = -1; panel.setVisibility(View.GONE);
            for (var button : tools) button.setChecked(false);
            refreshStatus();
        });
        panel = sheetRoot.findViewById(R.id.panel);
        sheetTitle = sheetRoot.findViewById(R.id.sheet_title);
        cameraControlsView = sheetRoot.findViewById(R.id.camera_controls);
        connectionControlsView = sheetRoot.findViewById(R.id.connection_controls);
        controls = new CameraControls(sheetRoot.getContext(), (LinearLayout)cameraControlsView, this::apply, this::status, () -> {
            CameraController active=controller; if(active!=null) active.local(Json.object("type","raw"));
        });
        int[] ids = {R.id.tool_camera, R.id.tool_video, R.id.tool_sensor, R.id.tool_effects};
        for (int i = 0; i < ids.length; i++) {
            final int index = i;
            com.google.android.material.button.MaterialButton button = findViewById(ids[i]);
            button.setCheckable(true); tools.add(button);
            button.setOnClickListener(v -> select(selectedTool == index ? -1 : index));
        }
        content.addOnLayoutChangeListener((v,l,t,r,b,ol,ot,or,ob) -> transform());
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
        if (state != null) {
            if (state.containsKey("camera-settings")) try { settings = new JSONObject(state.getString("camera-settings")); } catch (Exception ignored) { }
            restoredTool = state.getInt("selected-tool", -1); restoredScroll = state.getInt("panel-scroll", 0);
            controls.restoreEditor(state);
        }
        status = findViewById(R.id.status);
        connectionDetail = sheetRoot.findViewById(R.id.connection_detail);
        format = findViewById(R.id.tool_video);
        videoSummary = findViewById(R.id.video_summary);
        cameraAccess = findViewById(R.id.camera_access);
        savePassword = sheetRoot.findViewById(R.id.save_password);
        available = sheetRoot.findViewById(R.id.available);
        protect = sheetRoot.findViewById(R.id.protect);
        password = sheetRoot.findViewById(R.id.password);
        passwordField = sheetRoot.findViewById(R.id.password_field);
        copy = sheetRoot.findViewById(R.id.copy);
        passwordHelp = sheetRoot.findViewById(R.id.password_help);
        discardPassword = sheetRoot.findViewById(R.id.discard_password);
        sheetRoot.findViewById(R.id.close_settings).setOnClickListener(v -> select(-1));
        available.setChecked(getPreferences(MODE_PRIVATE).getBoolean("available",true));
        protect.setChecked(getPreferences(MODE_PRIVATE).getBoolean("protect",false));
        passwordField.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        savePassword.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        passwordHelp.setVisibility(protect.isChecked() ? View.VISIBLE : View.GONE);
        if (getPreferences(MODE_PRIVATE).contains("password-key")) passwordField.setPlaceholderText(getString(R.string.saved_password));
        available.setOnCheckedChangeListener((button,checked) -> {
            disable(); getPreferences(MODE_PRIVATE).edit().putBoolean("available",checked).apply();
            if (checked) resumeCamera();
            refreshStatus();
        });
        protect.setOnCheckedChangeListener((button,checked) -> {
            disable(); getPreferences(MODE_PRIVATE).edit().putBoolean("protect",checked).apply();
            passwordField.setVisibility(checked ? View.VISIBLE : View.GONE);
            savePassword.setVisibility(checked ? View.VISIBLE : View.GONE);
            passwordHelp.setVisibility(checked ? View.VISIBLE : View.GONE);
            if (!checked) password.setText("");
            enable(); refreshStatus();
        });
        password.addTextChangedListener(new android.text.TextWatcher() {
            public void beforeTextChanged(CharSequence s,int start,int count,int after) { }
            public void onTextChanged(CharSequence s,int start,int before,int count) {
                passwordDraft.value = s.toString();
                if (bridge != null) disable(); passwordField.setError(null);
                discardPassword.setVisibility(s.length() > 0 && getPreferences(MODE_PRIVATE).contains("password-key") ? View.VISIBLE : View.GONE);
                refreshStatus();
            }
            public void afterTextChanged(android.text.Editable s) { }
        });
        status.setOnClickListener(v -> select(selectedTool == 4 ? -1 : 4));
        format.setOnClickListener(v -> select(selectedTool == 1 ? -1 : 1));
        cameraAccess.setOnClickListener(v -> requestCamera());
        discardPassword.setOnClickListener(v -> { password.setText(""); enable(); refreshStatus(); });
        password.setOnEditorActionListener((v, action, event) -> {
            if (action == android.view.inputmethod.EditorInfo.IME_ACTION_DONE) { savePassword.performClick(); return true; }
            return false;
        });
        savePassword.setOnClickListener(v -> {
            try {
                byte[] salt = Auth.random(16), key = Auth.derive(password.getText().toString(),salt);
                disable();
                getPreferences(MODE_PRIVATE).edit().putString("password-salt",Bridge.hex(salt)).putString("password-key",Bridge.hex(key)).apply();
                password.setText(""); passwordField.setPlaceholderText(getString(R.string.saved_password));
                ((android.view.inputmethod.InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(password.getWindowToken(),0);
                enable();
            } catch (Exception e) {
                passwordField.setError(getString(e instanceof IllegalArgumentException?R.string.password_length_error:R.string.password_save_failed));
                passwordField.post(() -> {
                    password.requestFocus();
                    passwordField.requestRectangleOnScreen(new android.graphics.Rect(0,0,passwordField.getWidth(),passwordField.getHeight()),false);
                });
            }
        });
        copy.setOnClickListener(v -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            clipboard.setPrimaryClip(ClipData.newPlainText("OpenCam pairing", pairing));
            com.google.android.material.snackbar.Snackbar.make(content,getString(R.string.pairing_link_copied),com.google.android.material.snackbar.Snackbar.LENGTH_SHORT).show();
        });
        if (!passwordDraft.value.isEmpty()) password.setText(passwordDraft.value);
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
        } catch (Exception e) { disable(); status(getString(R.string.connection_failed,e.getMessage())); }
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
        int label = !cameraAllowed ? R.string.camera_access_needed : !previewReady ? (cameraFailure==null?R.string.starting_preview:R.string.camera_unavailable) : !available.isChecked() ? R.string.preview_only :
            bridge == null ? (protect.isChecked() ? R.string.password_not_saved : R.string.preview_only) :
            paired ? (streaming ? R.string.streaming_to_desktop : R.string.desktop_connected) : R.string.ready_to_connect;
        connectionState.setText(label);
        status.setContentDescription(getString(R.string.connection_description, getString(label)));
        int detail = bridge == null && protect.isChecked() && available.isChecked() ? R.string.discovery_paused :
            paired && streaming ? R.string.controls_synced : R.string.local_preview;
        cameraState.setText(detail);
        copy.setEnabled(bridge != null);
        cameraEmpty.setVisibility(previewReady ? View.GONE : View.VISIBLE);
        emptyTitle.setText(!cameraAllowed ? R.string.permission_title : cameraFailure!=null ? R.string.camera_unavailable : R.string.starting_preview);
        emptyDetail.setText(!cameraAllowed ? R.string.permission_detail : R.string.camera_retry_detail);
        emptyDetail.setVisibility(!cameraAllowed || cameraFailure!=null ? View.VISIBLE : View.GONE);
        if (cameraAllowed && cameraFailure != null) emptyDetail.setText(cameraFailure + " " + getString(R.string.camera_retry_detail));
        cameraAccess.setText(cameraAllowed ? R.string.retry_camera : R.string.allow_camera);
        cameraAccess.setVisibility(!cameraAllowed || cameraFailure!=null ? View.VISIBLE : View.GONE);
        videoSummary.setVisibility(controller != null && settings != null ? View.VISIBLE : View.GONE);
    }

    void requestCamera() {
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            if (controller != null && controller.session == null) { disable(); controller.close(); controller = null; }
            cameraFailure = null; resumeCamera();
        }
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
        if (selectedTool != 4) com.google.android.material.snackbar.Snackbar.make(sheet.isShowing()?sheetRoot:findViewById(R.id.content),value,com.google.android.material.snackbar.Snackbar.LENGTH_LONG).show();
    }); }
    @Override public void onResume() {
        super.onResume(); resumed = true;
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) resumeCamera();
        else if (!getPreferences(MODE_PRIVATE).getBoolean("asked-camera", false)) {
            getPreferences(MODE_PRIVATE).edit().putBoolean("asked-camera", true).apply();
            requestPermissions(new String[]{Manifest.permission.CAMERA}, 1);
        }
        refreshStatus();
        if (restoredTool >= 0) {
            int tool = restoredTool; restoredTool = -1; select(tool);
            panel.post(() -> panel.scrollTo(0, restoredScroll));
        }
    }
    @Override public void onPause() {
        resumed = false; previewReady = false;
        if (controller != null) { controller.close(); controller = null; }
        disable(); super.onPause();
    }
    @Override public void onSaveInstanceState(Bundle out) {
        if (settings != null) out.putString("camera-settings", settings.toString());
        out.putInt("selected-tool", selectedTool); out.putInt("panel-scroll", panel.getScrollY());
        controls.saveEditor(out); super.onSaveInstanceState(out);
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
            previewReady = false;
            controller = new CameraController(this, capabilities, this::cameraEvent);
            if (settings != null && capabilities.lenses.containsKey(settings.optString("camera"))) controller.current = CaptureSettings.copy(settings);
            if (preview.isAvailable()) controller.preview(preview.getSurfaceTexture());
        } catch (Exception e) { cameraFailure = getString(R.string.camera_start_failed,e.getMessage()); status(cameraFailure); }
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
                        if (controller != null && controller.session != null) { previewReady = true; cameraFailure = null; }
                        streaming = event.optBoolean("streaming");
                        videoSummary.setText(getString(R.string.format_summary,settings.optInt("width"),settings.optInt("height"),settings.optInt("fps")));
                        format.setContentDescription(getString(R.string.format_description,videoSummary.getText()));
                        refreshStatus();
                        if (event.optBoolean("conflict")) status(getString(R.string.settings_conflict));
                    }
                    case "error" -> {
                        if (controller != null && controller.session == null) { previewReady = false; cameraFailure = event.optString("message"); }
                        status(event.optString("message")); if (settings != null) controls.state(catalog, settings, selectedTool);
                    }
                    case "raw_saved", "raw_transfer_error" -> status(event.optString("message"));
                }
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
        if (tool < 0) { sheet.dismiss(); selectedTool = -1; panel.setVisibility(View.GONE); refreshStatus(); return; }
        selectedTool = tool;
        for (int i = 0; i < tools.size(); i++) tools.get(i).setChecked(i == tool);
        int[] titles = {R.string.camera,R.string.video,R.string.exposure,R.string.effects,R.string.connection};
        sheetTitle.setText(titles[tool]);
        panel.setVisibility(View.VISIBLE);
        connectionControlsView.setVisibility(tool == 4 ? View.VISIBLE : View.GONE);
        cameraControlsView.setVisibility(tool < 4 ? View.VISIBLE : View.GONE);
        if (settings != null) controls.state(catalog,settings,tool);
        else if (tool < 4) controls.unavailable();
        if (tool < 4 && checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) requestCamera();
        refreshStatus(); panel.scrollTo(0,0);
        if (!sheet.isShowing()) {
            sheet.show();
            if (sheet.getWindow() != null) {
                sheet.getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
                boolean lightSurface=com.google.android.material.color.MaterialColors.isColorLight(
                    com.google.android.material.color.MaterialColors.getColor(sheetRoot,com.google.android.material.R.attr.colorSurface));
                WindowCompat.getInsetsController(sheet.getWindow(),sheetRoot).setAppearanceLightStatusBars(lightSurface);
                WindowCompat.getInsetsController(sheet.getWindow(),sheetRoot).setAppearanceLightNavigationBars(lightSurface);
            }
        }
        sheet.getBehavior().setState(BottomSheetBehavior.STATE_EXPANDED);
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
    @Override public void onDestroy() {
        getSystemService(android.hardware.display.DisplayManager.class).unregisterDisplayListener(displays);
        sheet.setOnDismissListener(null); sheet.dismiss(); disable(); super.onDestroy();
    }
}
