use std::process;

use super::helpers::print_external;
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
            let result = results
                .iter()
                .map(|row| serde_json::from_str::<serde_json::Value>(row))
                .collect::<Result<Vec<_>, _>>();
            match result.and_then(|rows| serde_json::to_string_pretty(&rows)) {
                Ok(json) => print_external("search", &json),
                Err(e) => {
                    eprintln!("error: failed to encode search results: {e}");
                    process::exit(1);
                }
            }
            eprintln!("{} results", results.len());
        }
        Err(e) => {
            eprintln!("error: search failed: {e}");
            process::exit(1);
        }
    }
}
