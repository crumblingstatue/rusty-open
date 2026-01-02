#![allow(clippy::collapsible_if)]

use {
    detect_desktop_environment::DesktopEnvironment,
    egui_sf2g::{
        SfEgui,
        egui::{self, FontId},
        sf2g::{
            graphics::{FloatRect, RenderTarget, RenderWindow, View},
            system::Vector2,
            window::{Event, Style, VideoMode},
        },
    },
    std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        process::Command,
        str::Utf8Error,
    },
    thiserror::Error,
    url::Url,
    xdg_desktop_file::{args_from_exec_string, parse_desktop_file},
};

mod generic_xdg;
mod qt_xdg;
mod xdg_desktop_file;

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

fn open(arg: &OsStr, de: Option<DesktopEnvironment>) -> Status {
    let mut url_mime = None;
    let mut url_path = String::new();
    if let Some(text) = arg.to_str()
        && let Ok(url) = Url::parse(text)
    {
        let scheme = url.scheme();
        url_path = url.path().to_string();
        url_mime = Some(format!("x-scheme-handler/{scheme}"));
    }
    let mut mime = if let Some(url_mime) = url_mime {
        url_mime
    } else {
        match de.query_mime(arg) {
            Ok(mime) => mime,
            Err(err) => {
                return Status::XdgQueryError {
                    arg: arg.to_owned(),
                    err,
                };
            }
        }
    };
    // Special handling for `file://` URLs
    if mime == "x-scheme-handler/file" {
        match de.query_mime(url_path.as_ref()) {
            Ok(path_mime) => {
                mime = path_mime;
            }
            Err(err) => {
                return Status::XdgQueryError {
                    arg: arg.to_owned(),
                    err,
                };
            }
        }
    }
    let default = match de.query_default(&mime) {
        Ok(def) => Some(def),
        Err(XdgQueryError::Empty) => {
            return Status::CouldntDetermineDefault {
                arg: arg.to_owned(),
                mime,
            };
        }
        Err(err) => {
            return Status::XdgQueryError {
                arg: arg.to_owned(),
                err,
            };
        }
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
                            Err(e) => {
                                return Status::DesktopFileParseError(e);
                            }
                        }
                    }
                };
                if let Some(exec) = desktop_map.get("Exec") {
                    if let Some(tup) = args_from_exec_string(exec, arg) {
                        (parsed_exec, parsed_args) = tup;
                        args = &parsed_args[..];
                        to_exec = &parsed_exec;
                    } else {
                        return Status::InvalidExecString(exec.clone());
                    }
                }
            }
            Status::PromptExec {
                arg: arg.into(),
                de,
                mime,
                appfile_path,
                to_exec: to_exec.clone(),
                args: args.to_vec(),
            }
        }
        None => Status::CouldntDetermineDefault {
            arg: arg.to_owned(),
            mime,
        },
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

enum Status {
    NoArgs,
    XdgQueryError {
        arg: OsString,
        err: XdgQueryError,
    },
    DesktopFileParseError(std::io::Error),
    InvalidExecString(String),
    CouldntDetermineDefault {
        arg: OsString,
        mime: String,
    },
    PromptExec {
        arg: OsString,
        de: Option<DesktopEnvironment>,
        mime: String,
        appfile_path: PathBuf,
        to_exec: String,
        args: Vec<OsString>,
    },
    ExecError(std::io::Error),
}

