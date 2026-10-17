//! QR pairing: the desktop shows a QR code holding its addresses and a one-time secret. The phone scans it and
//! sends its own pairing link, AES-256-GCM sealed with a key derived from that secret, so a passive LAN
//! observer learns nothing and an outsider cannot forge a request. The user still has to press Accept.
use anyhow::{Context, Result, ensure};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub struct PairRequest {
    pub name: String,
    pub link: String,
}

pub struct Offer {
    pub uri: String,
    pub requests: mpsc::Receiver<PairRequest>,
    stop: Arc<AtomicBool>,
}

fn key(secret: &[u8; 16]) -> LessSafeKey {
    let mut sha = Sha256::new();
    sha.update(b"opencam-pair-v1");
    sha.update(secret);
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &sha.finalize()).unwrap())
}

#[cfg(test)]
fn seal(secret: &[u8; 16], nonce: [u8; 12], plain: &[u8]) -> Vec<u8> {
    let mut out = plain.to_vec();
    key(secret)
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut out)
        .unwrap();
    [&nonce[..], &out].concat()
}

fn open(secret: &[u8; 16], body: &[u8]) -> Result<PairRequest> {
    ensure!(body.len() > 12 + 16, "Pairing message too short");
    let mut data = body[12..].to_vec();
    let plain = key(secret)
        .open_in_place(
            Nonce::try_assume_unique_for_key(&body[..12]).unwrap(),
            Aad::empty(),
            &mut data,
        )
        .map_err(|_| anyhow::anyhow!("Pairing message was not sealed with this code"))?;
    let value: Value = serde_json::from_slice(plain)?;
    let link = value["link"].as_str().context("Pairing link missing")?;
    crate::protocol::Pairing::parse(link)?;
    Ok(PairRequest {
        name: value["name"]
            .as_str()
            .unwrap_or("Phone")
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect(),
        link: link.to_owned(),
    })
}

fn handle(mut stream: TcpStream, secret: &[u8; 16]) -> Result<PairRequest> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    ensure!(length <= 4096, "Pairing message too large");
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    let result = open(secret, &body);
    let _ = stream.write_all(&[result.is_ok() as u8]);
    result
}

/// Tailscale first (reachable over mobile data), then private LAN addresses.
fn addresses() -> Vec<Ipv4Addr> {
    let mut found: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(ip),
            _ => None,
        })
        .collect();
    found.sort_by_key(|ip| (!is_tailscale(*ip), !ip.is_private(), *ip));
    found.dedup();
    found.truncate(8);
    found
}

pub fn is_tailscale(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1])
}

impl Offer {
    pub fn start() -> Result<Self> {
        let listener =
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).context("No port for pairing")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let mut secret = [0; 16];
        SystemRandom::new()
            .fill(&mut secret)
            .map_err(|_| anyhow::anyhow!("No randomness available"))?;
        let hosts: Vec<_> = addresses().iter().map(Ipv4Addr::to_string).collect();
        ensure!(!hosts.is_empty(), "No network address to pair over");
        let uri = format!(
            "opencam-pair://?a={}&p={port}&s={}",
            hosts.join(","),
            secret
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let (sender, requests) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            // A forgotten code expires after ten minutes.
            let deadline = std::time::Instant::now() + Duration::from_secs(600);
            while !flag.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // The code is single use: stop after the first request that decrypts.
                        if let Ok(request) = handle(stream, &secret) {
                            let _ = sender.send(request);
                            break;
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        });
        Ok(Self {
            uri,
            requests,
            stop,
        })
    }

    pub fn image(&self) -> Result<image::RgbaImage> {
        use qrcode::{Color, QrCode};
        const SCALE: u32 = 6;
        const QUIET: u32 = 4;
        let code = QrCode::new(self.uri.as_bytes())?;
        let modules = code.width() as u32;
        let side = (modules + 2 * QUIET) * SCALE;
        let colors = code.to_colors();
        Ok(image::RgbaImage::from_fn(side, side, |x, y| {
            let (mx, my) = (
                (x / SCALE).wrapping_sub(QUIET),
                (y / SCALE).wrapping_sub(QUIET),
            );
            let dark =
                mx < modules && my < modules && colors[(my * modules + mx) as usize] == Color::Dark;
            image::Rgba(if dark {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            })
        }))
    }
}

impl Drop for Offer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link() -> String {
        format!(
            "opencam://100.1.2.3:4937?token={}&pin={}",
            "ab".repeat(16),
            "cd".repeat(32)
        )
    }

    #[test]
    fn only_the_scanned_code_can_pair_and_the_code_is_single_use() {
        let offer = Offer::start().unwrap();
        let port: u16 = offer
            .uri
            .split("&p=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let hex = offer.uri.split("&s=").nth(1).unwrap();
        let secret: [u8; 16] = crate::protocol::unhex(hex).unwrap();
        let send = |secret: &[u8; 16]| {
            let body = seal(
                secret,
                [7; 12],
                format!(r#"{{"name":"Pixel\n","link":"{}"}}"#, link()).as_bytes(),
            );
            let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
            socket.write_all(&(body.len() as u32).to_be_bytes())?;
            socket.write_all(&body)?;
            let mut reply = [0; 1];
            socket.read_exact(&mut reply).map(|_| reply[0])
        };
        let send = |secret: &[u8; 16]| -> std::io::Result<u8> { send(secret) };
        assert_eq!(send(&[9; 16]).unwrap(), 0);
        assert!(offer.requests.try_recv().is_err());
        assert_eq!(send(&secret).unwrap(), 1);
        let request = offer.requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!((request.name.as_str(), request.link), ("Pixel", link()));
        std::thread::sleep(Duration::from_millis(300));
        assert!(send(&secret).is_err());
        assert!(offer.image().unwrap().width() > 100);
    }

    #[test]
    fn tailscale_range_is_cgnat_100_64_slash_10() {
        assert!(
            is_tailscale(Ipv4Addr::new(100, 67, 1, 2))
                && is_tailscale(Ipv4Addr::new(100, 127, 0, 1))
        );
        assert!(
            !is_tailscale(Ipv4Addr::new(100, 128, 0, 1))
                && !is_tailscale(Ipv4Addr::new(10, 0, 0, 1))
        );
    }
}
