use std::collections::BTreeMap;
use std::env::current_dir;
use std::path::Path;
use std::process::Command;

use error::FatalError;

fn do_call(
    command: Vec<&str>,
    path: Option<&Path>,
    envs: Option<BTreeMap<&str, &str>>,
    dry_run: bool,
) -> Result<bool, FatalError> {
    if dry_run {
        if path.is_some() {
            println!("cd {}", path.unwrap().display());
        }
        println!("{}", command.join(" "));
        if path.is_some() {
            println!("cd -");
        }
        return Ok(true);
    }
    let mut iter = command.iter();
    let cmd_name = iter.next().unwrap();

    let mut cmd = Command::new(cmd_name);

    if let Some(p) = path {
        cmd.current_dir(p);
    }

    if let Some(e) = envs {
        for (key, val) in e.iter() {
            cmd.env(key, val);
        }
    }

    for arg in iter {
        if arg.len() > 0 {
            cmd.arg(arg);
        }
    }

    let mut child = cmd.spawn().map_err(FatalError::from)?;
    let result = child.wait().map_err(FatalError::from)?;

    Ok(result.success())
}

pub fn call(command: Vec<&str>, dry_run: bool) -> Result<bool, FatalError> {
    do_call(command, None, None, dry_run)
}

pub fn call_on_path(command: Vec<&str>, path: &Path, dry_run: bool) -> Result<bool, FatalError> {
    do_call(command, Some(path), None, dry_run)
}

pub fn call_with_env(
    command: Vec<&str>,
    envs: BTreeMap<&str, &str>,
    dry_run: bool,
) -> Result<bool, FatalError> {
    do_call(command, None, Some(envs), dry_run)
}

pub fn is_current_path(path: &Path) -> Result<bool, FatalError> {
    Ok(current_dir()? == path)
}
