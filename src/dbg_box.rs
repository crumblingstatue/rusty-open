pub fn dbg_box_show(msg: &str) {
    rfd::MessageDialog::new()
        .set_title("dbg!")
        .set_description(msg)
        .show();
}

/// Used for debugging in cases where we're not connected to a terminal
macro_rules! dbg_box {
    () => {
        dbg_box_show(&format!("[{}:{}]", file!(), line!()))
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                $crate::dbg_box::dbg_box_show(&format!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp));
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}
