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

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setContentView(R.layout.main);
        View content = findViewById(R.id.content);
        int pad = Math.round(24 * getResources().getDisplayMetrics().density);
        content.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            android.graphics.Insets ime = insets.getInsets(WindowInsets.Type.ime());
            view.setPadding(pad + bars.left, pad + bars.top, pad + bars.right, pad + Math.max(bars.bottom, ime.bottom));
            return insets;
        });
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
            Catalog catalog = new Catalog(getSystemService(CameraManager.class));
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
            controller = new CameraController(this, catalog, bridge);
            pairing.setText("opencam://" + address() + ":" + Bridge.PORT + "?token=" + bridge.token + "&pin=" + bridge.fingerprint);
            copy.setEnabled(true); start.setText("Disable connection");
            status.setText("Ready · " + (protect.isChecked() ? "password required" : "open on local network") + " · " + catalog.lenses.size() + " camera paths");
        } catch (Exception e) { disable(); status.setText("Could not enable camera: " + e.getMessage()); }
    }

    void disable() {
        if (bridge != null) { bridge.close(); bridge = null; }
        if (controller != null) { controller.close(); controller = null; }
        if (start != null) { start.setText("Enable connection"); copy.setEnabled(false); pairing.setText(""); status.setText("Camera access is off"); }
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request == 1 && results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) enable();
        else status.setText("Camera permission is needed. Enable it in Android app settings, then try again.");
    }
    @Override public void command(JSONObject command) { CameraController c = controller; if (c != null) c.command(command); }
    @Override public void disconnected() { CameraController c = controller; if (c != null) c.handler.post(c::stop); }
    @Override public void status(String value) { runOnUiThread(() -> { if (status != null) status.setText(value); }); }
    @Override public void onPause() { disable(); super.onPause(); }
    @Override public void onDestroy() { disable(); super.onDestroy(); }
}
