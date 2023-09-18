extern crate chrono;
extern crate git2;
extern crate minijinja;
extern crate serde;

use serde::{Deserialize, Serialize};

use chrono::prelude::*;
use git2::{Commit, DiffOptions, Repository};
use minijinja::{context, path_loader, Environment};
use std::collections::VecDeque;
use std::env;

use std::fs;
use std::path::{Path, PathBuf};

struct Page {
    pub meta: Metadata,
    pub template: String,
}

impl Page {
    fn new(p: &Path) -> Page {
        let content = fs::read_to_string(&p).unwrap_or("".to_owned());
        let parts: Vec<&str> = content.splitn(2, "\n---\n").collect();
        let mut meta: Metadata = toml::from_str(parts[0]).unwrap();
        meta.history = get_edits_for_file(p).unwrap_or_default();

        meta.published_time = meta
            .history
            .iter()
            .map(|edit| edit.datetime)
            .min()
            .unwrap_or(Utc::now());
        meta.last_modified_time = meta
            .history
            .iter()
            .map(|edit| edit.datetime)
            .max()
            .unwrap_or(Utc::now());
        Page {
            meta: meta,
            template: parts[1].to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct Metadata {
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
    #[serde(skip_deserializing)]
    pub published_time: DateTime<Utc>,
    #[serde(skip_deserializing)]
    pub last_modified_time: DateTime<Utc>,
    #[serde(skip_deserializing)]
    pub history: Vec<Edit>,
}

#[derive(Serialize)]
struct Edit {
    #[serde(with = "my_date_format")]
    pub datetime: DateTime<Utc>,
    pub summary: String,
    pub message: String,
}

mod my_date_format {
    use chrono::{DateTime, Locale, Utc};
    use serde::{self, Serializer};

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = date.format_localized("%e %B %Y", Locale::ru_RU).to_string();
        serializer.serialize_str(&s)
    }
}

fn is_file_changed_in_commit(repo: &Repository, commit: &Commit, path: &Path) -> bool {
    let parent_tree = commit.parent(0).ok().and_then(|commit| commit.tree().ok());

    let mut opts = DiffOptions::new();
    repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        commit.tree().ok().as_ref(),
        Some(&mut opts),
    )
    .ok()
    .map(|diff| {
        diff.deltas()
            .any(|dd| dd.new_file().path().map(|p| p.eq(path)).unwrap_or(false))
    })
    .unwrap_or(false)
}

fn get_commit_time(commit: &Commit) -> Option<DateTime<Utc>> {
    let t = commit.time();
    Utc.timestamp_opt(t.seconds(), 0).single()
}

fn get_edits_for_file(path: &Path) -> Option<Vec<Edit>> {
    let repo = Repository::discover(path).ok()?;
    let workdir = repo.workdir()?;
    let abs_path = fs::canonicalize(path).ok()?;
    let repo_path = fs::canonicalize(workdir).ok()?;
    let rel_path = abs_path.strip_prefix(repo_path).ok()?;
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.set_sorting(git2::Sort::TIME).ok()?;
    revwalk.push_head().ok()?;
    let mut history: Vec<Edit> = revwalk
        .filter_map(|rev| {
            rev.ok()
                .and_then(|oid| repo.find_commit(oid).ok()) // get commit
                .and_then(|commit| {
                    if is_file_changed_in_commit(&repo, &commit, &rel_path) {
                        Some(Edit {
                            datetime: get_commit_time(&commit).unwrap_or(Utc::now()),
                            summary: commit.summary().unwrap_or_default().to_owned(),
                            message: commit.message().unwrap_or_default().to_owned(),
                        })
                    } else {
                        None
                    }
                })
        })
        .collect();
    history.sort_by_key(|e| e.datetime);
    Some(history)
}

fn get_relative_link(relative_path: &Path) -> Option<&Path> {
    let filename = relative_path.file_name()?;
    if filename == "index.html" {
        relative_path.parent()
    } else {
        Some(relative_path)
    }
}

fn create_env(path: &str) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(path));
    env
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        println!("usage: bb src_dir build_dir");
        return;
    }
    let src = Path::new(&args[1]);
    let dst = Path::new(&args[2]);
    println!("{:?}", src);
    println!(
        "initialize minijinja with {:?}",
        src.join("_templates/**/*.html")
    );
    let env = create_env(&(src.join("_templates").to_str().unwrap()));
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::from(src));
    println!("let's roll");
    while let Some(dir) = queue.pop_front() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let filename = path.file_name().unwrap().to_str().unwrap();
            let relative_path = path.strip_prefix(src).unwrap();
            println!("{:?}", path);
            if filename.starts_with(".") || filename.starts_with("_") {
                continue;
            }
            if path.is_dir() {
                queue.push_back(path);
            } else {
                let dst_path = dst.join(&relative_path);
                let dst_parent = dst_path.parent().unwrap();
                if !dst_parent.exists() {
                    if let Err(err) = fs::create_dir_all(dst_parent) {
                        println!("{:?}", err);
                        continue;
                    }
                }
                let ext = path.extension();
                if ext.map_or(false, |e| e == "html") {
                    println!("write to {:?}", dst_path);
                    let page = Page::new(path.as_ref());

                    let result = env
                        .render_str(
                            &page.template,
                            context! {
                                title => &page.meta.title,
                                description => &page.meta.description,
                                image => &page.meta.image,
                                published_time => &page.meta.published_time.to_rfc3339(),
                                last_modified_time => &page.meta.last_modified_time.to_rfc3339(),
                                date =>
                                    &page
                                        .meta
                                        .published_time
                                        .format_localized("%e %B %Y", Locale::ru_RU)
                                        .to_string(),
                                link => &get_relative_link(relative_path),
                                history => &page.meta.history,
                            },
                        )
                        .unwrap();
                    if let Err(err) = fs::write(dst_path, result) {
                        println!("{:?}", err);
                    }
                } else {
                    println!("copy to {:?}", dst_path);
                    if let Err(err) = fs::copy(path.clone(), dst_path) {
                        println!("{:?}", err);
                    }
                }
            }
        }
    }
}
