use chrono::{Duration as ChronoDuration, Utc};
use std::time::{Duration, Instant};
use serde_json::json;

use super::{
    build_feed_url, build_search_url, compute_next_reddit_request, normalize_post_url,
    normalize_subreddit, parse_listing_posts, parse_post_result, parse_subreddit_info,
    reddit_backoff, should_retry_status, RedditComment,
};
use super::scan::filter_comments;

#[test]
fn normalize_subreddit_accepts_prefixed_or_plain_names() {
    assert_eq!(normalize_subreddit("rust").unwrap(), "rust");
    assert_eq!(normalize_subreddit("r/rust").unwrap(), "rust");
    assert_eq!(normalize_subreddit(" R/Rust ").unwrap(), "rust");
}

#[test]
fn normalize_subreddit_rejects_invalid_names() {
    assert!(normalize_subreddit("").is_err());
    assert!(normalize_subreddit("r/").is_err());
    assert!(normalize_subreddit("r/rust/top").is_err());
}

#[test]
fn normalize_post_url_rewrites_supported_hosts_to_old_reddit_json() {
    let url = normalize_post_url(
        "https://www.reddit.com/r/rust/comments/abc123/example_title/?utm_source=test",
    )
    .unwrap();

    assert_eq!(
        url,
        "https://old.reddit.com/r/rust/comments/abc123/example_title.json?limit=500"
    );
}

#[test]
fn normalize_post_url_ignores_query_and_fragment_without_trailing_slash() {
    let url = normalize_post_url(
        "https://www.reddit.com/r/rust/comments/abc123/example_title?context=3#top",
    )
    .unwrap();

    assert_eq!(
        url,
        "https://old.reddit.com/r/rust/comments/abc123/example_title.json?limit=500"
    );
}

#[test]
fn build_search_url_handles_global_and_subreddit_search() {
    assert_eq!(
        build_search_url("async rust", None, 10),
        "https://old.reddit.com/search.json?q=async%20rust&limit=10&sort=relevance"
    );
    assert_eq!(
        build_search_url("async rust", Some("rust"), 25),
        "https://old.reddit.com/r/rust/search.json?q=async%20rust&restrict_sr=on&limit=25&sort=relevance"
    );
    assert_eq!(build_feed_url("rust", 20), "https://old.reddit.com/r/rust.json?limit=20");
}

#[test]
fn reddit_backoff_is_small_and_linear() {
    assert_eq!(reddit_backoff(0), Duration::from_secs(2));
    assert_eq!(reddit_backoff(1), Duration::from_secs(4));
    assert_eq!(reddit_backoff(2), Duration::from_secs(6));
}

#[test]
fn compute_next_reddit_request_reserves_minimum_delay() {
    let now = Instant::now();
    let (delay, next_allowed) =
        compute_next_reddit_request(now, Some(now + Duration::from_millis(250)));

    assert!(delay.is_some());
    assert!(next_allowed > now);
}

#[test]
fn retryable_statuses_cover_429_and_server_errors() {
    assert!(should_retry_status(429));
    assert!(should_retry_status(500));
    assert!(should_retry_status(503));
    assert!(!should_retry_status(404));
}

#[test]
fn parse_subreddit_info_reads_about_payload() {
    let payload = json!({
        "kind": "t5",
        "data": {
            "display_name": "rust",
            "display_name_prefixed": "r/rust",
            "title": "The Rust Programming Language",
            "public_description": "Fearless concurrency",
            "subscribers": 123
        }
    });

    let info = parse_subreddit_info(&payload).unwrap();

    assert_eq!(info.name, "rust");
    assert_eq!(info.display_name, "r/rust");
    assert_eq!(info.title.as_deref(), Some("The Rust Programming Language"));
    assert_eq!(info.description.as_deref(), Some("Fearless concurrency"));
    assert_eq!(info.subscribers, Some(123));
}

#[test]
fn parse_subreddit_info_ignores_negative_subscriber_count() {
    let payload = json!({
        "kind": "t5",
        "data": {
            "display_name": "rust",
            "display_name_prefixed": "r/rust",
            "subscribers": -1
        }
    });

    let info = parse_subreddit_info(&payload).unwrap();

    assert_eq!(info.subscribers, None);
}

#[test]
fn parse_listing_posts_reads_posts_from_listing_payload() {
    let payload = json!({
        "kind": "Listing",
        "data": {
            "children": [{
                "kind": "t3",
                "data": {
                    "id": "abc123",
                    "title": "Rust 1.90 released",
                    "author": "example_user",
                    "subreddit": "rust",
                    "url": "https://www.rust-lang.org/",
                    "permalink": "/r/rust/comments/abc123/rust_190_released/",
                    "score": 420,
                    "num_comments": 37,
                    "created_utc": 1760000000,
                    "selftext": ""
                }
            }]
        }
    });

    let posts = parse_listing_posts(&payload).unwrap();

    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].id, "abc123");
    assert_eq!(
        posts[0].permalink,
        "https://reddit.com/r/rust/comments/abc123/rust_190_released/"
    );
}

#[test]
fn parse_post_result_returns_post_and_top_level_comments_only() {
    let payload = json!([
        {
            "kind": "Listing",
            "data": {
                "children": [{
                    "kind": "t3",
                    "data": {
                        "id": "abc123",
                        "title": "Rust 1.90 released",
                        "author": "example_user",
                        "subreddit": "rust",
                        "url": "https://www.rust-lang.org/",
                        "permalink": "/r/rust/comments/abc123/rust_190_released/",
                        "score": 420,
                        "num_comments": 37,
                        "created_utc": 1760000000,
                        "selftext": "release notes"
                    }
                }]
            }
        },
        {
            "kind": "Listing",
            "data": {
                "children": [
                    {
                        "kind": "t1",
                        "data": {
                            "id": "c1",
                            "author": "commenter1",
                            "body": "great release",
                            "score": 12,
                            "created_utc": 1760000100,
                            "permalink": "/r/rust/comments/abc123/rust_190_released/c1/"
                        }
                    },
                    {
                        "kind": "more",
                        "data": { "count": 3 }
                    }
                ]
            }
        }
    ]);

    let result = parse_post_result(&payload).unwrap();

    assert_eq!(result.post.id, "abc123");
    assert_eq!(result.comments.len(), 1);
    assert_eq!(result.comments[0].id, "c1");
}

#[test]
fn filter_comments_drops_deleted_short_and_old_comments() {
    let now = Utc::now();
    let comments = vec![
        RedditComment {
            id: "c1".to_string(),
            author: "[deleted]".to_string(),
            body: "this comment is long enough but deleted".to_string(),
            score: 1,
            created_utc: now.timestamp(),
            permalink: "https://reddit.com/c1".to_string(),
        },
        RedditComment {
            id: "c2".to_string(),
            author: "real_user".to_string(),
            body: "too short".to_string(),
            score: 1,
            created_utc: now.timestamp(),
            permalink: "https://reddit.com/c2".to_string(),
        },
        RedditComment {
            id: "c3".to_string(),
            author: "real_user".to_string(),
            body: "this comment is long enough but too old to keep around".to_string(),
            score: 1,
            created_utc: (now - ChronoDuration::days(31)).timestamp(),
            permalink: "https://reddit.com/c3".to_string(),
        },
        RedditComment {
            id: "c4".to_string(),
            author: "real_user".to_string(),
            body: "this comment is long enough and recent so it should survive".to_string(),
            score: 1,
            created_utc: now.timestamp(),
            permalink: "https://reddit.com/c4".to_string(),
        },
    ];

    let filtered = filter_comments(comments, now);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "c4");
}
