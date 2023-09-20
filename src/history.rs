use std::fs;
use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use gix::object::tree::diff::Action;
use gix::{discover, Commit, Repository};
use serde::Serialize;

#[derive(Serialize)]
pub struct Edit {
    #[serde(with = "my_date_format")]
    pub datetime: DateTime<Utc>,
    pub summary: String,
    pub message: String,
}

pub fn get_file_history(path: &Path) -> Option<Vec<Edit>> {
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
