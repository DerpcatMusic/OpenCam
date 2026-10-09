use gpui::{AssetSource, SharedString};
use std::borrow::Cow;
pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "icons/camera.svg" => Some(Cow::Borrowed(include_bytes!("icons/camera.svg"))),
            "icons/camera-off.svg" => Some(Cow::Borrowed(include_bytes!("icons/camera-off.svg"))),
            "icons/play.svg" => Some(Cow::Borrowed(include_bytes!("icons/play.svg"))),
            "icons/stop.svg" => Some(Cow::Borrowed(include_bytes!("icons/stop.svg"))),
            "icons/clipboard.svg" => Some(Cow::Borrowed(include_bytes!("icons/clipboard.svg"))),
            "icons/wifi.svg" => Some(Cow::Borrowed(include_bytes!("icons/wifi.svg"))),
            "icons/usb.svg" => Some(Cow::Borrowed(include_bytes!("icons/usb.svg"))),
            "icons/power.svg" => Some(Cow::Borrowed(include_bytes!("icons/power.svg"))),
            "icons/rotate.svg" => Some(Cow::Borrowed(include_bytes!("icons/rotate.svg"))),
            "icons/flip.svg" => Some(Cow::Borrowed(include_bytes!("icons/flip.svg"))),
            "icons/focus.svg" => Some(Cow::Borrowed(include_bytes!("icons/focus.svg"))),
            "icons/lock.svg" => Some(Cow::Borrowed(include_bytes!("icons/lock.svg"))),
            "icons/gauge.svg" => Some(Cow::Borrowed(include_bytes!("icons/gauge.svg"))),
            "icons/download.svg" => Some(Cow::Borrowed(include_bytes!("icons/download.svg"))),
            "icons/photo.svg" => Some(Cow::Borrowed(include_bytes!("icons/photo.svg"))),
            "icons/chevron.svg" => Some(Cow::Borrowed(include_bytes!("icons/chevron.svg"))),
            "icons/chevron-up.svg" => Some(Cow::Borrowed(include_bytes!("icons/chevron-up.svg"))),
            "icons/check.svg" => Some(Cow::Borrowed(include_bytes!("icons/check.svg"))),
            "icons/monitor.svg" => Some(Cow::Borrowed(include_bytes!("icons/monitor.svg"))),
            "icons/sun.svg" => Some(Cow::Borrowed(include_bytes!("icons/sun.svg"))),
            "icons/light.svg" => Some(Cow::Borrowed(include_bytes!("icons/light.svg"))),
            "icons/optical.svg" => Some(Cow::Borrowed(include_bytes!("icons/optical.svg"))),
            "icons/video.svg" => Some(Cow::Borrowed(include_bytes!("icons/video.svg"))),
            "icons/refresh.svg" => Some(Cow::Borrowed(include_bytes!("icons/refresh.svg"))),
            "icons/minus.svg" => Some(Cow::Borrowed(include_bytes!("icons/minus.svg"))),
            "icons/plus.svg" => Some(Cow::Borrowed(include_bytes!("icons/plus.svg"))),
            "icons/settings.svg" => Some(Cow::Borrowed(include_bytes!("icons/settings.svg"))),
            "icons/chart.svg" => Some(Cow::Borrowed(include_bytes!("icons/chart.svg"))),
            "icons/grid.svg" => Some(Cow::Borrowed(include_bytes!("icons/grid.svg"))),
            "icons/eye.svg" => Some(Cow::Borrowed(include_bytes!("icons/eye.svg"))),
            "icons/palette.svg" => Some(Cow::Borrowed(include_bytes!("icons/palette.svg"))),
            "icons/crop.svg" => Some(Cow::Borrowed(include_bytes!("icons/crop.svg"))),
            "icons/network.svg" => Some(Cow::Borrowed(include_bytes!("icons/network.svg"))),
            "icons/format.svg" => Some(Cow::Borrowed(include_bytes!("icons/format.svg"))),
            "icons/zebra.svg" => Some(Cow::Borrowed(include_bytes!("icons/zebra.svg"))),
            "icons/peaking.svg" => Some(Cow::Borrowed(include_bytes!("icons/peaking.svg"))),
            _ => None,
        })
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
