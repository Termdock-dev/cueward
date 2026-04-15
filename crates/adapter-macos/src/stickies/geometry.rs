use crate::MacosError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickyFrame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl StickyFrame {
    pub fn to_state_value(self) -> String {
        format!(
            "{{{{{}, {}}}, {{{}, {}}}}}",
            self.x, self.y, self.width, self.height
        )
    }

    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    pub fn with_size(self, size: StickySize) -> Self {
        Self {
            width: size.width,
            height: size.height,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickySize {
    pub width: i32,
    pub height: i32,
}

impl StickySize {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_state_value(self) -> String {
        format!("{{{}, {}}}", self.width, self.height)
    }
}

pub fn parse_frame(frame: &str) -> Result<StickyFrame, MacosError> {
    let Some((position, size)) = frame
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .and_then(|value| value.split_once("}, {"))
    else {
        return Err(MacosError::Other(format!("invalid frame string: {frame}")));
    };
    let Some((x, y)) = position.split_once(", ") else {
        return Err(MacosError::Other(format!("invalid frame string: {frame}")));
    };
    let Some((width, height)) = size.split_once(", ") else {
        return Err(MacosError::Other(format!("invalid frame string: {frame}")));
    };

    Ok(StickyFrame {
        x: x
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid frame string: {frame}")))?,
        y: y
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid frame string: {frame}")))?,
        width: width
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid frame string: {frame}")))?,
        height: height
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid frame string: {frame}")))?,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_expanded_size(size: &str) -> Result<StickySize, MacosError> {
    let Some(inner) = size.strip_prefix('{').and_then(|value| value.strip_suffix('}')) else {
        return Err(MacosError::Other(format!("invalid expanded size string: {size}")));
    };
    let Some((width, height)) = inner.split_once(", ") else {
        return Err(MacosError::Other(format!("invalid expanded size string: {size}")));
    };

    Ok(StickySize {
        width: width
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid expanded size string: {size}")))?,
        height: height
            .parse()
            .map_err(|_| MacosError::Other(format!("invalid expanded size string: {size}")))?,
    })
}
