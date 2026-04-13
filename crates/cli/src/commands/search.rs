use std::process;

use cueward_core::CueIndex;

pub(crate) fn dispatch(query: String, limit: usize) {
    let idx = match CueIndex::open_or_create() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: failed to open index: {e}");
            process::exit(1);
        }
    };

    match idx.search(&query, limit) {
        Ok(results) => {
            if results.is_empty() {
                eprintln!("no results found");
            } else {
                for r in &results {
                    println!("{r}");
                }
                eprintln!("{} results", results.len());
            }
        }
        Err(e) => {
            eprintln!("error: search failed: {e}");
            process::exit(1);
        }
    }
}
