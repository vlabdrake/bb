use std::fs;
use std::path::Path;

use chrono::{Locale, Utc};
use serde::{Deserialize, Serialize};

use crate::history;
use history::{get_file_history, Edit};

#[derive(Serialize)]
pub struct Context {
    title: String,
    description: Option<String>,
    image: Option<String>,
    published_time: String,
    last_modified_time: String,
    date: String,
    link: Option<String>,
    history: Vec<Edit>,
}

pub struct Page {
    pub context: Context,
    pub template: String,
}

impl Page {
    pub fn new(path: &Path, root: &Path) -> Page {
        let content = fs::read_to_string(&path).unwrap_or("".to_owned());
        let parts: Vec<&str> = content.splitn(2, "\n---\n").collect();
        let meta = Metadata::from_str(parts[0]);
        let template = parts[1].to_owned();
        let history = get_file_history(path).unwrap_or_default();
        let published_time = history
            .iter()
            .map(|edit| edit.datetime)
            .min()
            .unwrap_or(Utc::now());
        let last_modified_time = history
            .iter()
            .map(|edit| edit.datetime)
            .max()
            .unwrap_or(Utc::now());

        let context = Context {
            title: meta.title,
            description: meta.description,
            image: meta.image,
            published_time: published_time.to_rfc3339(),
            last_modified_time: last_modified_time.to_rfc3339(),
            date: published_time
                .format_localized("%e %B %Y", Locale::ru_RU)
                .to_string(),
            link: get_relative_link(path, root),
            history: history,
        };
        Page {
            context: context,
            template: template,
        }
    }
}

fn get_relative_link(path: &Path, root: &Path) -> Option<String> {
    let relative_path = path.strip_prefix(root).ok()?;
    let filename = relative_path.file_name()?;
    if filename == "index.html" {
        relative_path.parent()
    } else {
        Some(relative_path)
    }
    .map(|x| x.to_str().unwrap_or("").to_owned())
}

#[derive(Deserialize, Default)]
struct Metadata {
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
}

impl Metadata {
    pub fn from_str(s: &str) -> Metadata {
        toml::from_str(s).unwrap_or_default()
    }
}
