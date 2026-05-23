use crate::models::{parse_iso_datetime, HomeworkItem};
use chrono::{DateTime, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct CachePayload {
    updated_at: String,
    items: Vec<HomeworkCacheItem>,
}

#[derive(Serialize, Deserialize)]
struct HomeworkCacheItem {
    title: String,
    course: String,
    deadline: Option<String>,
    platform: String,
    submitted: bool,
    url: String,
}

fn cache_path() -> PathBuf {
    crate::config::app_dir().join("homework_cache.json")
}

pub fn save_homework_cache(items: &[HomeworkItem]) -> std::io::Result<()> {
    let payload = CachePayload {
        updated_at: Local::now().to_rfc3339(),
        items: items
            .iter()
            .map(|h| HomeworkCacheItem {
                title: h.title.clone(),
                course: h.course.clone(),
                deadline: h.deadline.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()),
                platform: h.platform.clone(),
                submitted: h.submitted,
                url: h.url.clone(),
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&payload).map_err(std::io::Error::other)?;
    fs::write(cache_path(), text)
}

pub fn load_homework_cache() -> (Vec<HomeworkItem>, Option<NaiveDateTime>) {
    let path = cache_path();
    if !path.exists() {
        return (vec![], None);
    }
    let Ok(text) = fs::read_to_string(path) else {
        return (vec![], None);
    };
    let Ok(payload) = serde_json::from_str::<CachePayload>(&text) else {
        return (vec![], None);
    };
    let items = payload
        .items
        .into_iter()
        .map(|d| HomeworkItem {
            title: d.title,
            course: d.course,
            deadline: d.deadline.as_deref().and_then(parse_iso_datetime),
            platform: d.platform,
            submitted: d.submitted,
            url: d.url,
        })
        .collect();
    let updated_at = DateTime::parse_from_rfc3339(&payload.updated_at)
        .ok()
        .map(|dt| dt.naive_local());
    (items, updated_at)
}
