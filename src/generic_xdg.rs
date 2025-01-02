use {
    crate::XdgQueryError,
    std::{ffi::OsStr, process::Command},
};

pub fn query_mime_xdg(arg: &OsStr) -> Result<String, XdgQueryError> {
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

pub fn query_default(mime: &str) -> Result<String, XdgQueryError> {
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
