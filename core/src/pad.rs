//! Virtual DualShock 4: the vocabulary the remote pad service speaks.
//!
//! A PS4 has no built-in network pad API. The community payload Control4Free
//! (a normal fake PKG, installable from this app) opens port 4264, takes a
//! WebRTC data channel and drives a virtual DualShock 4 from it. The heavy
//! lifting (DTLS, SCTP) happens in the app's own webview, so this module only
//! owns the piece the webview cannot do: the SDP exchange, which must carry no
//! `Origin` header, and the button bit mask.
//!
//! Protocol notes, from the payload's `docs/protocol.md`: input is the JSON
//! array sent on the `input` channel,
//! `[pad, seq, buttons, lx, ly, rx, ry, l2, r2, fingers, ...]`.

use crate::config::Host;
use crate::error::{Error, Result};
use serde::Serialize;
use std::time::Duration;

/// Button name and bit, in the order this app's pad UI draws them.
pub const BUTTONS: &[(&str, u32)] = &[
    ("share", 0x0001),
    ("l3", 0x0002),
    ("r3", 0x0004),
    ("options", 0x0008),
    ("up", 0x0010),
    ("right", 0x0020),
    ("down", 0x0040),
    ("left", 0x0080),
    ("l2", 0x0100),
    ("r2", 0x0200),
    ("l1", 0x0400),
    ("r1", 0x0800),
    ("triangle", 0x1000),
    ("circle", 0x2000),
    ("cross", 0x4000),
    ("square", 0x8000),
    ("ps", 0x10000),
    ("touchpad", 0x100000),
];

#[derive(Debug, Clone, Default, Serialize)]
pub struct PadStatus {
    pub ok: bool,
    pub version: Option<String>,
    pub protocol: Option<u64>,
    /// How many virtual controllers the service can create.
    pub pads: Option<u64>,
    pub message: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
}

/// Ask the payload for its status page.
pub fn status(host: &Host) -> PadStatus {
    let url = format!("{}/api/status", host.pad_base());
    match agent().get(&url).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            let value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            PadStatus {
                ok: true,
                version: value
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                protocol: value.get("protocol").and_then(|v| v.as_u64()),
                pads: value.get("pads").and_then(|v| v.as_u64()),
                message: body.chars().take(200).collect(),
            }
        }
        Err(ureq::Error::Status(code, _)) => PadStatus {
            ok: false,
            message: format!(
                "the pad service answered HTTP {code}; load Control4Free on the console"
            ),
            ..Default::default()
        },
        Err(e) => PadStatus {
            ok: false,
            message: format!("no pad service on port {}: {e}", host.pad_port),
            ..Default::default()
        },
    }
}

/// Post a WebRTC offer and return the payload's answer.
///
/// The payload refuses any `Origin` other than its own or `null`, and a native
/// HTTP call sends none, which is why this does not run in the webview.
pub fn exchange(host: &Host, offer: &str) -> Result<String> {
    if offer.trim().is_empty() {
        return Err(Error::Pad("empty WebRTC offer".into()));
    }
    let url = format!("{}/rtc", host.pad_base());
    match agent()
        .post(&url)
        .set("Content-Type", "text/plain")
        .send_string(offer)
    {
        Ok(resp) => {
            let answer = resp.into_string().unwrap_or_default();
            if answer.trim().is_empty() {
                Err(Error::Pad("the pad service returned an empty answer".into()))
            } else {
                Ok(answer)
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            let detail = resp.into_string().unwrap_or_default();
            Err(Error::Pad(format!(
                "offer refused (HTTP {code}): {}",
                detail.trim()
            )))
        }
        Err(e) => Err(Error::Pad(format!("offer failed: {e}"))),
    }
}

/// Bit mask for one button name, accepting `x` as an alias for cross.
pub fn bit(name: &str) -> Option<u32> {
    let name = name.trim().to_ascii_lowercase();
    let name = if name == "x" { "cross" } else { &name };
    BUTTONS
        .iter()
        .find(|(button, _)| *button == name)
        .map(|(_, mask)| *mask)
}

/// Bit mask for a `a+b+c` combo.
pub fn combo(names: &[&str]) -> Result<u32> {
    let mut mask = 0u32;
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        mask |= bit(name).ok_or_else(|| Error::Invalid(format!("unknown button '{name}'")))?;
    }
    if mask == 0 {
        return Err(Error::Invalid("no buttons given".into()));
    }
    Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_has_a_unique_bit() {
        let mut seen = std::collections::HashSet::new();
        for (name, mask) in BUTTONS {
            assert!(seen.insert(*mask), "{name} reuses a bit");
            assert_eq!(bit(name), Some(*mask));
        }
    }

    #[test]
    fn parses_aliases_and_combos() {
        assert_eq!(bit("X"), Some(0x4000));
        assert_eq!(combo(&["cross", "up"]).unwrap(), 0x4010);
        assert_eq!(combo(&["cross+up"]).unwrap_err().to_string(), "invalid data: unknown button 'cross+up'");
        assert!(combo(&["jump"]).is_err());
        assert!(combo(&[]).is_err());
    }

    #[test]
    fn refuses_an_empty_offer() {
        let host = Host::new("test", "127.0.0.1");
        assert!(exchange(&host, "  ").is_err());
    }
}
