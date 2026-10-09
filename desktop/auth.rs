use anyhow::{Result, ensure};
use ring::{hmac, pbkdf2};
use std::num::NonZeroU32;
pub const ITERATIONS: u32 = 210_000;
pub fn proof(
    password: &str,
    salt: &[u8; 16],
    challenge: &[u8; 32],
    pin: &[u8; 32],
) -> Result<String> {
    ensure!(
        (8..=128).contains(&password.chars().count()),
        "Enter the phone password (8–128 characters)"
    );
    let mut key = [0; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).unwrap(),
        salt,
        password.as_bytes(),
        &mut key,
    );
    let mut context = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, &key));
    key.fill(0);
    context.update(b"opencam-auth-v1\0");
    context.update(challenge);
    context.update(pin);
    Ok(context
        .sign()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proof_is_bound_to_challenge_certificate_and_password() {
        let expected = proof("fixture-password", &[1; 16], &[2; 32], &[3; 32]).unwrap();
        assert_ne!(
            expected,
            proof("wrong-password", &[1; 16], &[2; 32], &[3; 32]).unwrap()
        );
        assert_ne!(
            expected,
            proof("fixture-password", &[1; 16], &[4; 32], &[3; 32]).unwrap()
        );
        assert_ne!(
            expected,
            proof("fixture-password", &[1; 16], &[2; 32], &[4; 32]).unwrap()
        );
        assert!(proof("short", &[1; 16], &[2; 32], &[3; 32]).is_err());
        assert_eq!(
            expected,
            "e4c3904d0cb98b16158615e91c5b66c9f0480d45fb9a26c37df5f0938a9337b2"
        );
    }
}
