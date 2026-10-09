package dev.opencam;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import javax.crypto.Mac;
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
}
