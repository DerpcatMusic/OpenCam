package dev.opencam;
public class AuthTest {
    public static void main(String[] args) throws Exception {
        byte[] salt=new byte[16], challenge=new byte[32], pin=new byte[32];
        java.util.Arrays.fill(salt,(byte)1); java.util.Arrays.fill(challenge,(byte)2); java.util.Arrays.fill(pin,(byte)3);
        byte[] key=Auth.derive("fixture-password",salt);
        assert Auth.verify(key,challenge,pin,"e4c3904d0cb98b16158615e91c5b66c9f0480d45fb9a26c37df5f0938a9337b2");
        pin[0]=4; assert !Auth.verify(key,challenge,pin,"e4c3904d0cb98b16158615e91c5b66c9f0480d45fb9a26c37df5f0938a9337b2");
        boolean rejected=false; try { Auth.derive("short",salt); } catch(IllegalArgumentException e) { rejected=true; }
        assert rejected;
        String unicodeKey="2c06feb776dd457bd03021a9dacb3cd9321d15a5bd86ff6c3a002734b029f6e8";
        assert java.security.MessageDigest.isEqual(Auth.derive("fixture-🔑",salt),Auth.unhex(unicodeKey,32));
        System.out.println("Password derivation and certificate binding passed");
    }
}
