use anyhow::{Result, ensure};

pub fn clipboard_pairing(text: &str) -> Result<String> {
    let text = text.trim();
    ensure!(
        text.len() <= 1024 && text.is_ascii() && !text.chars().any(char::is_control),
        "Copy the complete pairing link from OpenCam on your phone"
    );
    crate::protocol::Pairing::parse(text)?;
    Ok(text.to_owned())
}

pub fn recovery_hint(error: &str) -> &'static str {
    let error = error.to_ascii_lowercase();
    if error.contains("password") || error.contains("authentication") {
        "Enter the saved phone password, then connect again. It works for both Wi-Fi and USB."
    } else if error.contains("certificate") || error.contains("pairing") || error.contains("token")
    {
        "Select the phone again or copy a fresh pairing link from its Connection settings."
    } else if error.contains("adb") || error.contains("usb") {
        "Enable USB debugging, unlock the phone and approve this computer. Check that adb is on PATH."
    } else if error.contains("discovery") || error.contains("multicast") {
        "Copy the pairing link from the phone and paste it here. USB also works when LAN discovery is blocked."
    } else {
        "Keep OpenCam open with Available to desktops on. Check the same local network, or try USB."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_preserves_a_complete_link_and_rejects_modified_credentials() {
        let link = format!(
            "opencam://192.168.1.10:4937?token={}&pin={}",
            "ab".repeat(16),
            "cd".repeat(32)
        );
        assert_eq!(clipboard_pairing(&format!("\n {link} \r\n")).unwrap(), link);
        assert!(clipboard_pairing(&link.replace("?token=", "?token=\n")).is_err());
        assert!(clipboard_pairing(&format!("{link}&token={}", "ef".repeat(16))).is_err());
        assert!(clipboard_pairing("OpenCam pairing link").is_err());
    }
}
