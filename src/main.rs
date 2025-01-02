use {
    detect_desktop_environment::DesktopEnvironment,
    rfd::{MessageDialog, MessageDialogResult},
    std::{
        collections::HashMap,
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        process::Command,
        str::Utf8Error,
    },
    thiserror::Error,
};

#[macro_use]
mod dbg_box;
mod generic_xdg;
mod qt_xdg;

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

trait QueryExt {
    fn query_mime(&self, arg: &OsStr) -> Result<String, XdgQueryError>;
    fn query_default(&self, mime: &str) -> Result<String, XdgQueryError>;
}

impl QueryExt for Option<DesktopEnvironment> {
    fn query_mime(&self, arg: &OsStr) -> Result<String, XdgQueryError> {
        match self {
            Some(DesktopEnvironment::Lxqt) => qt_xdg::query_mime(arg),
            _ => generic_xdg::query_mime_xdg(arg),
        }
    }

    fn query_default(&self, mime: &str) -> Result<String, XdgQueryError> {
        match self {
            Some(DesktopEnvironment::Lxqt) => qt_xdg::query_default(mime),
            _ => generic_xdg::query_default(mime),
        }
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

fn open(arg: &OsStr, de: Option<DesktopEnvironment>) {
    //let debug = matches!(std::env::var("RUSTY_OPEN_DEBUG").as_deref(), Ok("hello"));
    let debug = true; // During development
    let is_url = is_url(arg);
    if is_url {
        open_with("firefox", &[arg]);
    } else {
        let mime = match de.query_mime(arg) {
            Ok(mime) => mime,
            Err(e) => {
                MessageDialog::new()
                    .set_description(format!("Error: {e}"))
                    .show();
                return;
            }
        };
        let default = match de.query_default(&mime) {
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
                let mut appfile_path = PathBuf::default();
                if default.ends_with(".desktop") {
                    appfile_path = Path::new("/usr/share/applications").join(&default);
                    let desktop_map = match parse_desktop_file(&appfile_path) {
                        Ok(map) => map,
                        Err(_) => {
                            appfile_path = dirs::data_dir()
                                .unwrap()
                                .join("applications")
                                .join(&default);
                            match parse_desktop_file(&appfile_path) {
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
                        "Arg: {arg}\nDE: {de}\nMime: {mime}\nApp file: {appfile_path}\nExecutable: {to_exec}\nArgs: {args:?}",
                        arg = arg.to_string_lossy(),
                        appfile_path = appfile_path.display(),
                        de = de_opt_str(de),
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
                MessageDialog::new()
                    .set_description(format!(
                        "No default app could be determined for mime {mime}"
                    ))
                    .show();
            }
        }
    }
}

fn de_opt_str(de: Option<DesktopEnvironment>) -> &'static str {
    match de {
        Some(de) => match de {
            DesktopEnvironment::Cinnamon => "Cinnamon",
            DesktopEnvironment::Cosmic => "Cosmic",
            DesktopEnvironment::Dde => "Dde",
            DesktopEnvironment::Ede => "Ede",
            DesktopEnvironment::Endless => "Endless",
            DesktopEnvironment::Enlightenment => "Enlightenment",
            DesktopEnvironment::Gnome => "Gnome",
            DesktopEnvironment::Hyprland => "Hyprland",
            DesktopEnvironment::Kde => "KDE",
            DesktopEnvironment::Lxde => "LXDE",
            DesktopEnvironment::Lxqt => "LXQt",
            DesktopEnvironment::MacOs => "Mac OS",
            DesktopEnvironment::Mate => "Mate",
            DesktopEnvironment::Old => "Old",
            DesktopEnvironment::Pantheon => "Pantheon",
            DesktopEnvironment::Razor => "Razor",
            DesktopEnvironment::Rox => "Rox",
            DesktopEnvironment::Sway => "Sway",
            DesktopEnvironment::Tde => "Tde",
            DesktopEnvironment::Unity => "Unity",
            DesktopEnvironment::Windows => "Windows",
            DesktopEnvironment::Xfce => "Xfce",
            _ => "TODO",
        },
        None => "<Could not detect desktop environment>",
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

enum ParseStatus {
    // Initial status, trying to find desktop entry group
    Init,
    // Parsing desktop entry
    DesktopEntry,
}

fn parse_desktop_file(path: impl AsRef<Path>) -> Result<DesktopMap, std::io::Error> {
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

fn main() {
    let de = DesktopEnvironment::detect();
    match std::env::args_os().nth(1) {
        Some(arg) => open(&arg, de),
        None => {
            MessageDialog::new()
                .set_title("Rusty-open")
                .set_description("No arguments provided, nothing to open")
                .show();
        }
    }
}
