use {
    rfd::{MessageDialog, MessageDialogResult},
    std::{
        collections::HashMap,
        ffi::{OsStr, OsString},
        path::Path,
        process::Command,
        str::Utf8Error,
    },
    thiserror::Error,
};

#[macro_use]
mod dbg_box;

fn is_string_url(str: &str) -> bool {
    str.starts_with("file://") || str.starts_with("http://") || str.starts_with("https://")
}

fn is_url(arg: &OsStr) -> bool {
    match arg.to_str() {
        Some(str) => is_string_url(str),
        None => false,
    }
}

#[derive(Error, Debug)]
enum XdgQueryError {
    #[error("Invalid utf-8: {0}")]
    InvalidUtf8(Utf8Error),
    #[error("Empty response")]
    Empty,
}

fn query_mime(arg: &OsStr) -> Result<String, XdgQueryError> {
    let out = Command::new("xdg-mime")
        .args(["query".as_ref(), "filetype".as_ref(), arg])
        .output()
        .unwrap()
        .stdout;
    match std::str::from_utf8(&out) {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Err(XdgQueryError::Empty)
            } else {
                Ok(trimmed.to_string())
            }
        }
        Err(e) => Err(XdgQueryError::InvalidUtf8(e)),
    }
}

fn query_default(mime: &str) -> Result<String, XdgQueryError> {
    let out = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output()
        .unwrap()
        .stdout;
    match std::str::from_utf8(&out) {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Err(XdgQueryError::Empty)
            } else {
                Ok(trimmed.to_string())
            }
        }
        Err(e) => Err(XdgQueryError::InvalidUtf8(e)),
    }
}

fn fallback_default(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "application/vnd.microsoft.portable-executable" => "wine",
        _ => return None,
    })
}

fn open_with(command: impl AsRef<OsStr>, args: &[impl AsRef<OsStr>]) {
    if let Err(e) = Command::new(command).args(args).spawn() {
        MessageDialog::new().set_description(format!("Error: {e}"));
    }
}

fn open(arg: &OsStr) {
    //let debug = matches!(std::env::var("RUSTY_OPEN_DEBUG").as_deref(), Ok("hello"));
    let debug = true; // During development
    let is_url = is_url(arg);
    if is_url {
        open_with("firefox", &[arg]);
    } else {
        let mime = match query_mime(arg) {
            Ok(mime) => mime,
            Err(e) => {
                MessageDialog::new()
                    .set_description(format!("Error: {e}"))
                    .show();
                return;
            }
        };
        let default = match query_default(&mime) {
            Ok(def) => Some(def),
            Err(XdgQueryError::InvalidUtf8(e)) => {
                dbg_box!(e);
                return;
            }
            Err(XdgQueryError::Empty) => fallback_default(&mime).map(|s| s.to_string()),
        };
        match default {
            Some(default) => {
                let mut args = &[arg.to_owned()][..];
                let mut to_exec = &default;
                let parsed_args;
                let parsed_exec;
                if default.ends_with(".desktop") {
                    let desktop_map = match parse_desktop_file(
                        Path::new("/usr/share/applications").join(&default),
                    ) {
                        Ok(map) => map,
                        Err(_) => {
                            match parse_desktop_file(
                                dirs::data_dir()
                                    .unwrap()
                                    .join("applications")
                                    .join(&default),
                            ) {
                                Ok(map) => map,
                                Err(_) => {
                                    MessageDialog::new()
                                        .set_description(format!(
                                            "Could not find matching .desktop file for {default}"
                                        ))
                                        .show();
                                    return;
                                }
                            }
                        }
                    };
                    if let Some(exec) = desktop_map.get("Exec") {
                        (parsed_exec, parsed_args) = args_from_exec_string(exec, arg);
                        args = &parsed_args[..];
                        to_exec = &parsed_exec;
                    }
                }
                let mut ok = true;
                if debug {
                    let msg = format!(
                        "Arg: {arg}\nMime: {mime}\nDefault: {default}\nExecutable: {to_exec}\nArgs: {args:?}",
                        arg = arg.to_string_lossy()
                    );
                    if MessageDialog::new()
                        .set_description(msg)
                        .set_buttons(rfd::MessageButtons::OkCancel)
                        .show()
                        == MessageDialogResult::Cancel
                    {
                        ok = false;
                    }
                }
                if ok {
                    open_with(to_exec, args);
                }
            }
            None => {
                MessageDialog::new().set_description("No default app could be determined");
            }
        }
    }
}

fn args_from_exec_string(exec: &str, arg: &OsStr) -> (String, Vec<OsString>) {
    let mut tokens = exec.split_whitespace();
    let exec = tokens.next().unwrap().to_string();
    let args = tokens
        .map(|tok| {
            if tok == "%U" || tok == "%u" || tok == "%f" {
                arg.to_owned()
            } else {
                tok.into()
            }
        })
        .collect();
    (exec, args)
}

type DesktopMap = HashMap<String, String>;

fn parse_desktop_file(path: impl AsRef<Path>) -> Result<DesktopMap, std::io::Error> {
    // .desktop files are UTF-8 according to spec
    let mut map = HashMap::new();
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}

fn main() {
    match std::env::args_os().nth(1) {
        Some(arg) => open(&arg),
        None => {
            MessageDialog::new()
                .set_title("Rusty-open")
                .set_description("No arguments provided, nothing to open")
                .show();
        }
    }
}
