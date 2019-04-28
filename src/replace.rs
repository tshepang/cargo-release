use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;

use crate::config::Replace;
use crate::error::FatalError;
use regex::Regex;

fn load_from_file(path: &Path) -> io::Result<String> {
    let mut file = r#try!(File::open(path));
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
pub fn replace_in(input: &str, r: &Replacements<'_>) -> String {
    let mut s = input.to_string();
    for (k, v) in r {
        s = s.replace(k, v);
    }
    s
}

pub fn do_file_replacements(
    replace_config: &[Replace],
    replacements: &Replacements<'_>,
    cwd: &Path,
    dry_run: bool,
) -> Result<bool, FatalError> {
    for replace in replace_config {
        let file = cwd.join(replace.file.as_path());
        let pattern = replace.search.as_str();
        let to_replace = replace.replace.as_str();

        let replacer = replace_in(to_replace, replacements);

        let data = load_from_file(&file)?;

        let r = Regex::new(pattern).map_err(FatalError::from)?;
        let result = r.replace_all(&data, replacer.as_str());

        if !dry_run {
            save_to_file(&file, &result)?;
        }
    }
    Ok(true)
}
