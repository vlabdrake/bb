mod history;
mod page;

use minijinja::{path_loader, Environment};
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use page::Page;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        println!("usage: {} src_dir build_dir", args[0]);
        return;
    }
    build(&args[1], &args[2]);
}

fn build<P: AsRef<Path>>(src: P, dst: P) {
    println!(
        "initialize minijinja with {:?}",
        src.as_ref().join("_templates")
    );
    let env = create_env(&src.as_ref().join("_templates"));
    walk(src.as_ref(), |path| {
        let relative_path = path.strip_prefix(src.as_ref()).unwrap();
        let dst_path = dst.as_ref().join(&relative_path);
        let dst_parent = dst_path.parent().unwrap();
        if !dst_parent.exists() {
            fs::create_dir_all(dst_parent)?;
        }
        if path.extension().map_or(false, |ext| ext == "html") {
            println!("render {:?} to {:?}", path, dst_path);
            let page = Page::new(path.as_ref(), src.as_ref());
            let result = env.render_str(&page.template, &page.context).unwrap();
            fs::write(dst_path, result)?;
        } else {
            println!("copy {:?} to {:?}", path, dst_path);
            fs::copy(path.clone(), dst_path)?;
        }
        Ok::<(), std::io::Error>(())
    });
}

fn create_env(path: &Path) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(path));
    env
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

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|x| x.to_str())
        .map(|f| f.starts_with(".") || f.starts_with("_"))
        .unwrap_or(false)
}
