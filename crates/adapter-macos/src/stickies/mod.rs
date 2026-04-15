use plist::Value;
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::MacosError;

mod color;
mod display;
mod geometry;
mod state;

#[cfg(test)]
use self::color::color_dictionary;
pub use self::color::StickyColorPreset;
use self::display::{
    cascade_frame_for_display, find_display_bounds, load_display_bounds, resolve_frame_for_display,
    StickyDisplayBounds,
};
use self::geometry::{parse_expanded_size, parse_frame, StickyFrame, StickySize};
use self::state::{
    derive_sticky_title, ensure_update_fields, find_entry_index, load_state_entries_for_mutation, max_z_order,
    parse_saved_state, read_sticky_body, saved_state_path, sticky_dir, sticky_rtf_path, stickies_root_dir,
    write_saved_state, write_sticky_body, StickyStateEntry, TITLE_KEY,
};
#[cfg(test)]
use self::state::parse_saved_state_value;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StickiesNote {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickyMutationOptions {
    pub color: Option<StickyColorPreset>,
    pub display: Option<u32>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl StickyMutationOptions {
    fn has_geometry_options(&self) -> bool {
        self.display.is_some()
            || self.x.is_some()
            || self.y.is_some()
            || self.width.is_some()
            || self.height.is_some()
    }

    fn explicit_size(&self) -> Option<StickySize> {
        match (self.width, self.height) {
            (Some(width), Some(height)) => Some(StickySize { width, height }),
            _ => None,
        }
    }

    fn explicit_position(&self) -> Option<(i32, i32)> {
        match (self.x, self.y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }
}

fn ensure_update_request(
    title: Option<&str>,
    body: Option<&str>,
    options: &StickyMutationOptions,
) -> Result<(), MacosError> {
    if title.is_none()
        && body.is_none()
        && options.color.is_none()
        && options.display.is_none()
        && options.x.is_none()
        && options.y.is_none()
        && options.width.is_none()
        && options.height.is_none()
    {
        return ensure_update_fields(title, body);
    }
    Ok(())
}

fn list_stickies_from_root(root: &Path) -> Result<Vec<StickiesNote>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Err(MacosError::Other("stickies state not found".into()));
    }

    let entries = parse_saved_state(&state_path)?;
    let mut notes = Vec::new();
    for entry in entries {
        let body_path = sticky_rtf_path(root, &entry.id);
        if !body_path.exists() {
            continue;
        }
        let body = read_sticky_body(&body_path)?;
        let stored_title = entry.raw.get(TITLE_KEY).and_then(Value::as_string);
        let title = derive_sticky_title(stored_title, &body, &entry.id);
        notes.push(StickiesNote {
            id: entry.id,
            title,
            body,
        });
    }

    Ok(notes)
}

fn default_state_entry(id: &str, title: &str) -> StickyStateEntry {
    let mut raw = plist::Dictionary::new();
    raw.insert("UUID".into(), Value::String(id.into()));
    raw.insert("ExpandedSize".into(), Value::String("{300, 200}".into()));
    raw.insert("ExpandFrameY".into(), Value::Integer(0.into()));
    raw.insert("Floating".into(), Value::Integer(0.into()));
    raw.insert("Frame".into(), Value::String("{{200, 900}, {300, 200}}".into()));
    raw.insert("SpellCheckingTypes".into(), Value::Integer(9191.into()));
    raw.insert("Translucent".into(), Value::Integer(0.into()));
    raw.insert("ZOrder".into(), Value::Integer(1.into()));
    raw.insert(TITLE_KEY.into(), Value::String(title.into()));
    StickyStateEntry { id: id.into(), raw }
}

fn clone_state_entry(
    template: Option<&StickyStateEntry>,
    id: &str,
    title: &str,
    z_order: i64,
    options: &StickyMutationOptions,
    displays: &[StickyDisplayBounds],
    entries: &[StickyStateEntry],
) -> Result<StickyStateEntry, MacosError> {
    match template {
        Some(template) => {
            let mut raw = template.raw.clone();
            raw.insert("UUID".into(), Value::String(id.into()));
            raw.insert(TITLE_KEY.into(), Value::String(title.into()));
            raw.insert("ZOrder".into(), Value::Integer(z_order.into()));
            let frame = resolve_create_frame(entries, Some(template), options, displays)?;
            let size = StickySize {
                width: frame.width,
                height: frame.height,
            };
            if let Some(color) = options.color {
                apply_color_scheme(&mut raw, color.scheme());
            }
            raw.insert("Frame".into(), Value::String(frame.to_state_value()));
            raw.insert("ExpandedSize".into(), Value::String(size.to_state_value()));
            Ok(StickyStateEntry { id: id.into(), raw })
        }
        None => {
            let mut entry = default_state_entry(id, title);
            let frame = resolve_create_frame(entries, None, options, displays)?;
            let size = StickySize {
                width: frame.width,
                height: frame.height,
            };
            if let Some(color) = options.color {
                apply_color_scheme(&mut entry.raw, color.scheme());
            }
            entry.raw.insert("Frame".into(), Value::String(frame.to_state_value()));
            entry.raw
                .insert("ExpandedSize".into(), Value::String(size.to_state_value()));
            Ok(entry)
        }
    }
}

fn size_from_entry(entry: &StickyStateEntry) -> Option<StickySize> {
    entry.raw
        .get("ExpandedSize")
        .and_then(Value::as_string)
        .and_then(|value| parse_expanded_size(value).ok())
        .or_else(|| {
            entry.raw
                .get("Frame")
                .and_then(Value::as_string)
                .and_then(|value| parse_frame(value).ok())
                .map(|frame| StickySize {
                    width: frame.width,
                    height: frame.height,
                })
        })
}

fn frame_from_entry(entry: &StickyStateEntry) -> Option<StickyFrame> {
    entry.raw
        .get("Frame")
        .and_then(Value::as_string)
        .and_then(|value| parse_frame(value).ok())
}

fn resolve_create_frame(
    entries: &[StickyStateEntry],
    template: Option<&StickyStateEntry>,
    options: &StickyMutationOptions,
    displays: &[StickyDisplayBounds],
) -> Result<StickyFrame, MacosError> {
    let size = options
        .explicit_size()
        .or_else(|| template.and_then(size_from_entry))
        .unwrap_or(StickySize {
            width: 300,
            height: 200,
        });

    if let Some(display_index) = options.display {
        let display = find_display_bounds(displays, display_index)?;
        if let Some((x, y)) = options.explicit_position() {
            return Ok(resolve_frame_for_display(display, size, x, y));
        }

        let existing_frames = entries
            .iter()
            .filter_map(frame_from_entry)
            .filter(|frame| display.contains(*frame))
            .collect::<Vec<_>>();
        return Ok(cascade_frame_for_display(display, size, &existing_frames));
    }

    if let Some((x, y)) = options.explicit_position() {
        return Ok(StickyFrame {
            x,
            y,
            width: size.width,
            height: size.height,
        });
    }

    Ok(template
        .and_then(frame_from_entry)
        .map(|frame| frame.offset(24, -24).with_size(size))
        .unwrap_or(StickyFrame {
            x: 200,
            y: 900,
            width: size.width,
            height: size.height,
        }))
}

fn resolve_updated_frame(
    current: StickyFrame,
    options: &StickyMutationOptions,
    displays: &[StickyDisplayBounds],
) -> Result<StickyFrame, MacosError> {
    let size = options.explicit_size().unwrap_or(StickySize {
        width: current.width,
        height: current.height,
    });

    if let Some(display_index) = options.display {
        let display = find_display_bounds(displays, display_index)?;
        if let Some((x, y)) = options.explicit_position() {
            return Ok(resolve_frame_for_display(display, size, x, y));
        }
        return Ok(resolve_frame_for_display(display, size, 40, 40));
    }

    if let Some((x, y)) = options.explicit_position() {
        return Ok(StickyFrame {
            x,
            y,
            width: size.width,
            height: size.height,
        });
    }

    Ok(current.with_size(size))
}

fn apply_color_scheme(raw: &mut plist::Dictionary, scheme: self::color::StickyColorScheme) {
    raw.insert("ControlColor".into(), Value::Dictionary(scheme.control));
    raw.insert("HighlightColor".into(), Value::Dictionary(scheme.highlight));
    raw.insert("SpineColor".into(), Value::Dictionary(scheme.spine));
    raw.insert("StickyColor".into(), Value::Dictionary(scheme.sticky));
}

fn create_sticky_in_root(root: &Path, title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    create_sticky_in_root_with_options(root, title, body, &StickyMutationOptions::default(), &[])
}

fn create_sticky_in_root_with_options(
    root: &Path,
    title: &str,
    body: &str,
    options: &StickyMutationOptions,
    displays: &[StickyDisplayBounds],
) -> Result<StickiesNote, MacosError> {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let mut entries = load_state_entries_for_mutation(root)?;
    let next_z_order = max_z_order(&entries) + 1;
    let template = entries.first().cloned();
    entries.push(clone_state_entry(
        template.as_ref(),
        &id,
        title,
        next_z_order,
        options,
        displays,
        &entries,
    )?);
    let body_path = sticky_rtf_path(root, &id);
    write_sticky_body(&body_path, body)?;
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        let _ = fs::remove_dir_all(sticky_dir(root, &id));
        return Err(err);
    }

    Ok(StickiesNote {
        id,
        title: title.to_string(),
        body: body.to_string(),
    })
}

