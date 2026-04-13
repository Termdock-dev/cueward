use std::process;

use clap::Subcommand;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum RedditAction {
    /// Read a subreddit feed
    Feed {
        /// Subreddit name, e.g. rust or r/rust
        subreddit: String,
        /// Max posts to return
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Read a Reddit post and its top-level comments
    Post {
        /// Reddit post URL
        url: String,
    },
    /// Search Reddit posts
    Search {
        /// Search query
        query: String,
        /// Restrict search to a subreddit, e.g. rust or r/rust
        #[arg(long)]
        subreddit: Option<String>,
        /// Max posts to return
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

fn print_serialized_or_exit(source: &str, serialized: Result<String, serde_json::Error>) {
    match serialized {
        Ok(json) => print_external(source, &json),
        Err(error) => {
            eprintln!("error: failed to serialize result: {error}");
            process::exit(1);
        }
    }
}

pub(crate) fn dispatch(action: RedditAction) {
    match action {
        RedditAction::Feed { subreddit, limit } => {
            match cueward_adapter_macos::reddit::feed(&subreddit, limit) {
                Ok(result) => {
                    print_serialized_or_exit("reddit/feed", serde_json::to_string_pretty(&result));
                    eprintln!("{} post(s)", result.posts.len());
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    process::exit(1);
                }
            }
        }
        RedditAction::Post { url } => match cueward_adapter_macos::reddit::post(&url) {
            Ok(result) => {
                print_serialized_or_exit("reddit/post", serde_json::to_string_pretty(&result));
                eprintln!("{} top-level comment(s)", result.comments.len());
            }
            Err(error) => {
                eprintln!("error: {error}");
                process::exit(1);
            }
        },
        RedditAction::Search {
            query,
            subreddit,
            limit,
        } => match cueward_adapter_macos::reddit::search(&query, subreddit.as_deref(), limit) {
            Ok(result) => {
                print_serialized_or_exit("reddit/search", serde_json::to_string_pretty(&result));
                eprintln!("{} post(s)", result.posts.len());
            }
            Err(error) => {
                eprintln!("error: {error}");
                process::exit(1);
            }
        },
    }
}
