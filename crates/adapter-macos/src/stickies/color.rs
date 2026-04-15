use plist::{Dictionary, Value};
use std::str::FromStr;

use crate::MacosError;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct StickyColorScheme {
    pub control: Dictionary,
    pub highlight: Dictionary,
    pub spine: Dictionary,
    pub sticky: Dictionary,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickyColorPreset {
    Blue,
    Yellow,
    Green,
    Pink,
    Purple,
    Gray,
}

impl StickyColorPreset {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn scheme(self) -> StickyColorScheme {
        match self {
            Self::Blue => StickyColorScheme {
                control: color_dictionary(0.1411764705882353, 0.8156862745098039, 0.9137254901960784, 1.0),
                highlight: color_dictionary(0.00784313725490196, 0.7372549019607844, 0.8431372549019608, 1.0),
                spine: color_dictionary(0.5372549019607843, 0.9411764705882353, 1.0, 1.0),
                sticky: color_dictionary(0.6784313725490196, 0.9568627450980393, 1.0, 1.0),
            },
            Self::Yellow => StickyColorScheme {
                control: color_dictionary(0.8588235294117647, 0.7725490196078432, 0.011764705882352941, 1.0),
                highlight: color_dictionary(0.7372549019607844, 0.6627450980392157, 0.00784313725490196, 1.0),
                spine: color_dictionary(0.996078431372549, 0.9176470588235294, 0.23921568627450981, 1.0),
                sticky: color_dictionary(0.996078431372549, 0.9568627450980393, 0.611764705882353, 1.0),
            },
            Self::Green => StickyColorScheme {
                control: color_dictionary(0.3176470588235294, 0.7333333333333333, 0.3176470588235294, 1.0),
                highlight: color_dictionary(0.2823529411764706, 0.6352941176470588, 0.2823529411764706, 1.0),
                spine: color_dictionary(0.5137254901960784, 0.996078431372549, 0.5137254901960784, 1.0),
                sticky: color_dictionary(0.6980392156862745, 1.0, 0.6313725490196078, 1.0),
            },
            Self::Pink => StickyColorScheme {
                control: color_dictionary(0.9725490196078431, 0.4980392156862745, 0.4980392156862745, 1.0),
                highlight: color_dictionary(0.8862745098039215, 0.4588235294117647, 0.4588235294117647, 1.0),
                spine: color_dictionary(1.0, 0.6980392156862745, 0.6980392156862745, 1.0),
                sticky: color_dictionary(1.0, 0.7803921568627451, 0.7803921568627451, 1.0),
            },
            Self::Purple => StickyColorScheme {
                control: color_dictionary(0.49019607843137253, 0.6078431372549019, 0.9215686274509803, 1.0),
                highlight: color_dictionary(0.4588235294117647, 0.5686274509803921, 0.8627450980392157, 1.0),
                spine: color_dictionary(0.6078431372549019, 0.7137254901960784, 0.996078431372549, 1.0),
                sticky: color_dictionary(0.7137254901960784, 0.792156862745098, 1.0, 1.0),
            },
            Self::Gray => StickyColorScheme {
                control: color_dictionary(0.6588235294117647, 0.6588235294117647, 0.6588235294117647, 1.0),
                highlight: color_dictionary(0.6196078431372549, 0.6196078431372549, 0.6196078431372549, 1.0),
                spine: color_dictionary(0.8549019607843137, 0.8549019607843137, 0.8549019607843137, 1.0),
                sticky: color_dictionary(0.9333333333333333, 0.9333333333333333, 0.9333333333333333, 1.0),
            },
        }
    }
}

impl FromStr for StickyColorPreset {
    type Err = MacosError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "blue" => Ok(Self::Blue),
            "yellow" => Ok(Self::Yellow),
            "green" => Ok(Self::Green),
            "pink" => Ok(Self::Pink),
            "purple" => Ok(Self::Purple),
            "gray" | "grey" => Ok(Self::Gray),
            _ => Err(MacosError::Other(format!(
                "unsupported sticky color: {value}"
            ))),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn color_dictionary(red: f64, green: f64, blue: f64, alpha: f64) -> Dictionary {
    let mut raw = Dictionary::new();
    raw.insert("Red".into(), Value::Real(red));
    raw.insert("Green".into(), Value::Real(green));
    raw.insert("Blue".into(), Value::Real(blue));
    raw.insert("Alpha".into(), Value::Real(alpha));
    raw
}
