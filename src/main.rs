use {
    rfd::{MessageDialog, MessageDialogResult},
    std::{ffi::OsStr, process::Command, str::Utf8Error},
    thiserror::Error,
};

#[macro_use]
mod dbg_box;

fn is_string_url(str: &str) -> bool {
    str.starts_with("file://")
}

fn is_url(arg: &OsStr) -> bool {
    match arg.to_str() {
        Some(str) => is_string_url(str),
        None => false,
    }
}

#[derive(Error, Debug)]
enum MimeQueryError {
    #[error("Invalid utf-8: {0}")]
    InvalidUtf8(Utf8Error),
    #[error("Empty mime")]
    Empty,
}

fn query_mime(arg: &OsStr) -> Result<String, MimeQueryError> {
    let out = Command::new("xdg-mime")
        .args(["query".as_ref(), "filetype".as_ref(), arg])
        .output()
        .unwrap()
        .stdout;
    match std::str::from_utf8(&out) {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Err(MimeQueryError::Empty)
            } else {
                Ok(trimmed.to_string())
            }
        }
        Err(e) => Err(MimeQueryError::InvalidUtf8(e)),
    }
}

fn query_default(mime: &str) -> String {
    String::from_utf8_lossy(
        &Command::new("xdg-mime")
            .args(["query", "default", mime])
            .output()
            .unwrap()
            .stdout,
    )
    .into_owned()
}

fn fallback_default(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "application/vnd.microsoft.portable-executable" => "wine",
        _ => return None,
    })
}

fn open_with(command: impl AsRef<OsStr>, arg: &OsStr) {
    if let Err(e) = Command::new(command).arg(arg).spawn() {
        MessageDialog::new().set_description(format!("Error: {e}"));
    }
}

fn open(arg: &OsStr) {
    //let debug = matches!(std::env::var("RUSTY_OPEN_DEBUG").as_deref(), Ok("hello"));
    let debug = true; // During development
    let is_url = is_url(arg);
    if is_url {
        open_with("firefox", arg);
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
        let xdg_default = query_default(&mime);
        let default = if xdg_default.is_empty() {
            fallback_default(&mime).map(|s| s.to_string())
        } else {
            Some(xdg_default)
        };
        match default {
            Some(default) => {
                let mut ok = true;
                if debug {
                    let msg = format!(
                        "Arg: {arg}\nMime: {mime}\nDefault: {default}",
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
                    open_with(default, arg);
                }
            }
            None => {
                MessageDialog::new().set_description("No default app could be determined");
            }
        }
    }
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
