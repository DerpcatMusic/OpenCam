package dev.opencam;
public class PairTest {
    public static void main(String[] args) throws Exception {
        assert Auth.tailscale("100.67.124.81") && Auth.tailscale("100.127.0.1");
        assert !Auth.tailscale("100.128.0.1") && !Auth.tailscale("192.168.1.2");
        byte[] secret=new byte[16];
        byte[] sealed=Auth.seal(secret,"{}".getBytes());
        assert sealed.length==12+2+16;
        // Fixed vector shared with desktop/qrpair.rs (nonce zeroed there; here we only check key derivation by decrypting).
        java.security.MessageDigest sha=java.security.MessageDigest.getInstance("SHA-256");
        sha.update("opencam-pair-v1".getBytes()); sha.update(secret);
        javax.crypto.Cipher c=javax.crypto.Cipher.getInstance("AES/GCM/NoPadding");
        c.init(javax.crypto.Cipher.DECRYPT_MODE,new javax.crypto.spec.SecretKeySpec(sha.digest(),"AES"),new javax.crypto.spec.GCMParameterSpec(128,sealed,0,12));
        assert new String(c.doFinal(sealed,12,sealed.length-12)).equals("{}");
        System.out.println("Pairing encryption passed");
    }
}
