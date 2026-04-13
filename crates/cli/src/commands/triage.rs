use std::process;

use cueward_core::{CueIndex, Tagger, inbox};

pub(crate) fn dispatch() {
    let batches = match inbox::load_all() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: failed to read inbox: {e}");
            process::exit(1);
        }
    };

    if batches.is_empty() {
        eprintln!("inbox is empty. run `cueward capture` first.");
        return;
    }

    let tagger = Tagger::load();
    let idx = match CueIndex::open_or_create() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: failed to open index: {e}");
            process::exit(1);
        }
    };

    let mut total = 0;
    for (path, mut cues) in batches {
        if let Some(t) = &tagger {
            t.tag_all(&mut cues);
        }

        match idx.add_cues(&cues) {
            Ok(n) => total += n,
            Err(e) => {
                eprintln!("error: failed to index: {e}");
                process::exit(1);
            }
        }

        if let Err(e) = inbox::mark_done(&path) {
            eprintln!("error: failed to move {}: {e}", path.display());
            eprintln!("aborting to prevent duplicate indexing on next triage run");
            process::exit(1);
        }
    }

    if tagger.is_some() {
        eprintln!("auto-tagged with ~/.cueward/tags.toml");
    } else {
        eprintln!("no tags.toml found, skipping auto-tag");
    }
    eprintln!("indexed {total} cues");
}
