extern crate chrono;
extern crate serde;
extern crate tera;

use serde::Deserialize;

use chrono::prelude::*;
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
    fn new(p: PathBuf) -> Page {
        let content = fs::read_to_string(p).unwrap();
        let parts: Vec<&str> = content.splitn(2, "\n---\n").collect();
        let config: Config = toml::from_str(parts[0]).unwrap();
        Page {
            meta: Metadata {
                title: config.title,
                published_time: Utc::now(),
                modified_time: Utc::now(),
            },
            template: parts[1].to_owned(),
        }
    }
}

struct Metadata {
    pub title: String,
    pub published_time: DateTime<Utc>,
    pub modified_time: DateTime<Utc>,
}

#[derive(Deserialize)]
struct Config {
    title: String,
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
                if ext != None && ext.unwrap() == "html" {
                    println!("write to {:?}", dst_path);
                    let page = Page::new(path);
                    let mut context = Context::new();
                    context.insert("title", &page.meta.title);
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
