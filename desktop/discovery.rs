use anyhow::{Context, Result};
use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent};
use std::{collections::BTreeMap, net::Ipv4Addr};
const SERVICE: &str = "_opencam._tcp.local.";
#[derive(Clone, PartialEq)]
pub struct Phone {
    pub name: String,
    pub link: String,
    pub protected: bool,
}
pub struct Discovery {
    daemon: ServiceDaemon,
    events: Receiver<ServiceEvent>,
    pub phones: BTreeMap<String, Phone>,
}
impl Discovery {
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new().context("LAN discovery unavailable; use pairing link")?;
        let events = daemon.browse(SERVICE)?;
        Ok(Self {
            daemon,
            events,
            phones: BTreeMap::new(),
        })
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for event in self.events.try_iter().take(64) {
            match event {
                ServiceEvent::ServiceResolved(service) => {
                    if service.get_property_val_str("protocol") != Some("1") {
                        continue;
                    }
                    let mut addresses: Vec<_> = service.get_addresses_v4().into_iter().collect();
                    addresses.sort_by_key(|ip| (!ip.is_private(), *ip));
                    let Some(ip) = addresses.first() else {
                        continue;
                    };
                    let Some(token) = service.get_property_val_str("token") else {
                        continue;
                    };
                    let Some(pin) = service.get_property_val_str("pin") else {
                        continue;
                    };
                    let Ok(link) = pairing_link(*ip, service.get_port(), token, pin) else {
                        continue;
                    };
                    let name = service
                        .get_fullname()
                        .strip_suffix(SERVICE)
                        .unwrap_or("Phone")
                        .trim_end_matches('.')
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(80)
                        .collect();
                    let phone = Phone {
                        name,
                        link,
                        protected: service.get_property_val_str("auth") == Some("password"),
                    };
                    if self.phones.get(service.get_fullname()) != Some(&phone) {
                        if self.phones.len() < 64
                            || self.phones.contains_key(service.get_fullname())
                        {
                            self.phones
                                .insert(service.get_fullname().to_string(), phone);
                            changed = true;
                        }
                    }
                }
                ServiceEvent::ServiceRemoved(_, name) => {
                    changed |= self.phones.remove(&name).is_some();
                }
                _ => {}
            }
        }
        changed
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}
fn pairing_link(ip: Ipv4Addr, port: u16, token: &str, pin: &str) -> Result<String> {
    let link = format!("opencam://{ip}:{port}?token={token}&pin={pin}");
    crate::protocol::Pairing::parse(&link)?;
    Ok(link)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_discovery_credentials_before_using_them() {
        assert!(
            pairing_link(
                Ipv4Addr::LOCALHOST,
                4937,
                &"ab".repeat(16),
                &"cd".repeat(32)
            )
            .is_ok()
        );
        assert!(pairing_link(Ipv4Addr::LOCALHOST, 4937, "ab&pin=evil", &"cd".repeat(32)).is_err());
        assert!(pairing_link(Ipv4Addr::LOCALHOST, 0, &"ab".repeat(16), &"cd".repeat(32)).is_err());
    }
}
