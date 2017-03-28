use std::io::prelude::*;
use std::io;
use std::fs::{self, File};
use std::path::Path;

use regex::Regex;
use error::FatalError;
use toml::Value;

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = try!(File::open(path));
    let mut s = String::new();
    try!(file.read_to_string(&mut s));
    Ok(s)
}

fn save_to_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = try!(File::create(path));
    try!(file.write_all(&content.as_bytes()));
    Ok(())
}

pub fn do_replace_versions(replace_config: &Value, version: &str) -> Result<bool, FatalError> {

    if let &Value::Array(ref v) = replace_config {
        for tbl in v {
            if let &Value::Table(ref t) = tbl {
                let file = t.get("file").and_then(|v| v.as_str()).unwrap();
                let pattern = t.get("regex").and_then(|v| v.as_str()).unwrap();

                let data = try!(load_from_file(&Path::new(file)));

                let r = Regex::new(pattern).unwrap();
                let result = r.replace_all(&data, version);

                try!(save_to_file(&Path::new(file), &result));
            }
        }
    }
    Ok(true)
}
