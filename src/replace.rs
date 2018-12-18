use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;

use error::FatalError;
use regex::Regex;
use toml::Value;

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = try!(File::open(path));
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

fn save_to_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(&content.as_bytes())?;
    Ok(())
}

pub type Replacements<'a> = HashMap<&'a str, String>;
pub fn replace_in(input: &str, r: &Replacements) -> String {
    let mut s = input.to_string();
    for (k, v) in r {
        s = s.replace(k, v);
    }
    s
}

pub fn do_file_replacements(
    replace_config: &Value,
    replacements: &Replacements,
    dry_run: bool,
) -> Result<bool, FatalError> {
    if let &Value::Array(ref v) = replace_config {
        for tbl in v {
            if let &Value::Table(ref t) = tbl {
                let file = t
                    .get("file")
                    .and_then(|v| v.as_str())
                    .ok_or(FatalError::ReplacerConfigError)?;
                let pattern = t
                    .get("search")
                    .and_then(|v| v.as_str())
                    .ok_or(FatalError::ReplacerConfigError)?;
                let to_replace = t
                    .get("replace")
                    .and_then(|v| v.as_str())
                    .ok_or(FatalError::ReplacerConfigError)?;
                let replace_string = replace_in(to_replace, replacements);
                let replacer = replace_string.as_str();

                let data = load_from_file(&Path::new(file))?;

                let r = Regex::new(pattern).map_err(FatalError::from)?;
                let result = r.replace_all(&data, replacer);

                if !dry_run {
                    save_to_file(&Path::new(file), &result)?;
                }
            }
        }
    }
    Ok(true)
}
