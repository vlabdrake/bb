extern crate chrono;
extern crate git2;
extern crate serde;
extern crate tera;

use serde::Deserialize;

use chrono::prelude::*;
use git2::{Commit, DiffOptions, Repository};
use std::collections::VecDeque;
use std::env;
use tera::{Context, Tera};

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
        let (published, modified) =
            get_times_for_path(p.as_ref()).unwrap_or((Utc::now(), Utc::now()));
        meta.published_time = published;
        meta.modified_time = modified;
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
    pub modified_time: DateTime<Utc>,
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

fn get_times_for_path(path: &Path) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let repo = Repository::discover(path).ok()?;
    let workdir = repo.workdir()?;
    let abs_path = fs::canonicalize(path).ok()?;
    let repo_path = fs::canonicalize(workdir).ok()?;
    let rel_path = abs_path.strip_prefix(repo_path).ok()?;
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.set_sorting(git2::Sort::TIME).ok()?;
    revwalk.push_head().ok()?;
    let mut modification_timestamps = revwalk.filter_map(|rev| {
        rev.ok()
            .and_then(|oid| repo.find_commit(oid).ok()) // get commit
            .and_then(|commit| {
                // get commit time
                if is_file_changed_in_commit(&repo, &commit, &rel_path) {
                    get_commit_time(&commit)
                } else {
                    None
                }
            })
    });
    let modified = modification_timestamps.next()?;
    let created = modification_timestamps.last().unwrap_or(modified);
    Some((created, modified))
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
        "initialize terra with {:?}",
        src.join("_templates/**/*.html")
    );
    let mut tera = Tera::new(&(src.join("_templates/**/*.html").to_str().unwrap())).unwrap();
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::from(src));
    println!("let's roll");
    while let Some(dir) = queue.pop_front() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let filename = path.file_name().unwrap().to_str().unwrap();
            println!("{:?}", path);
            if filename.starts_with(".") || filename.starts_with("_") {
                continue;
            }
            if path.is_dir() {
                queue.push_back(path);
            } else {
                let dst_path = dst.join(path.clone().strip_prefix(src).unwrap());
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
                    let mut context = Context::new();
                    context.insert("title", &page.meta.title);
                    context.insert("description", &page.meta.description);
                    context.insert("image", &page.meta.image);
                    context.insert("published_time", &page.meta.published_time.to_rfc3339());
                    context.insert("modified_time", &page.meta.modified_time.to_rfc3339());
                    context.insert(
                        "date",
                        &page
                            .meta
                            .published_time
                            .format_localized("%e %B %Y", Locale::ru_RU)
                            .to_string(),
                    );
                    let result = tera.render_str(&page.template, &context).unwrap();
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
