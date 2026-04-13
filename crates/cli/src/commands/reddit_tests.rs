use clap::Parser;

use super::reddit::RedditAction;
use super::{Cli, Command};

#[test]
fn cli_parses_reddit_feed() {
    let cli =
        Cli::try_parse_from(["cueward", "reddit", "feed", "rust"]).expect("parse reddit feed");

    match cli.command {
        Command::Reddit {
            action: RedditAction::Feed { subreddit, limit },
        } => {
            assert_eq!(subreddit, "rust");
            assert_eq!(limit, 20);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reddit_feed_with_prefixed_subreddit_and_limit() {
    let cli = Cli::try_parse_from(["cueward", "reddit", "feed", "r/rust", "--limit", "50"])
        .expect("parse reddit feed with limit");

    match cli.command {
        Command::Reddit {
            action: RedditAction::Feed { subreddit, limit },
        } => {
            assert_eq!(subreddit, "r/rust");
            assert_eq!(limit, 50);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reddit_post() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reddit",
        "post",
        "https://www.reddit.com/r/rust/comments/abc123/example_title/",
    ])
    .expect("parse reddit post");

    match cli.command {
        Command::Reddit {
            action: RedditAction::Post { url },
        } => assert_eq!(
            url,
            "https://www.reddit.com/r/rust/comments/abc123/example_title/"
        ),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reddit_search() {
    let cli = Cli::try_parse_from(["cueward", "reddit", "search", "async rust"])
        .expect("parse reddit search");

    match cli.command {
        Command::Reddit {
            action: RedditAction::Search {
                query,
                subreddit,
                limit,
            },
        } => {
            assert_eq!(query, "async rust");
            assert_eq!(subreddit, None);
            assert_eq!(limit, 10);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reddit_search_with_subreddit_and_limit() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reddit",
        "search",
        "async rust",
        "--subreddit",
        "r/rust",
        "--limit",
        "25",
    ])
    .expect("parse reddit search with subreddit");

    match cli.command {
        Command::Reddit {
            action: RedditAction::Search {
                query,
                subreddit,
                limit,
            },
        } => {
            assert_eq!(query, "async rust");
            assert_eq!(subreddit.as_deref(), Some("r/rust"));
            assert_eq!(limit, 25);
        }
        _ => panic!("unexpected command"),
    }
}
