use std::io::prelude::*;
use std::io;
use std::fs::File;
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

pub fn do_replace_versions(replace_config: &Value,
                           version: &str,
                           dry_run: bool)
                           -> Result<bool, FatalError> {

    if let &Value::Array(ref v) = replace_config {
        for tbl in v {
            if let &Value::Table(ref t) = tbl {
                let file = try!(t.get("file")
                                    .and_then(|v| v.as_str())
                                    .ok_or(FatalError::ReplacerConfigError));
                let pattern = try!(t.get("search")
                                       .and_then(|v| v.as_str())
                                       .ok_or(FatalError::ReplacerConfigError));
                let replace = try!(t.get("replace")
                                       .and_then(|v| v.as_str())
                                       .ok_or(FatalError::ReplacerConfigError));
                let replace_with_version = replace.replace("{{version}}", version);
                let replacer = replace_with_version.as_str();

                let data = try!(load_from_file(&Path::new(file)));

                let r = try!(Regex::new(pattern).map_err(FatalError::from));
                let result = r.replace_all(&data, replacer);

                if !dry_run {
                    try!(save_to_file(&Path::new(file), &result));
                }
            }
        }
    }
    Ok(true)
}
