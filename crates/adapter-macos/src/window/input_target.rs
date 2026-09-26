use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use super::snapshot::SnapshotImage;
use super::target::{WindowIdentity, now_seconds};
use crate::MacosError;
use crate::screenshot::CapturableWindow;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct InputTarget {
    kind: String,
    version: u8,
    pub issued_at: u64,
    pub window: WindowIdentity,
    pub image_width: u32,
    pub image_height: u32,
}

impl InputTarget {
    pub fn issue(window: &CapturableWindow, image: &SnapshotImage) -> Result<String, MacosError> {
        let target = Self {
            kind: "window_input".into(),
            version: 1,
            issued_at: now_seconds()?,
            window: WindowIdentity::from(window),
            image_width: image.width,
            image_height: image.height,
        };
        serde_json::to_vec(&target)
            .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            .map_err(|error| MacosError::Other(format!("failed to encode input target: {error}")))
    }

    pub fn decode(token: &str, now: u64) -> Result<Self, MacosError> {
        let invalid =
            || MacosError::Other("invalid window input target; take a new snapshot".into());
        if token.len() > 16_384 {
            return Err(invalid());
        }
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| invalid())?;
        let target: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if target.kind != "window_input"
            || target.version != 1
            || now
                .checked_sub(target.issued_at)
                .is_none_or(|age| age > 300)
            || target.window.owner_pid <= 0
            || target.window.window_id == 0
            || target.window.bounds.width <= 0
            || target.window.bounds.height <= 0
            || target.image_width == 0
            || target.image_height == 0
            || target.image_width > i32::MAX as u32
            || target.image_height > i32::MAX as u32
        {
            return Err(invalid());
        }
        Ok(target)
    }

    pub fn frame_point(&self, x: f64, y: f64) -> Result<(f64, f64), MacosError> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= f64::from(self.image_width)
            || y >= f64::from(self.image_height)
        {
            return Err(MacosError::Other(
                "input point must be inside the snapshot image".into(),
            ));
        }
        Ok((
            x * f64::from(self.window.bounds.width) / f64::from(self.image_width),
            y * f64::from(self.window.bounds.height) / f64::from(self.image_height),
        ))
    }
}
