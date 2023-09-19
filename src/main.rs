extern crate chrono;
extern crate gix;
extern crate minijinja;
extern crate serde;

use serde::{Deserialize, Serialize};

use chrono::prelude::*;
use gix::object::tree::diff::Action;
use gix::{discover, Commit, Repository};
use minijinja::{path_loader, Environment};
use std::collections::VecDeque;
use std::env;

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Context {
    title: String,
    description: Option<String>,
    image: Option<String>,
    published_time: String,
    last_modified_time: String,
    date: String,
    link: Option<String>,
    history: Vec<Edit>,
}

struct Page {
    pub context: Context,
    pub template: String,
}

impl Page {
    fn new(path: &Path, root: &Path) -> Page {
        let content = fs::read_to_string(&path).unwrap_or("".to_owned());
        let parts: Vec<&str> = content.splitn(2, "\n---\n").collect();
        let meta: Metadata = toml::from_str(parts[0]).unwrap();
        let history = get_edits_for_file(path).unwrap_or_default();
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
            template: parts[1].to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct Metadata {
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
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
    is_file_changed_in_commit_helper(repo, commit, path).unwrap_or(false)
}

fn is_file_changed_in_commit_helper(
    repo: &Repository,
    commit: &Commit,
    path: &Path,
) -> Option<bool> {
    let tree = commit.tree().ok()?;
    let parent_tree = commit
        .parent_ids()
        .next()
        .and_then(|id| id.object().ok()?.try_into_commit().ok()?.tree().ok())
        .unwrap_or(repo.empty_tree());

    let mut changed = false;
    let _ = parent_tree
        .changes()
        .ok()?
        .track_path()
        .for_each_to_obtain_tree(&tree, |change| {
            changed = path.to_str().map(|p| p == change.location).unwrap_or(false);
            let action = if changed {
                Action::Cancel
            } else {
                Action::Continue
            };
            Ok::<_, std::io::Error>(action)
        });
    Some(changed)
}

fn make_edit(commit: &Commit) -> Option<Edit> {
    let message = commit.message().ok()?;
    let t = commit.time().ok()?;
    Some(Edit {
        datetime: Utc
            .timestamp_opt(t.seconds, 0)
            .single()
            .unwrap_or(Utc::now()),
        summary: message.summary().to_string(),
        message: message.title.to_string(),
    })
}

fn get_edits_for_file(path: &Path) -> Option<Vec<Edit>> {
    let repo = discover(path.parent()?).ok()?;
    let workdir = repo.work_dir()?;
    let abs_path = fs::canonicalize(path).ok()?;
    let repo_path = fs::canonicalize(workdir).ok()?;
    let rel_path = abs_path.strip_prefix(repo_path).ok()?;
    let mut history: Vec<Edit> = repo
        .rev_walk([repo.head_id().ok()?])
        .all()
        .ok()?
        .filter_map(|rev| {
            rev.ok()
                .and_then(|info| info.object().ok()) // get commit
                .and_then(|commit| {
                    if is_file_changed_in_commit(&repo, &commit, &rel_path) {
                        make_edit(&commit)
                    } else {
                        None
                    }
                })
        })
        .collect();
    history.sort_by_key(|e| e.datetime);
    Some(history)
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

fn create_env(path: &Path) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(path));
    env
}

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|x| x.to_str())
        .map(|f| f.starts_with(".") || f.starts_with("_"))
        .unwrap_or(false)
}

fn walk<E: std::fmt::Debug>(root: &Path, for_each: impl Fn(&Path) -> Result<(), E>) {
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::from(root));
    while let Some(dir) = queue.pop_front() {
        if let Some(entries) = fs::read_dir(dir).ok() {
            for entry in entries {
                if let Some(entry) = entry.ok() {
                    let path = entry.path();

                    if is_ignored(&path) {
                        continue;
                    }

                    if path.is_dir() {
                        queue.push_back(path);
                        continue;
                    }

                    if let Err(err) = for_each(&path) {
                        println!("{:?}", err);
                    }
                }
            }
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        println!("usage: {} src_dir build_dir", args[0]);
        return;
    }
    let src = Path::new(&args[1]);
    let dst = Path::new(&args[2]);
    println!("initialize minijinja with {:?}", src.join("_templates"));
    let env = create_env(&src.join("_templates"));
    walk(src, |path| {
        let relative_path = path.strip_prefix(src).unwrap();
        let dst_path = dst.join(&relative_path);
        let dst_parent = dst_path.parent().unwrap();
        if !dst_parent.exists() {
            fs::create_dir_all(dst_parent)?;
        }
        if path.extension().map_or(false, |ext| ext == "html") {
            println!("render {:?} to {:?}", path, dst_path);
            let page = Page::new(path.as_ref(), src);
            let result = env.render_str(&page.template, &page.context).unwrap();
            fs::write(dst_path, result)?;
        } else {
            println!("copy {:?} to {:?}", path, dst_path);
            fs::copy(path.clone(), dst_path)?;
        }
        Ok::<(), std::io::Error>(())
    });
}
