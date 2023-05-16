extern crate tera;

use std::env;
use std::collections::VecDeque;
use tera::{Context, Tera};


use std::fs;
use std::path::{Path, PathBuf};


fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        println!("usage: bb src_dir build_dir");
        return
    }
    let src = Path::new(&args[1]);
    let dst = Path::new(&args[2]);
    println!("{:?}", src);
    let context = Context::new();
    println!("initialize terra with {:?}", src.join("**/*.html"));
    let tera = Tera::new(&(src.join("**/*.html").to_str().unwrap())).unwrap();
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::from(src));
    println!("let's roll");
    while let Some(dir) = queue.pop_front() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let filename = path.file_name().unwrap().to_str().unwrap();
            println!("{:?}", path);
            if filename.starts_with(".") || filename.starts_with("_") {continue;}
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
                    let template = &path.strip_prefix(src).unwrap().to_str().unwrap();
                    let result = tera.render(template, &context).unwrap();
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