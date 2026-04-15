use std::process::Command;

use crate::MacosError;

use super::geometry::{StickyFrame, StickySize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickyDisplayBounds {
    pub frame: StickyFrame,
    pub visible_frame: StickyFrame,
}

impl StickyDisplayBounds {
    pub fn contains(self, frame: StickyFrame) -> bool {
        frame.x >= self.frame.x
            && frame.x < self.frame.x + self.frame.width
            && frame.y >= self.frame.y
            && frame.y < self.frame.y + self.frame.height
    }
}

pub fn resolve_frame_for_display(
    display: StickyDisplayBounds,
    size: StickySize,
    x: i32,
    y: i32,
) -> StickyFrame {
    StickyFrame {
        x: display.visible_frame.x + x,
        y: display.visible_frame.y + display.visible_frame.height - size.height - y,
        width: size.width,
        height: size.height,
    }
}

pub fn cascade_frame_for_display(
    display: StickyDisplayBounds,
    size: StickySize,
    existing: &[StickyFrame],
) -> StickyFrame {
    existing
        .iter()
        .copied()
        .max_by_key(|frame| (frame.x, frame.y))
        .map(|frame| frame.offset(24, -24).with_size(size))
        .unwrap_or_else(|| resolve_frame_for_display(display, size, 40, 40))
}

pub fn load_display_bounds() -> Result<Vec<StickyDisplayBounds>, MacosError> {
    let script = r#"import AppKit
for screen in NSScreen.screens {
    let f = screen.frame
    let v = screen.visibleFrame
    print("\(Int(f.origin.x))\t\(Int(f.origin.y))\t\(Int(f.size.width))\t\(Int(f.size.height))\t\(Int(v.origin.x))\t\(Int(v.origin.y))\t\(Int(v.size.width))\t\(Int(v.size.height))")
}"#;
    let output = Command::new("swift")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| MacosError::Other(format!("swift: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "failed to load display bounds: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            if parts.len() != 8 {
                return Err(MacosError::Other(format!(
                    "invalid display bounds output: {line}"
                )));
            }
            let ints: Result<Vec<i32>, _> = parts.iter().map(|part| part.parse::<i32>()).collect();
            let ints = ints.map_err(|_| {
                MacosError::Other(format!("invalid display bounds output: {line}"))
            })?;

            Ok(StickyDisplayBounds {
                frame: StickyFrame {
                    x: ints[0],
                    y: ints[1],
                    width: ints[2],
                    height: ints[3],
                },
                visible_frame: StickyFrame {
                    x: ints[4],
                    y: ints[5],
                    width: ints[6],
                    height: ints[7],
                },
            })
        })
        .collect()
}

pub fn find_display_bounds(
    displays: &[StickyDisplayBounds],
    index: u32,
) -> Result<StickyDisplayBounds, MacosError> {
    let zero_based = index
        .checked_sub(1)
        .ok_or_else(|| MacosError::Other(format!("invalid display number: {index}")))?;
    displays
        .get(zero_based as usize)
        .copied()
        .ok_or_else(|| MacosError::Other(format!("display not found: {index}")))
}
