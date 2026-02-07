use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    form::{field, v_form},
    input::{Input, InputState, NumberInput},
    Disableable, Root,
};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing_subscriber::fmt::writer::MakeWriter;

use kv_downloader::{commands::Download, commands::DownloadArgs, keystore::Keystore};

const LOG_BUFFER_LIMIT: usize = 16_000;
const LOG_POLL_INTERVAL_MS: u64 = 200;

struct LogMakeWriter {
    buffer: Arc<Mutex<String>>,
}

struct LogWriter {
    buffer: Arc<Mutex<String>>,
}

impl<'a> MakeWriter<'a> for LogMakeWriter {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.buffer.clone(),
        }
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let text = String::from_utf8_lossy(buf);
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push_str(&text);
            if buffer.len() > LOG_BUFFER_LIMIT {
                let excess = buffer.len() - LOG_BUFFER_LIMIT;
                buffer.drain(..excess);
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct GuiApp {
    username: Entity<InputState>,
    password: Entity<InputState>,
    song_url: Entity<InputState>,
    download_path: Entity<InputState>,
    transpose: Entity<InputState>,
    headless: bool,
    count_in: bool,
    force_restart: bool,
    busy: bool,
    has_credentials: bool,
    log_buffer: Arc<Mutex<String>>,
    log_text: SharedString,
    status: SharedString,
}

impl GuiApp {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        log_buffer: Arc<Mutex<String>>,
    ) -> Self {
        let username = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Karaoke Version username")
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
                .masked(true)
        });
        let song_url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Song URL (e.g. https://www.karaoke-version.com/...)")
        });
        let download_path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Download folder (optional)")
        });
        let transpose = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("-4 to 4")
                .default_value("0")
        });

        let has_credentials = Keystore::get_credentials().is_ok();

        Self {
            username,
            password,
            song_url,
            download_path,
            transpose,
            headless: false,
            count_in: false,
            force_restart: false,
            busy: false,
            has_credentials,
            log_buffer,
            log_text: SharedString::from(""),
            status: SharedString::from("Ready"),
        }
    }

    fn start_log_poller(&mut self, cx: &mut Context<Self>) {
        let buffer = self.log_buffer.clone();
        cx.spawn(|this: WeakEntity<GuiApp>, cx: &mut AsyncApp| {
            let mut app = cx.clone();
            async move {
                let mut last_len = 0usize;
                loop {
                    Timer::after(Duration::from_millis(LOG_POLL_INTERVAL_MS)).await;
                    let snapshot = buffer
                        .lock()
                        .ok()
                        .map(|buf| buf.clone())
                        .unwrap_or_default();

                    if snapshot.len() != last_len {
                        last_len = snapshot.len();
                        let text = SharedString::from(snapshot);
                        let _ = this.update(&mut app, |view, cx| {
                            view.log_text = text;
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn set_status(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = message.into();
        cx.notify();
    }

    fn save_credentials(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }

        let user = self.username.read(cx).value().trim().to_string();
        let pass = self.password.read(cx).value().trim().to_string();

        if user.is_empty() || pass.is_empty() {
            self.set_status("Username and password are required.", cx);
            return;
        }

        match Keystore::login(&user, &pass) {
            Ok(_) => {
                self.has_credentials = true;
                self.set_status("Credentials saved.", cx);
            }
            Err(err) => self.set_status(format!("Failed to save credentials: {err}"), cx),
        }
    }

    fn logout(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }

        match Keystore::logout() {
            Ok(_) => {
                self.has_credentials = false;
                self.set_status("Logged out. Credentials cleared.", cx);
            }
            Err(err) => self.set_status(format!("Failed to logout: {err}"), cx),
        }
    }

    fn start_download(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }

        let song_url = self.song_url.read(cx).value().trim().to_string();
        if song_url.is_empty() {
            self.set_status("Song URL is required.", cx);
            return;
        }

        let transpose_raw = self.transpose.read(cx).value().trim().to_string();
        let transpose = match parse_transpose(&transpose_raw) {
            Ok(value) => value,
            Err(message) => {
                self.set_status(message, cx);
                return;
            }
        };

        let download_path = self.download_path.read(cx).value().trim().to_string();
        let download_path = if download_path.is_empty() {
            None
        } else {
            Some(download_path)
        };

        let args = DownloadArgs {
            song_url,
            headless: self.headless,
            download_path,
            transpose: Some(transpose),
            count_in: self.count_in,
            force_restart: self.force_restart,
            browser_idle_timeout_secs: 300,
        };

        self.busy = true;
        self.set_status("Starting download...", cx);

        cx.spawn(|this: WeakEntity<GuiApp>, cx: &mut AsyncApp| {
            let mut app = cx.clone();
            async move {
                let result = app
                    .background_executor()
                    .spawn(async move { Download::run(args) })
                    .await;

                let status = match result {
                    Ok(_) => SharedString::from("Download complete."),
                    Err(err) => SharedString::from(format!("Download failed: {err}")),
                };

                let _ = this.update(&mut app, |view, cx| {
                    view.busy = false;
                    view.status = status;
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

impl Render for GuiApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.busy;
        let credentials_section = if self.has_credentials {
            div()
                .child(
                    Button::new("logout")
                        .label("Log Out")
                        .disabled(disabled)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.logout(cx);
                        })),
                )
                .into_any_element()
        } else {
            v_form()
                .child(
                    field()
                        .label("Username")
                        .child(Input::new(&self.username).disabled(disabled)),
                )
                .child(
                    field()
                        .label("Password")
                        .child(
                            Input::new(&self.password)
                                .disabled(disabled)
                                .mask_toggle(),
                        ),
                )
                .child(
                    field()
                        .label_indent(false)
                        .child(
                            Button::new("save-credentials")
                                .label("Save Credentials")
                                .primary()
                                .disabled(disabled)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.save_credentials(cx);
                                })),
                        ),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .child(
                div().child("Credentials").child(credentials_section),
            )
            .child(
                div()
                    .child("Download")
                    .child(
                        v_form()
                            .child(
                                field()
                                    .label("Song URL")
                                    .child(Input::new(&self.song_url).disabled(disabled)),
                            )
                            .child(
                                field()
                                    .label("Download Path")
                                    .child(Input::new(&self.download_path).disabled(disabled)),
                            )
                            .child(
                                field()
                                    .label("Transpose")
                                    .child(NumberInput::new(&self.transpose).disabled(disabled)),
                            )
                            .child(
                                field()
                                    .label("Options")
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                Checkbox::new("headless")
                                                    .label("Headless browser")
                                                    .checked(self.headless)
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, checked, _, cx| {
                                                        view.headless = *checked;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Checkbox::new("count-in")
                                                    .label("Include count-in")
                                                    .checked(self.count_in)
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, checked, _, cx| {
                                                        view.count_in = *checked;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Checkbox::new("force-restart")
                                                    .label("Force restart")
                                                    .checked(self.force_restart)
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, checked, _, cx| {
                                                        view.force_restart = *checked;
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                field()
                                    .label_indent(false)
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                Button::new("download")
                                                    .label("Download")
                                                    .primary()
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.start_download(cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new("clear-status")
                                                    .label("Clear Status")
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.set_status("Ready", cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .child("Status")
                    .child(div().child(self.status.clone())),
            )
            .child(
                div()
                    .child("Trace")
                    .child(
                        div()
                            .bg(hsla(0.0, 0.0, 0.08, 1.0))
                            .text_color(hsla(0.0, 0.0, 0.85, 1.0))
                            .font_family(".ZedMono")
                            .text_xs()
                            .p_2()
                            .h(px(140.0))
                            .overflow_hidden()
                            .child(self.log_text.clone()),
                    ),
            )
    }
}

fn parse_transpose(raw: &str) -> Result<i8, SharedString> {
    if raw.trim().is_empty() {
        return Ok(0);
    }

    let parsed: i8 = raw
        .trim()
        .parse()
        .map_err(|_| SharedString::from("Transpose must be an integer between -4 and 4."))?;

    if !(-4..=4).contains(&parsed) {
        return Err(SharedString::from(
            "Transpose must be between -4 and 4.",
        ));
    }

    Ok(parsed)
}

fn main() {
    dotenv::dotenv().ok();
    let log_buffer = Arc::new(Mutex::new(String::new()));
    init_tracing(log_buffer.clone());

    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            let _ = cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| GuiApp::new(window, cx, log_buffer.clone()));
                view.update(cx, |view, cx| view.start_log_poller(cx));
                cx.new(|cx| Root::new(view, window, cx))
            });
        });
}

fn init_tracing(log_buffer: Arc<Mutex<String>>) {
    let make_writer = LogMakeWriter { buffer: log_buffer };
    let _ = tracing_subscriber::fmt()
        .with_writer(make_writer)
        .with_ansi(false)
        .with_target(false)
        .with_level(true)
        .try_init();
}
