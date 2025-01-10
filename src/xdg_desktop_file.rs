use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::Path,
};

pub fn args_from_exec_string(exec: &str, arg: &OsStr) -> Option<(String, Vec<OsString>)> {
    let mut tokens = shlex::split(exec)?;
    if tokens.is_empty() {
        return None;
    }
    let exec = tokens.remove(0);
    let args = tokens
        .into_iter()
        .map(|tok| {
            if tok == "%U" || tok == "%u" || tok == "%f" {
                arg.to_owned()
            } else {
                tok.into()
            }
        })
        .collect();
    Some((exec, args))
}

type DesktopMap = HashMap<String, String>;

enum ParseStatus {
    // Initial status, trying to find desktop entry group
    Init,
    // Parsing desktop entry
    DesktopEntry,
}

pub fn parse_desktop_file(path: impl AsRef<Path>) -> Result<DesktopMap, std::io::Error> {
    // .desktop files are UTF-8 according to spec
    let mut status = ParseStatus::Init;
    let mut map = HashMap::new();
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        match status {
            ParseStatus::Init => {
                if line.trim() == "[Desktop Entry]" {
                    status = ParseStatus::DesktopEntry;
                }
            }
            ParseStatus::DesktopEntry => {
                // Another group is starting, we're not interested
                if line.trim().starts_with('[') {
                    return Ok(map);
                }
                // Collect key-value pairs
                if let Some((k, v)) = line.split_once('=') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }
    Ok(map)
}