fn update_sticky_in_root(
    root: &Path,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<StickiesNote, MacosError> {
    update_sticky_in_root_with_options(
        root,
        id,
        title,
        body,
        &StickyMutationOptions::default(),
        &[],
    )
}

fn update_sticky_in_root_with_options(
    root: &Path,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
    options: &StickyMutationOptions,
    displays: &[StickyDisplayBounds],
) -> Result<StickiesNote, MacosError> {
    ensure_update_request(title, body, options)?;

    let mut entries = load_state_entries_for_mutation(root)?;
    let index = find_entry_index(&entries, id)?;
    let body_path = sticky_rtf_path(root, id);
    if !body_path.exists() {
        return Err(MacosError::Other(format!("sticky body not found: {id}")));
    }

    let current_body = read_sticky_body(&body_path)?;
    let original_raw = entries[index].raw.clone();
    let next_body = body.unwrap_or(&current_body).to_string();

    if let Some(title) = title {
        entries[index]
            .raw
            .insert(TITLE_KEY.into(), Value::String(title.to_string()));
    }
    if let Some(color) = options.color {
        apply_color_scheme(&mut entries[index].raw, color.scheme());
    }
    if options.has_geometry_options() {
        let current_frame = frame_from_entry(&entries[index]).unwrap_or(StickyFrame {
            x: 200,
            y: 900,
            width: 300,
            height: 200,
        });
        let next_frame = resolve_updated_frame(current_frame, options, displays)?;
        let next_size = StickySize {
            width: next_frame.width,
            height: next_frame.height,
        };
        entries[index]
            .raw
            .insert("Frame".into(), Value::String(next_frame.to_state_value()));
        entries[index]
            .raw
            .insert("ExpandedSize".into(), Value::String(next_size.to_state_value()));
    }

    if body.is_some() {
        write_sticky_body(&body_path, &next_body)?;
    }
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        entries[index].raw = original_raw;
        if body.is_some() {
            let _ = write_sticky_body(&body_path, &current_body);
        }
        return Err(err);
    }

    let next_title = entries[index]
        .raw
        .get(TITLE_KEY)
        .and_then(Value::as_string)
        .map(str::to_string)
        .unwrap_or_else(|| derive_sticky_title(None, &next_body, id));

    Ok(StickiesNote {
        id: id.to_string(),
        title: next_title,
        body: next_body,
    })
}