fn main() {
    let de = DesktopEnvironment::detect();
    let default_w = 320;
    let default_h = 80;
    let mut current_w = default_w;
    let mut current_h = default_h;
    let mut rw = RenderWindow::new(
        [default_w, default_h],
        "rusty-open",
        Style::DEFAULT,
        &Default::default(),
    )
    .unwrap();
    center_window(&mut rw);
    rw.set_vertical_sync_enabled(true);
    let mut sf_egui = SfEgui::new(&rw);
    set_up_style(&sf_egui);
    let mut status = Status::NoArgs;
    if let Some(arg) = std::env::args_os().nth(1) {
        status = open(&arg, de);
    }
    let mut fallback_exec_string = String::new();
    while rw.is_open() {
        while let Some(ev) = rw.poll_event() {
            sf_egui.add_event(&ev);
            match ev {
                Event::Closed => rw.close(),
                Event::Resized { width, height } => {
                    current_w = width;
                    current_h = height;
                    rw.set_view(
                        &View::from_rect(FloatRect::new(0., 0., width as f32, height as f32))
                            .unwrap(),
                    );
                }
                _ => {}
            }
        }
        let di = sf_egui
            .run(&mut rw, |rw, ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    match &status {
                        Status::NoArgs => {
                            ui.label(format!("Rusty-open running on {}.", de_opt_str(de)));
                            ui.label("No arguments provided. Nothing to do.");
                            ui.vertical_centered(|ui| {
                                if ui.button("Okay then").clicked() {
                                    rw.close();
                                }
                            });
                        }
                        Status::XdgQueryError { arg, err } => {
                            ui.vertical_centered(|ui| {
                                ui.heading("XDG Query error");

                                egui::Grid::new("info_grid").show(ui, |ui| {
                                    ui.label("xdg-open arg");
                                    ui.code(arg.display().to_string());
                                    ui.end_row();
                                    ui.label("Error");
                                    ui.code(err.to_string());
                                });
                                ui.vertical_centered(|ui| {
                                    if ui.button("Ok").clicked() {
                                        rw.close();
                                    }
                                });
                            });
                        }
                        Status::DesktopFileParseError(error) => {
                            ui.heading("Desktop file parse error");
                            ui.code(error.to_string());
                        }
                        Status::InvalidExecString(s) => {
                            ui.heading("Invalid exec string");
                            ui.code(s);
                        }
                        Status::CouldntDetermineDefault { arg, mime } => {
                            ui.heading("Couldn't determine default application");
                            let mut err = None;
                            egui::Grid::new("info_grid").show(ui, |ui| {
                                ui.label("Mime");
                                ui.code(mime);
                                ui.end_row();
                                ui.label("Arg string");
                                ui.code(arg.display().to_string());
                                ui.end_row();
                                ui.label("Path to executable");
                                ui.text_edit_singleline(&mut fallback_exec_string);
                            });
                            ui.vertical_centered(|ui| {
                                let [k_enter, k_esc] = ui.input(|inp| {
                                    [
                                        inp.key_pressed(egui::Key::Enter),
                                        inp.key_pressed(egui::Key::Escape),
                                    ]
                                });
                                if ui.button("✔ Run (Enter)").clicked() || k_enter {
                                    match spawn_command(&fallback_exec_string, &[arg.to_owned()]) {
                                        Ok(()) => {
                                            rw.close();
                                            return;
                                        }
                                        Err(e) => {
                                            err = Some(e);
                                        }
                                    }
                                }
                                if ui.button("🗙 Cancel (Escape)").clicked() || k_esc {
                                    rw.close();
                                }
                            });
                            if let Some(e) = err {
                                status = Status::ExecError(e);
                            }
                        }
                        Status::ExecError(err) => {
                            ui.heading("Exec error");
                            ui.code(err.to_string());
                        }
                        Status::PromptExec {
                            arg,
                            de,
                            mime,
                            appfile_path,
                            to_exec,
                            args,
                        } => {
                            let mut err = None;
                            egui::Grid::new("info_grid").show(ui, |ui| {
                                ui.label("xdg-open arg");
                                ui.code(arg.display().to_string());
                                ui.end_row();
                                ui.label("Detected DE");
                                ui.label(de_opt_str(*de));
                                ui.end_row();
                                ui.label("File mime type");
                                ui.code(mime);
                                ui.end_row();
                                ui.label(".desktop file");
                                ui.code(appfile_path.display().to_string());
                                ui.end_row();
                                ui.label("Executable");
                                ui.code(to_exec);
                                ui.end_row();
                                ui.label("arguments");
                                ui.end_row();
                            });
                            ui.indent("args_indent", |ui| {
                                for arg in args {
                                    ui.code(arg.display().to_string());
                                    ui.end_row();
                                }
                            });
                            ui.separator();
                            ui.vertical_centered(|ui| {
                                let [k_enter, k_esc] = ui.input(|inp| {
                                    [
                                        inp.key_pressed(egui::Key::Enter),
                                        inp.key_pressed(egui::Key::Escape),
                                    ]
                                });
                                if ui.button("✔ Run (Enter)").clicked() || k_enter {
                                    match spawn_command(to_exec, args) {
                                        Ok(()) => {
                                            rw.close();
                                            return;
                                        }
                                        Err(e) => {
                                            err = Some(e);
                                        }
                                    }
                                }
                                if ui.button("🗙 Cancel (Escape)").clicked() || k_esc {
                                    rw.close();
                                }
                            });
                            if let Some(e) = err {
                                status = Status::ExecError(e);
                            }
                        }
                    };
                    let ui_rect = ui.max_rect();
                    let content_w = ui_rect.width() as u32;
                    let content_h = ui_rect.height() as u32;
                    if content_w > current_w || content_h > current_h {
                        // Some horizontal padding seems to be needed
                        let new_w = content_w + 24;
                        let new_h = content_h;
                        // TODO: Better limit than magic number
                        if new_w < 1280 {
                            rw.recreate(
                                [new_w, new_h],
                                "rusty-open",
                                Style::DEFAULT,
                                &Default::default(),
                            );
                            rw.set_vertical_sync_enabled(true);
                            center_window(rw);
                        }
                    }
                });
            })
            .unwrap();
        sf_egui.draw(di, &mut rw, None);
        rw.display();
    }
}

fn set_up_style(sf_egui: &SfEgui) {
    sf_egui.context().style_mut(|style| {
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            FontId::new(16.0, egui::FontFamily::Monospace),
        );
    });
}

fn spawn_command(cmd: &str, args: &[OsString]) -> std::io::Result<()> {
    Command::new(cmd).args(args).spawn().map(|_| ())
}

fn center_window(rw: &mut RenderWindow) {
    let Vector2 { x, y } = rw.size();
    let desktop_mode = VideoMode::desktop_mode();
    let width_diff = desktop_mode.width - x;
    let height_diff = desktop_mode.height - y;
    let new_x = width_diff / 2;
    let new_y = height_diff / 2;
    rw.set_position(Vector2::new(new_x as i32, new_y as i32));
}
