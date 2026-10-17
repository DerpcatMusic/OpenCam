package dev.opencam;

import android.net.Uri;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.util.*;
import javax.crypto.Cipher;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;

/** Sends this phone's pairing link to the desktop that displayed a QR code, encrypted with the QR's one-time secret. */
final class PairClient {
    record Target(List<String> hosts, int port, byte[] secret) {
        boolean tailscale() { return hosts.stream().anyMatch(Auth::tailscale); }
    }

    static Target parse(String text) {
        Uri uri = Uri.parse(text == null ? "" : text.trim());
        String hosts = uri.getQueryParameter("a"), port = uri.getQueryParameter("p"), secret = uri.getQueryParameter("s");
        if (!"opencam-pair".equals(uri.getScheme()) || hosts == null || port == null || secret == null) throw new IllegalArgumentException("Not an OpenCam pairing code");
        List<String> list = new ArrayList<>();
        for (String host : hosts.split(",")) if (host.matches("[0-9.]{7,15}") && list.size() < 8) list.add(host);
        int number = Integer.parseInt(port);
        if (list.isEmpty() || number < 1 || number > 65535) throw new IllegalArgumentException("Not an OpenCam pairing code");
        return new Target(list, number, Auth.unhex(secret, 16));
    }

    /** Blocking; tries every desktop address in order and returns the one that accepted. */
    static String send(Target target, String name, String link) throws Exception {
        byte[] body = Auth.seal(target.secret, Json.object("name", name, "link", link).toString().getBytes(StandardCharsets.UTF_8));
        IOException last = null;
        for (String host : target.hosts) {
            try (Socket socket = new Socket()) {
                socket.connect(new InetSocketAddress(host, target.port), 3000);
                socket.setSoTimeout(5000);
                DataOutputStream out = new DataOutputStream(socket.getOutputStream());
                out.writeInt(body.length); out.write(body); out.flush();
                if (socket.getInputStream().read() == 1) return host;
                last = new IOException("Desktop rejected the pairing code");
            } catch (IOException e) { last = e; }
        }
        throw last;
    }
}