fn delete_sticky_in_root(root: &Path, id: &str) -> Result<(), MacosError> {
    let mut entries = load_state_entries_for_mutation(root)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == original_len {
        return Err(MacosError::Other(format!("sticky not found: {id}")));
    }

    write_saved_state(&saved_state_path(root), &entries)?;
    let dir = sticky_dir(root, id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|err| MacosError::Other(format!("failed to delete sticky dir: {err}")))?;
    }
    Ok(())
}

/// List all Stickies notes.
pub fn list_stickies() -> Result<Vec<StickiesNote>, MacosError> {
    list_stickies_from_root(&stickies_root_dir()?)
}

/// Create a Stickies note.
pub fn create_sticky(title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    create_sticky_in_root(&stickies_root_dir()?, title, body)
}

/// Create a Stickies note with geometry options.
pub fn create_sticky_with_options(
    title: &str,
    body: &str,
    options: &StickyMutationOptions,
) -> Result<StickiesNote, MacosError> {
    let displays = if options.display.is_some() {
        load_display_bounds()?
    } else {
        Vec::new()
    };
    create_sticky_in_root_with_options(&stickies_root_dir()?, title, body, options, &displays)
}

/// Update a Stickies note by UUID.
pub fn update_sticky(id: &str, title: Option<&str>, body: Option<&str>) -> Result<StickiesNote, MacosError> {
    update_sticky_in_root(&stickies_root_dir()?, id, title, body)
}

/// Update a Stickies note with geometry options.
pub fn update_sticky_with_options(
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
    options: &StickyMutationOptions,
) -> Result<StickiesNote, MacosError> {
    let displays = if options.display.is_some() {
        load_display_bounds()?
    } else {
        Vec::new()
    };
    update_sticky_in_root_with_options(&stickies_root_dir()?, id, title, body, options, &displays)
}

/// Delete a Stickies note by UUID.
pub fn delete_sticky(id: &str) -> Result<(), MacosError> {
    delete_sticky_in_root(&stickies_root_dir()?, id)
}

#[cfg(test)]
mod tests;
