package dev.opencam;

import android.content.Context;
import android.os.SystemClock;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.io.*;
import java.net.*;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.security.*;
import java.util.*;
import java.util.concurrent.*;
import javax.net.ssl.*;
import javax.security.auth.x500.X500Principal;
import org.json.JSONObject;

final class Bridge implements AutoCloseable {
    static final int PORT = 4937, MAX_PACKET = 16 * 1024 * 1024;
    record Packet(int kind, byte[] data) { }
    interface Listener {
        void command(JSONObject command);
        void disconnected();
        void status(String status);
    }
    final Listener listener;
    final String token, fingerprint;
    final SSLServerSocket server;
    final Auth.Credential credential;
    long retryAfter;
    android.net.nsd.NsdManager nsd;
    android.net.nsd.NsdManager.RegistrationListener registration;
    volatile boolean running = true;
    volatile Peer peer;
    volatile Runnable syncFrame = () -> { };

    Bridge(Context context, Listener listener, Auth.Credential credential) throws Exception {
        this.credential = credential;
        this.listener = listener;
        byte[] secret = new byte[16];
        new SecureRandom().nextBytes(secret);
        token = hex(secret);
        KeyStore keys = KeyStore.getInstance("AndroidKeyStore");
        keys.load(null);
        if (keys.containsAlias("opencam-server") && !keys.getCertificate("opencam-server").getPublicKey().getAlgorithm().equals("EC"))
            keys.deleteEntry("opencam-server");
        if (!keys.containsAlias("opencam-server")) {
            KeyPairGenerator generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore");
            generator.initialize(new KeyGenParameterSpec.Builder("opencam-server", KeyProperties.PURPOSE_SIGN | KeyProperties.PURPOSE_VERIFY)
                    // Conscrypt signs prehashed TLS messages through the non-exportable keystore key.
                    .setAlgorithmParameterSpec(new java.security.spec.ECGenParameterSpec("secp256r1"))
                    .setDigests(KeyProperties.DIGEST_NONE, KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA384, KeyProperties.DIGEST_SHA512)
                    .setCertificateSubject(new X500Principal("CN=OpenCam"))
                    .setCertificateNotAfter(new Date(4102444800000L)).build());
            generator.generateKeyPair();
        }
        fingerprint = hex(MessageDigest.getInstance("SHA-256").digest(keys.getCertificate("opencam-server").getEncoded()));
        KeyManagerFactory km = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm());
        km.init(keys, null);
        SSLContext tls = SSLContext.getInstance("TLS");
        tls.init(km.getKeyManagers(), null, null);
        server = (SSLServerSocket) tls.getServerSocketFactory().createServerSocket(PORT);
        server.setSoTimeout(1000);
        advertise(context);
        new Thread(this::accept, "opencam-accept").start();
    }

    void advertise(Context context) {
        nsd = context.getSystemService(android.net.nsd.NsdManager.class);
        android.net.nsd.NsdServiceInfo service = new android.net.nsd.NsdServiceInfo();
        service.setServiceName("OpenCam " + android.os.Build.MODEL);
        service.setServiceType("_opencam._tcp."); service.setPort(PORT);
        service.setAttribute("protocol","1"); service.setAttribute("pin",fingerprint);
        service.setAttribute("token",token); service.setAttribute("auth",credential == null ? "open" : "password");
        registration = new android.net.nsd.NsdManager.RegistrationListener() {
            public void onServiceRegistered(android.net.nsd.NsdServiceInfo info) { if (!running) try { nsd.unregisterService(this); } catch (RuntimeException ignored) { } }
            public void onRegistrationFailed(android.net.nsd.NsdServiceInfo info, int code) { listener.status("Discovery unavailable · use pairing link"); }
            public void onServiceUnregistered(android.net.nsd.NsdServiceInfo info) { }
            public void onUnregistrationFailed(android.net.nsd.NsdServiceInfo info, int code) { }
        };
        try { nsd.registerService(service,android.net.nsd.NsdManager.PROTOCOL_DNS_SD,registration); }
        catch (RuntimeException e) { registration = null; listener.status("Discovery unavailable · use pairing link"); }
    }

    static String hex(byte[] value) {
        StringBuilder s = new StringBuilder();
        for (byte b : value) s.append(String.format(Locale.ROOT, "%02x", b & 255));
        return s.toString();
    }

    void accept() {
        while (running) {
            SSLSocket accepted = null;
            try {
                SSLSocket socket = (SSLSocket) server.accept();
                accepted = socket;
                if (peer != null || SystemClock.elapsedRealtime() < retryAfter) { socket.close(); continue; }
                socket.setTcpNoDelay(true);
                socket.setSendBufferSize(128 * 1024);
                socket.setSoTimeout(10_000);
                socket.startHandshake();
                Peer candidate = new Peer(socket);
                JSONObject hello = candidate.readJson();
                if (!"hello".equals(hello.optString("type")) || hello.optInt("protocol") != 1
                        || !MessageDigest.isEqual(token.getBytes(java.nio.charset.StandardCharsets.UTF_8),
                        hello.optString("token").getBytes(java.nio.charset.StandardCharsets.UTF_8))) {
                    socket.close(); continue;
                }
                if (credential != null) {
                    byte[] challenge = Auth.random(32);
                    candidate.writeJson(Json.object("type","auth","salt",hex(credential.salt()),"challenge",hex(challenge),"iterations",Auth.ITERATIONS));
                    JSONObject response = candidate.readJson();
                    boolean valid = "auth".equals(response.optString("type")) && Auth.verify(credential.key(),challenge,Auth.unhex(fingerprint,32),response.optString("proof"));
                    if (!valid) {
                        retryAfter = SystemClock.elapsedRealtime() + 2000;
                        candidate.writeJson(Json.object("type","error","message","Password incorrect"));
                        socket.close(); continue;
                    }
                }
                socket.setSoTimeout(15_000);
                peer = candidate;
                candidate.start();
                listener.status("Desktop paired · encrypted connection");
                listener.command(hello);
            } catch (SocketTimeoutException timeout) { }
            catch (Exception e) { if (credential != null) retryAfter = SystemClock.elapsedRealtime() + 2000; if (running) listener.status("Pairing failed: " + e.getMessage()); }
            finally {
                if (accepted != null && (peer == null || peer.socket != accepted))
                    try { accepted.close(); } catch (IOException ignored) { }
            }
        }
    }

    final class Peer implements AutoCloseable {
        final SSLSocket socket;
        final DataInputStream input;
        final DataOutputStream output;
        final ArrayBlockingQueue<Packet> outgoing = new ArrayBlockingQueue<>(6);
        volatile boolean alive = true, waitingKeyframe = true;
        volatile long writeStarted;
        Peer(SSLSocket socket) throws IOException {
            this.socket = socket;
            input = new DataInputStream(socket.getInputStream());
            output = new DataOutputStream(socket.getOutputStream());
        }
        JSONObject readJson() throws Exception {
            int kind = input.readUnsignedByte(), size = input.readInt();
            if (kind != 1 || size < 2 || size > 65536) throw new IOException("Invalid control packet");
            byte[] body = new byte[size];
            input.readFully(body);
            return new JSONObject(new String(body, java.nio.charset.StandardCharsets.UTF_8));
        }
        void writeJson(JSONObject value) throws IOException {
            byte[] bytes = value.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
            output.writeByte(1); output.writeInt(bytes.length); output.write(bytes); output.flush();
        }
        void start() {
            new Thread(() -> {
                try { while (alive) listener.command(readJson()); }
                catch (Exception e) { if (alive) listener.status(e.getMessage() == null ? "Desktop disconnected" : "Desktop disconnected: " + e.getMessage()); }
                finally { close(); }
            }, "opencam-commands").start();
            new Thread(() -> {
                try {
                    while (alive) {
                        Packet p = outgoing.poll(1, TimeUnit.SECONDS);
                        if (p == null) continue;
                        writeStarted = SystemClock.elapsedRealtime();
                        output.writeByte(p.kind);
                        output.writeInt(p.data.length);
                        output.write(p.data);
                        output.flush();
                        writeStarted = 0;
                    }
                } catch (Exception e) { if (alive) listener.status("Transport interrupted: " + e.getMessage()); }
                finally { close(); }
            }, "opencam-video").start();
            new Thread(() -> {
                try {
                    while (alive) {
                        Thread.sleep(250);
                        if (writeStarted != 0 && SystemClock.elapsedRealtime() - writeStarted > 750) close();
                    }
                } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
            }, "opencam-watchdog").start();
        }
        synchronized void send(JSONObject value) {
            byte[] bytes = value.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
            if (bytes.length > MAX_PACKET || !outgoing.offer(new Packet(1, bytes))) close();
        }
        synchronized void video(byte[] data, long pts, int flags, byte[] config) {
            boolean key = (flags & android.media.MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0;
            long queuedVideo = outgoing.stream().filter(p -> p.kind == 2).count();
            if (queuedVideo >= 2) {
                outgoing.removeIf(p -> p.kind == 2);
                waitingKeyframe = true;
                syncFrame.run();
            }
            if (waitingKeyframe && !key) return;
            waitingKeyframe = false;
            byte[] header = key && config != null ? config : new byte[0];
            ByteBuffer packet = ByteBuffer.allocate(12 + header.length + data.length).order(ByteOrder.BIG_ENDIAN);
            packet.putLong(pts).putInt(flags).put(header).put(data);
            if (packet.capacity() > MAX_PACKET || !outgoing.offer(new Packet(2, packet.array()))) {
                waitingKeyframe = true;
                syncFrame.run();
            }
        }
        void resetVideo() {
            outgoing.removeIf(p -> p.kind == 2);
            waitingKeyframe = true;
        }
        @Override public synchronized void close() {
            if (!alive) return;
            alive = false;
            try { socket.close(); } catch (IOException ignored) { }
            if (peer == this) { peer = null; listener.disconnected(); }
        }
    }

    void send(JSONObject value) { Peer p = peer; if (p != null) p.send(value); }
    void video(byte[] data, long pts, int flags, byte[] config) { Peer p = peer; if (p != null) p.video(data, pts, flags, config); }
    void resetVideo() { Peer p = peer; if (p != null) p.resetVideo(); }
    @Override public void close() {
        running = false;
        if (registration != null) { try { nsd.unregisterService(registration); } catch (RuntimeException ignored) { } registration = null; }
        Peer p = peer;
        if (p != null) p.close();
        try { server.close(); } catch (IOException ignored) { }
    }
}
