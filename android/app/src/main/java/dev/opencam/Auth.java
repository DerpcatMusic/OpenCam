package dev.opencam;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.security.GeneralSecurityException;
import javax.crypto.Cipher;
import javax.crypto.Mac;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;

final class Auth {
    static final int ITERATIONS = 210_000;
    record Credential(byte[] salt, byte[] key) { }
    static byte[] random(int length) { byte[] bytes = new byte[length]; new SecureRandom().nextBytes(bytes); return bytes; }
    static byte[] derive(String password, byte[] salt) throws Exception {
        if (password.length() < 8 || password.length() > 128 || salt.length != 16)
            throw new IllegalArgumentException("Use a password of 8–128 characters");
        PBEKeySpec spec = new PBEKeySpec(password.toCharArray(), salt, ITERATIONS, 256);
        try { return SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256").generateSecret(spec).getEncoded(); }
        finally { spec.clearPassword(); }
    }
    static byte[] proof(byte[] key, byte[] challenge, byte[] pin) throws Exception {
        if (key.length != 32 || challenge.length != 32 || pin.length != 32) throw new IllegalArgumentException("Invalid authentication data");
        Mac mac = Mac.getInstance("HmacSHA256"); mac.init(new SecretKeySpec(key, "HmacSHA256"));
        mac.update("opencam-auth-v1\0".getBytes(StandardCharsets.UTF_8)); mac.update(challenge); return mac.doFinal(pin);
    }
    static byte[] unhex(String value, int length) {
        if (value.length() != length * 2 || !value.matches("[0-9a-fA-F]+")) throw new IllegalArgumentException("Invalid authentication value");
        byte[] bytes = new byte[length];
        for (int i=0;i<length;i++) bytes[i]=(byte)Integer.parseInt(value.substring(i*2,i*2+2),16);
        return bytes;
    }
    static boolean verify(byte[] key, byte[] challenge, byte[] pin, String supplied) throws Exception {
        return MessageDigest.isEqual(proof(key,challenge,pin), unhex(supplied,32));
    }

    static boolean tailscale(String host) {
        String[] part = host.split("\\.");
        if (part.length != 4 || !part[0].equals("100")) return false;
        try { int second = Integer.parseInt(part[1]); return second >= 64 && second <= 127; }
        catch (NumberFormatException e) { return false; }
    }

    static byte[] seal(byte[] secret, byte[] plain) throws GeneralSecurityException {
        MessageDigest sha = MessageDigest.getInstance("SHA-256");
        sha.update("opencam-pair-v1".getBytes(StandardCharsets.US_ASCII)); sha.update(secret);
        byte[] iv = Auth.random(12);
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(sha.digest(), "AES"), new GCMParameterSpec(128, iv));
        byte[] sealed = cipher.doFinal(plain), out = new byte[12 + sealed.length];
        System.arraycopy(iv, 0, out, 0, 12); System.arraycopy(sealed, 0, out, 12, sealed.length);
        return out;
    }
}
