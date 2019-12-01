use std::path::Path;

use regex::Regex;

use crate::config::Replace;
use crate::error::FatalError;

#[derive(Clone, Default, Debug)]
pub struct Template<'a> {
    pub prev_version: Option<&'a str>,
    pub version: Option<&'a str>,
    pub crate_name: Option<&'a str>,
    pub date: Option<&'a str>,

    pub prefix: Option<&'a str>,
    pub tag_name: Option<&'a str>,
    pub next_version: Option<&'a str>,
}

impl<'a> Template<'a> {
    pub fn render(&self, input: &str) -> String {
        let mut s = input.to_string();
        if let Some(prev_version) = self.prev_version {
            s = s.replace("{{prev_version}}", prev_version);
        }
        if let Some(version) = self.version {
            s = s.replace("{{version}}", version);
        }
        if let Some(crate_name) = self.crate_name {
            s = s.replace("{{crate_name}}", crate_name);
        }
        if let Some(date) = self.date {
            s = s.replace("{{date}}", date);
        }

        if let Some(prefix) = self.prefix {
            s = s.replace("{{prefix}}", prefix);
        }
        if let Some(tag_name) = self.tag_name {
            s = s.replace("{{tag_name}}", tag_name);
        }
        if let Some(next_version) = self.next_version {
            s = s.replace("{{next_version}}", next_version);
        }
        s
    }
}

pub fn do_file_replacements(
    replace_config: &[Replace],
    template: &Template<'_>,
    cwd: &Path,
    prerelease: bool,
    dry_run: bool,
) -> Result<bool, FatalError> {
    for replace in replace_config {
        if replace.prerelease && prerelease {
            log::debug!("Pre-release, not replacing {}", replace.search);
            continue;
        }

        let to_replace = replace.replace.as_str();
        let replacer = template.render(to_replace);

        let file = cwd.join(replace.file.as_path());
        log::debug!("Substituting values for {}", file.display());
        let data = std::fs::read_to_string(&file)?;

        let pattern = replace.search.as_str();
        let r = Regex::new(pattern).map_err(FatalError::from)?;
        let min = replace.min.or(replace.exactly).unwrap_or(1);
        let max = replace.min.or(replace.exactly).unwrap_or(std::usize::MAX);
        let actual = r.find_iter(&data).count();
        if actual < min {
            return Err(FatalError::ReplacerMinError(
                pattern.to_owned(),
                min,
                actual,
            ))?;
        } else if max < actual {
            return Err(FatalError::ReplacerMaxError(
                pattern.to_owned(),
                min,
                actual,
            ))?;
        }

        let result = r.replace_all(&data, replacer.as_str());

        if dry_run {
            let ch = difference::Changeset::new(&data, &result, "\n");
            log::trace!("Change:\n{}", ch);
        } else {
            std::fs::write(&file, result.as_ref())?;
        }
    }
    Ok(true)
}
