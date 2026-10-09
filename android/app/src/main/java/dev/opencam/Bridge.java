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
    record Pixels(byte[] data, long pts, int dataSpace, long epoch) { }
    static final class RawTransfer implements AutoCloseable {
        final InputStream input; final byte[] id; final String text; final long size;
        final MessageDigest digest; long offset;
        RawTransfer(InputStream input, long size) throws Exception {
            this.input=input; this.size=size; id=Auth.random(16); text=hex(id); digest=MessageDigest.getInstance("SHA-256");
        }
        public void close() { try { input.close(); } catch (IOException ignored) { } }
    }
    interface Listener {
        void command(JSONObject command);
        void disconnected();
        void status(String status);
    }
    final Listener listener;
    final Context context;
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
        this.context = context;
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
        final ArrayBlockingQueue<Packet> outgoing = new ArrayBlockingQueue<>(32);
        final ArrayBlockingQueue<Packet> encoded = new ArrayBlockingQueue<>(2);
        final Semaphore ready = new Semaphore(0);
        Pixels pixels;
        RawTransfer raw;
        volatile boolean writingPixels;
        volatile long videoEpoch;
        volatile boolean alive = true, waitingKeyframe = true;
        volatile long writeStarted;
        volatile long lastCommand;
        Peer(SSLSocket socket) throws IOException {
            this.socket = socket;
            input = new DataInputStream(new BufferedInputStream(socket.getInputStream(), 65536));
            output = new DataOutputStream(new BufferedOutputStream(socket.getOutputStream(), 128 * 1024));
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
            lastCommand=SystemClock.elapsedRealtime();
            new Thread(() -> {
                try { while (alive) {
                    JSONObject command = readJson();lastCommand=SystemClock.elapsedRealtime();
                    if (command.optString("type").equals("ping")) send(Json.object("type","pong","sent",command.optDouble("sent"),"phoneUs",SystemClock.elapsedRealtimeNanos()/1000));
                    else listener.command(command);
                } }
                catch (Exception e) { if (alive) listener.status(e.getMessage() == null ? "Desktop disconnected" : "Desktop disconnected: " + e.getMessage()); }
                finally { close(); }
            }, "opencam-commands").start();
            new Thread(() -> {
                try {
                    byte[] chunk = new byte[65536];
                    while (alive) {
                        ready.tryAcquire(1, TimeUnit.SECONDS); ready.drainPermits();
                        Packet p = outgoing.poll();
                        if (p == null) p = encoded.poll();
                        if (p != null) { writePacket(p); writeRawChunk(chunk); wake(); continue; }
                        Pixels frame;
                        synchronized (this) { frame = pixels; pixels = null; writingPixels = frame != null; }
                        if (frame != null) {
                            try { for (int offset=0; offset<frame.data.length && alive && frame.epoch==videoEpoch;) {
                                for (int i=0; i<4 && (p=outgoing.poll())!=null; i++) writePacket(p);
                                if(frame.epoch!=videoEpoch)break;
                                int count=Math.min(65536,frame.data.length-offset);
                                writeStarted=SystemClock.elapsedRealtime();
                                output.writeByte(3); output.writeInt(20+count); output.writeLong(frame.pts); output.writeInt(frame.data.length); output.writeInt(offset);output.writeInt(frame.dataSpace);
                                output.write(frame.data,offset,count); output.flush(); writeStarted=0; offset+=count;
                                writeRawChunk(chunk);
                            } } finally { writingPixels=false; }
                        }
                        writeRawChunk(chunk);
                        if (frame != null || raw != null) wake();
                    }
                } catch (Exception e) { if (alive) listener.status("Transport interrupted: " + e.getMessage()); }
                finally { close(); }
            }, "opencam-video").start();
            new Thread(() -> {
                try {
                    while (alive) {
                        Thread.sleep(250);
                        long now=SystemClock.elapsedRealtime();
                        if ((writeStarted != 0 && now-writeStarted > 750) || now-lastCommand > 3500) close();
                    }
                } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
            }, "opencam-watchdog").start();
        }
        void writeRawChunk(byte[] chunk) throws IOException {
            RawTransfer file;
            synchronized(this){file=raw;if(file==null || !outgoing.isEmpty())return;}
            int count=file.input.read(chunk);
            if(count<0){
                if(file.offset!=file.size)throw new IOException("RAW file changed during transfer");
                writePacket(new Packet(1,Json.object("type","raw_file_end","id",file.text,"sha256",hex(file.digest.digest())).toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)));
                file.close();synchronized(this){if(raw==file)raw=null;}
            }else if(count>0){
                writeStarted=SystemClock.elapsedRealtime();output.writeByte(4);output.writeInt(24+count);output.write(file.id);output.writeLong(file.offset);output.write(chunk,0,count);output.flush();writeStarted=0;
                file.digest.update(chunk,0,count);file.offset+=count;
            }
        }
        void wake() { if (ready.availablePermits()==0) ready.release(); }
        void writePacket(Packet packet) throws IOException {
            writeStarted=SystemClock.elapsedRealtime(); output.writeByte(packet.kind); output.writeInt(packet.data.length); output.write(packet.data); output.flush(); writeStarted=0;
        }
        synchronized void send(JSONObject value) {
            byte[] bytes = value.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
            if (bytes.length > MAX_PACKET || !outgoing.offer(new Packet(1, bytes))) close(); else wake();
        }
        synchronized void video(byte[] data, long pts, int flags, byte[] config,long epoch) {
            if(!alive || epoch!=videoEpoch)return;
            boolean key = (flags & android.media.MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0;
            if (encoded.remainingCapacity()==0) { encoded.clear(); waitingKeyframe=true; syncFrame.run(); }
            if (waitingKeyframe && !key) return;
            waitingKeyframe=false;
            byte[] header=key && config!=null ? config : new byte[0];
            ByteBuffer packet=ByteBuffer.allocate(12+header.length+data.length).order(ByteOrder.BIG_ENDIAN);
            packet.putLong(pts).putInt(flags).put(header).put(data);
            if (packet.capacity()>MAX_PACKET || !encoded.offer(new Packet(2,packet.array()))) { waitingKeyframe=true;syncFrame.run(); } else wake();
        }
        synchronized boolean pixelsReady() { return alive && pixels==null && !writingPixels; }
        synchronized void pixels(byte[] data,long pts,int dataSpace,long epoch) { if (epoch==videoEpoch && pixelsReady()) { pixels=new Pixels(data,pts,dataSpace,videoEpoch);wake(); } }
        void resetVideo() { synchronized (this) { videoEpoch++;encoded.clear();pixels=null;waitingKeyframe=true; } }
        synchronized void raw(android.net.Uri uri) throws Exception {
            if (raw!=null) throw new IOException("A RAW transfer is already active");
            long size; try (var file=context.getContentResolver().openAssetFileDescriptor(uri,"r")) {
                if (file==null) throw new IOException("RAW file is unavailable"); size=file.getLength();if(size<0)size=file.getParcelFileDescriptor().getStatSize();
            }
            if (size<=0 || size>512L*1024*1024) throw new IOException("RAW file exceeds transfer limit");
            InputStream input=context.getContentResolver().openInputStream(uri);
            if (input==null) throw new IOException("RAW file is unavailable");
            try { raw=new RawTransfer(input,size); send(Json.object("type","raw_file_begin","id",raw.text,"bytes",size,"format","dng")); }
            catch (Exception e) { input.close();raw=null;throw e; }
        }
        @Override public synchronized void close() {
            if (!alive) return;
            alive = false; ready.release(); pixels=null;encoded.clear();outgoing.clear();
            if (raw!=null) { raw.close();raw=null; }
            Runnable disconnect=()->{try{socket.close();}catch(IOException ignored){}};
            if(android.os.Looper.myLooper()==android.os.Looper.getMainLooper())new Thread(disconnect,"opencam-close").start();else disconnect.run();
            if (peer == this) { peer = null; listener.disconnected(); }
        }
    }

    void send(JSONObject value) { Peer p = peer; if (p != null) p.send(value); }
    void raw(android.net.Uri uri) throws Exception { Peer p=peer;if(p!=null)p.raw(uri); }
    void resetVideo() { Peer p = peer; if (p != null) p.resetVideo(); }
    @Override public void close() {
        running = false;
        if (registration != null) { try { nsd.unregisterService(registration); } catch (RuntimeException ignored) { } registration = null; }
        Peer p = peer;
        if (p != null) p.close();
        try { server.close(); } catch (IOException ignored) { }
    }
}
