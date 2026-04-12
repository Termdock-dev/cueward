use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use aho_corasick::AhoCorasick;
use serde::Deserialize;

use crate::Cue;

#[derive(Debug, Deserialize)]
struct TagsConfig {
    #[serde(flatten)]
    tags: HashMap<String, TagDef>,
}

#[derive(Debug, Deserialize)]
struct TagDef {
    keywords: Vec<String>,
}

pub struct Tagger {
    tag_names: Vec<String>,
    automaton: AhoCorasick,
    /// Maps pattern index → tag index
    pattern_to_tag: Vec<usize>,
}

impl Tagger {
    fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".cueward/tags.toml")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        let content = fs::read_to_string(&path).ok()?;
        let config: TagsConfig = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: failed to parse {}: {e}", path.display());
                return None;
            }
        };

        let mut tag_names = Vec::new();
        let mut patterns = Vec::new();
        let mut pattern_to_tag = Vec::new();

        for (tag_name, def) in &config.tags {
            let tag_idx = tag_names.len();
            tag_names.push(tag_name.clone());
            for keyword in &def.keywords {
                patterns.push(keyword.to_lowercase());
                pattern_to_tag.push(tag_idx);
            }
        }

        if patterns.is_empty() {
            return None;
        }

        let automaton = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(&patterns)
            .ok()?;

        Some(Self {
            tag_names,
            automaton,
            pattern_to_tag,
        })
    }

    pub fn tag(&self, cue: &mut Cue) {
        let text = format!("{} {}", cue.title.as_deref().unwrap_or(""), &cue.content);

        let mut seen = std::collections::HashSet::new();
        for mat in self.automaton.find_iter(&text) {
            let tag_idx = self.pattern_to_tag[mat.pattern().as_usize()];
            if seen.insert(tag_idx) {
                let tag = &self.tag_names[tag_idx];
                if !cue.tags.contains(tag) {
                    cue.tags.push(tag.clone());
                }
            }
        }
    }

    pub fn tag_all(&self, cues: &mut [Cue]) {
        for cue in cues {
            self.tag(cue);
        }
    }
}
