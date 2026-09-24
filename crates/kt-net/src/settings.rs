//! Network settings and their platform defaults (DESIGN §2 settings table).
//!
//! Two separate helping settings:
//! - **Help others sync** (§7a): store other groups' slots in RAM. All platforms; phones
//!   only on Wi-Fi (and charging, by default).
//! - **Relay connections for others** (D51): carry encrypted live traffic for devices
//!   that can't connect directly. **Desktop only, on by default.** Only reachable desktops
//!   (public IPv4 or a UPnP router) actually serve; the rest silently don't.

use crate::helper::HelperLimits;
use crate::helper_relay::HelperRelayConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Desktop,
    Phone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpOthers {
    /// Phones: on Wi-Fi while charging, plus on Wi-Fi while the app is open (default).
    WifiAndCharging,
    /// Phones: on Wi-Fi only while the app is open.
    WifiWhileOpen,
    /// Desktops: whenever the app runs (default).
    On,
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetSettings {
    platform: Platform,
    pub help_others: HelpOthers,
    relay_for_others: bool,
}

impl NetSettings {
    pub fn defaults(platform: Platform) -> NetSettings {
        match platform {
            Platform::Desktop => NetSettings { platform, help_others: HelpOthers::On, relay_for_others: true },
            Platform::Phone => NetSettings { platform, help_others: HelpOthers::WifiAndCharging, relay_for_others: false },
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Whether the "Relay connections for others" setting exists on this platform.
    pub fn relay_setting_available(&self) -> bool {
        self.platform == Platform::Desktop
    }

    pub fn relay_for_others(&self) -> bool {
        self.relay_for_others
    }

    /// Change the relay setting. Refused on phones (the setting doesn't exist there).
    pub fn set_relay_for_others(&mut self, on: bool) -> Result<(), &'static str> {
        if !self.relay_setting_available() {
            return Err("relaying for others is desktop only");
        }
        self.relay_for_others = on;
        Ok(())
    }

    /// Storage-helping limits to run with right now, if helping. `on_wifi` / `charging` /
    /// `app_open` describe the phone's state (ignored on desktops).
    pub fn helper_limits(&self, on_wifi: bool, charging: bool, app_open: bool) -> Option<HelperLimits> {
        let active = match (self.platform, self.help_others) {
            (_, HelpOthers::Off) => false,
            (Platform::Desktop, _) => true,
            (Platform::Phone, HelpOthers::WifiAndCharging) => on_wifi && (charging || app_open),
            (Platform::Phone, HelpOthers::WifiWhileOpen) => on_wifi && app_open,
            (Platform::Phone, HelpOthers::On) => on_wifi,
        };
        active.then(|| match self.platform {
            Platform::Desktop => HelperLimits::default(),
            Platform::Phone => HelperLimits { capacity: 24 * 1024 * 1024, ..HelperLimits::default() },
        })
    }

    /// The live relay to run for strangers, if any (open access: D51). The caller still
    /// checks reachability (public IPv4 or UPnP) and provides the certificate.
    pub fn relay_config(&self) -> Option<HelperRelayConfig> {
        (self.relay_setting_available() && self.relay_for_others).then(HelperRelayConfig::default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_for_others_is_on_for_desktop_and_absent_on_phones() {
        let mut d = NetSettings::defaults(Platform::Desktop);
        assert!(d.relay_setting_available() && d.relay_for_others() && d.relay_config().is_some());
        d.set_relay_for_others(false).unwrap();
        assert!(d.relay_config().is_none());
        assert!(d.helper_limits(false, false, false).is_some(), "storage helping is a separate setting");

        let mut p = NetSettings::defaults(Platform::Phone);
        assert!(!p.relay_setting_available() && !p.relay_for_others() && p.relay_config().is_none());
        assert!(p.set_relay_for_others(true).is_err());
        assert!(p.helper_limits(true, true, false).is_some());
        assert!(p.helper_limits(false, true, true).is_none(), "phones help only on Wi-Fi");
    }
}
