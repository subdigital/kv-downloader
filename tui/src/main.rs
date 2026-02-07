use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, terminal};
use kv_downloader::{driver, keystore, tasks};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use scraper::{Html, Selector};
use std::io::{self, Stdout, Write};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing_subscriber::fmt::MakeWriter;
use url::Url;

#[derive(Clone, Debug)]
struct Song {
    title: String,
    artist: String,
    url: String,
}

#[derive(Clone, Debug)]
struct Track {
    name: String,
    selected: bool,
}

#[derive(Clone, Debug)]
struct SongDetails {
    tracks: Vec<String>,
    base_key: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Songs,
    SongDetail,
    DownloadStatus,
    DownloadDone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailFocus {
    Tracks,
    Download,
}

struct App {
    screen: Screen,
    menu_items: Vec<String>,
    menu_index: usize,
    songs: Vec<Song>,
    songs_state: ListState,
    tracks: Vec<Track>,
    tracks_state: ListState,
    detail_focus: DetailFocus,
    intro_click: bool,
    key_shift: i8,
    base_key: Option<String>,
    status: String,
    current_song: Option<Song>,
    download: Option<DownloadState>,
    logs: Arc<Mutex<LogBuffer>>,
    log_scroll: u16,
    loading: Option<LoadingState>,
    spinner_index: usize,
    confirm: Option<ConfirmState>,
    done_menu_items: Vec<String>,
    done_menu_index: usize,
}

impl App {
    fn new(logs: Arc<Mutex<LogBuffer>>) -> Self {
        let mut app = Self {
            screen: Screen::Menu,
            menu_items: Vec::new(),
            menu_index: 0,
            songs: Vec::new(),
            songs_state: ListState::default(),
            tracks: Vec::new(),
            tracks_state: ListState::default(),
            detail_focus: DetailFocus::Tracks,
            intro_click: false,
            key_shift: 0,
            base_key: None,
            status: String::new(),
            current_song: None,
            download: None,
            logs,
            log_scroll: 0,
            loading: None,
            spinner_index: 0,
            confirm: None,
            done_menu_items: vec![
                "Open in File Explorer".to_string(),
                "Back to Songs".to_string(),
                "Quit".to_string(),
            ],
            done_menu_index: 0,
        };
        app.refresh_menu();
        app
    }

    fn refresh_menu(&mut self) {
        let authenticated = is_authenticated();
        self.menu_items = if authenticated {
            vec![
                "Browse My Songs".to_string(),
                "Logout".to_string(),
                "Quit".to_string(),
            ]
        } else {
            vec![
                "Browse My Songs".to_string(),
                "Login".to_string(),
                "Quit".to_string(),
            ]
        };
        self.menu_index = 0;
    }
}

fn main() -> Result<()> {
    run_tui()
}

fn run_tui() -> Result<()> {
    let logs = Arc::new(Mutex::new(LogBuffer::new(1000)));
    setup_tracing(logs.clone());

    let mut terminal = setup_terminal()?;
    let tick_rate = Duration::from_millis(80);
    let mut last_tick = Instant::now();
    let mut app = App::new(logs);

    loop {
        update_download_state(&mut app);
        update_loading_state(&mut app);
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key_event(&mut terminal, &mut app, key)? {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            if app.loading.is_some() {
                app.spinner_index = app.spinner_index.wrapping_add(1);
            }
            last_tick = Instant::now();
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, terminal::DisableLineWrap)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        terminal::EnableLineWrap
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_key_event(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Ok(true);
    }

    match app.screen {
        Screen::Menu => handle_menu_keys(terminal, app, key),
        Screen::Songs => handle_songs_keys(terminal, app, key),
        Screen::SongDetail => handle_detail_keys(terminal, app, key),
        Screen::DownloadStatus => handle_download_keys(app, key),
        Screen::DownloadDone => handle_done_keys(app, key),
    }
}

fn handle_menu_keys(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    match key.code {
        KeyCode::Esc => return Ok(true),
        KeyCode::Up => {
            if app.menu_index > 0 {
                app.menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.menu_index + 1 < app.menu_items.len() {
                app.menu_index += 1;
            }
        }
        KeyCode::Enter => {
            let selected = app
                .menu_items
                .get(app.menu_index)
                .cloned()
                .unwrap_or_default();
            match selected.as_str() {
                "Login" => {
                    app.status = "".to_string();
                    restore_terminal(terminal)?;
                    let login_result = login_flow();
                    *terminal = setup_terminal()?;
                    match login_result {
                        Ok(_) => {
                            app.status = "Login complete".to_string();
                            app.refresh_menu();
                        }
                        Err(err) => {
                            app.status = format!("Login failed: {}", err);
                        }
                    }
                }
                "Logout" => {
                    keystore::Keystore::logout()?;
                    keystore::Keystore::clear_auth_cookie()?;
                    app.status = "Logged out".to_string();
                    app.refresh_menu();
                }
                "Browse My Songs" => {
                    app.status = "".to_string();
                    match cookie_value() {
                        Ok(cookie) => start_loading_songs(app, cookie),
                        Err(err) => app.status = format!("Not authenticated: {}", err),
                    }
                }
                "Quit" => return Ok(true),
                _ => {}
            }
        }
        KeyCode::Char('q') => return Ok(true),
        _ => {}
    }

    Ok(false)
}

fn handle_songs_keys(
    _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Menu;
            app.status = "".to_string();
        }
        KeyCode::Up => {
            let next = app
                .songs_state
                .selected()
                .and_then(|idx| idx.checked_sub(1))
                .unwrap_or(0);
            if !app.songs.is_empty() {
                app.songs_state.select(Some(next));
            }
        }
        KeyCode::Down => {
            let selected = app.songs_state.selected().unwrap_or(0);
            let next = if selected + 1 >= app.songs.len() {
                selected
            } else {
                selected + 1
            };
            if !app.songs.is_empty() {
                app.songs_state.select(Some(next));
            }
        }
        KeyCode::Enter => {
            if let Some(idx) = app.songs_state.selected() {
                if let Some(song) = app.songs.get(idx).cloned() {
                    app.status = "".to_string();
                    match cookie_value() {
                        Ok(cookie) => start_loading_tracks(app, cookie, song),
                        Err(err) => app.status = format!("Not authenticated: {}", err),
                    }
                }
            }
        }
        _ => {}
    }

    Ok(false)
}

fn handle_detail_keys(
    _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    if app.confirm.is_some() {
        return handle_confirm_keys(app, key);
    }

    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Menu;
            app.status = "".to_string();
        }
        KeyCode::Tab => {
            app.detail_focus = match app.detail_focus {
                DetailFocus::Tracks => DetailFocus::Download,
                DetailFocus::Download => DetailFocus::Tracks,
            };
        }
        KeyCode::Up => {
            if app.detail_focus == DetailFocus::Tracks {
                let next = app
                    .tracks_state
                    .selected()
                    .and_then(|idx| idx.checked_sub(1))
                    .unwrap_or(0);
                if !app.tracks.is_empty() {
                    app.tracks_state.select(Some(next));
                }
            }
        }
        KeyCode::Down => {
            if app.detail_focus == DetailFocus::Tracks {
                let selected = app.tracks_state.selected().unwrap_or(0);
                let next = if selected + 1 >= app.tracks.len() {
                    selected
                } else {
                    selected + 1
                };
                if !app.tracks.is_empty() {
                    app.tracks_state.select(Some(next));
                }
            }
        }
        KeyCode::Char(' ') => {
            if app.detail_focus == DetailFocus::Tracks {
                if let Some(idx) = app.tracks_state.selected() {
                    if let Some(track) = app.tracks.get_mut(idx) {
                        track.selected = !track.selected;
                    }
                }
            }
        }
        KeyCode::Char('i') => {
            app.intro_click = !app.intro_click;
        }
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Right => {
            if app.key_shift < 4 {
                app.key_shift += 1;
            }
        }
        KeyCode::Char('-') | KeyCode::Left => {
            if app.key_shift > -4 {
                app.key_shift -= 1;
            }
        }
        KeyCode::Enter => {
            if app.detail_focus == DetailFocus::Download {
                if let Some(song) = app.current_song.clone() {
                    app.status = "Starting download...".to_string();
                    let selected_tracks: Vec<String> = app
                        .tracks
                        .iter()
                        .filter(|t| t.selected)
                        .map(|t| t.name.clone())
                        .collect();
                    if selected_tracks.is_empty() {
                        app.status = "Select at least one track to download".to_string();
                        return Ok(false);
                    }
                    app.confirm = Some(ConfirmState {
                        message: vec![
                            "This will start a Chromium browser then minimize it.".to_string(),
                            "For more reliable downloads, make sure you don't interact with this window at all."
                                .to_string(),
                            "Continue?".to_string(),
                        ],
                        selected: 0,
                        payload: ConfirmPayload::StartDownload {
                            song,
                            selected_tracks,
                            intro_click: app.intro_click,
                            key_shift: app.key_shift,
                        },
                    });
                }
            }
        }
        _ => {}
    }

    Ok(false)
}

fn ui(frame: &mut ratatui::Frame, app: &mut App) {
    match app.screen {
        Screen::Menu => render_menu(frame, app),
        Screen::Songs => render_songs(frame, app),
        Screen::SongDetail => render_detail(frame, app),
        Screen::DownloadStatus => render_download_status(frame, app),
        Screen::DownloadDone => render_download_done(frame, app),
    }
    if app.loading.is_some() {
        render_loading_overlay(frame, app);
    }
    if app.confirm.is_some() {
        render_confirm_overlay(frame, app);
    }
}

fn render_menu(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.size());

    let logo = vec![
        Line::from(Span::styled(
            r"   __ ___   __  ___                  __             __       ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"  / //_/ | / / / _ \___ _    _____  / /__  ___ ____/ /__ ____",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r" / ,<  | |/ / / // / _ \ |/|/ / _ \/ / _ \/ _ `/ _  / -_) __/",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"/_/|_| |___/ /____/\___/__,__/_//_/_/\___/\_,_/\_,_/\__/_/   ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ];
    let logo = Paragraph::new(Text::from(logo))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("KV Downloader")
                .border_style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(logo, chunks[0]);

    let items: Vec<ListItem> = app
        .menu_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let prefix = if i == app.menu_index { ">" } else { " " };
            let style = if i == app.menu_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{} {}", prefix, item),
                style,
            )))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Main Menu")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(list, chunks[1]);

    let status = Paragraph::new(app.status.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(status, chunks[2]);

    let help = Paragraph::new("Enter: select | ESC: back | Ctrl+C: quit")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_songs(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(frame.size());

    let items: Vec<ListItem> = app
        .songs
        .iter()
        .map(|song| ListItem::new(format!("{} - {}", song.title, song.artist)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("My Songs")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[0], &mut app.songs_state);

    let help = Paragraph::new("Enter: open song | ESC: menu")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Left);
    frame.render_widget(help, chunks[1]);
}

fn render_detail(frame: &mut ratatui::Frame, app: &mut App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(frame.size());

    let title = app
        .current_song
        .as_ref()
        .map(|s| format!("{} - {}", s.title, s.artist))
        .unwrap_or_else(|| "Song".to_string());

    let key_display = format_key_display(app.base_key.as_deref(), app.key_shift);
    let controls = vec![
        Line::from(vec![
            Span::styled("Intro Click: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(if app.intro_click { "On" } else { "Off" }),
            Span::raw("   "),
            Span::styled("Key: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(key_display),
        ]),
        Line::from("Toggle intro click with 'i'. Adjust key with +/- or arrows."),
    ];

    let header = Paragraph::new(Text::from(controls))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(header, layout[0]);

    let items: Vec<ListItem> = app
        .tracks
        .iter()
        .map(|track| {
            let bracket_style = Style::default().fg(Color::DarkGray);
            let mark_style = if track.selected {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let text_style = if track.selected {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            let line = Line::from(vec![
                Span::styled("[", bracket_style),
                Span::styled(if track.selected { "x" } else { " " }, mark_style),
                Span::styled("] ", bracket_style),
                Span::styled(track.name.clone(), text_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let track_block_title = if app.detail_focus == DetailFocus::Tracks {
        "Tracks (space to toggle)"
    } else {
        "Tracks"
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(track_block_title)
                .borders(Borders::ALL)
                .border_style(if app.detail_focus == DetailFocus::Tracks {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, layout[1], &mut app.tracks_state);

    render_download_block(
        frame,
        layout[2],
        app.detail_focus == DetailFocus::Download,
        &app.status,
    );

    let help = Paragraph::new("Tab: switch pane | Space: toggle | i: intro | +/-: key | ESC: menu")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, layout[3]);
}

fn render_download_status(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(9)].as_ref())
        .split(frame.size());

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(chunks[0]);

    let mut title = "Download Status".to_string();
    if let Some(download) = &app.download {
        title = format!("Download Status - {} - {}", download.song.title, download.song.artist);
    }

    let track_items = if let Some(download) = &app.download {
        download
            .tracks
            .iter()
            .map(|track| {
                let marker = match track.status {
                    TrackStatus::Pending => "...",
                    TrackStatus::Downloading => ">>",
                    TrackStatus::Done => "OK",
                    TrackStatus::Failed => "!!",
                };
                let style = match track.status {
                    TrackStatus::Downloading => Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    TrackStatus::Done => Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    TrackStatus::Failed => Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                    TrackStatus::Pending => Style::default().fg(Color::Gray),
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{} {}", marker, track.name),
                    style,
                )))
            })
            .collect::<Vec<_>>()
    } else {
        vec![ListItem::new("No download in progress.")]
    };

    let track_list = List::new(track_items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(track_list, columns[0]);

    let logs = app
        .logs
        .lock()
        .ok()
        .map(|buf| buf.lines.clone())
        .unwrap_or_default();
    let visible_lines = columns[1].height.saturating_sub(2) as usize;
    let max_scroll = logs
        .len()
        .saturating_sub(visible_lines)
        .min(u16::MAX as usize) as u16;
    if app.log_scroll > max_scroll {
        app.log_scroll = max_scroll;
    }
    let log_text = logs.iter().map(|line| parse_ansi_line(line)).collect::<Vec<_>>();
    let log_block = Paragraph::new(Text::from(log_text))
        .block(
            Block::default()
                .title("Logs")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.log_scroll, 0));
    frame.render_widget(log_block, columns[1]);

    let footer = Paragraph::new("Up/Down: scroll logs | ESC: menu (after finish)")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

fn render_download_done(frame: &mut ratatui::Frame, app: &mut App) {
    let Some(download) = &app.download else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(7)].as_ref())
        .split(frame.size());

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Download complete",
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Song: {} - {}",
        download.song.title, download.song.artist
    )));
    if let Some(duration) = download.duration {
        lines.push(Line::from(format!(
            "Duration: {}",
            format_duration(duration)
        )));
    }
    lines.push(Line::from(format!(
        "Location: {}",
        download.download_dir.display()
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Tracks:"));
    for track in &download.tracks {
        lines.push(Line::from(format!("  - {}", track.name)));
    }

    let details = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title("Summary")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(details, chunks[0]);

    let items: Vec<ListItem> = app
        .done_menu_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let prefix = if i == app.done_menu_index { ">" } else { " " };
            let style = if i == app.done_menu_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{} {}", prefix, item),
                style,
            )))
        })
        .collect();

    let menu = List::new(items)
        .block(
            Block::default()
                .title("Next")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(menu, chunks[1]);
}

fn render_download_block(
    frame: &mut ratatui::Frame,
    area: Rect,
    focused: bool,
    status: &str,
) {
    let mut title = "Download".to_string();
    if focused {
        title.push_str(" (Enter)");
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    let mut lines = vec![Line::from(Span::styled(
        "[ Download ]",
        Style::default().add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }),
    ))];
    if !status.trim().is_empty() {
        lines.push(Line::from(Span::raw(status)));
    }
    let content = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .block(block);

    frame.render_widget(content, area);
}

fn login_flow() -> Result<()> {
    let user = kv_downloader::prompt::prompt("Username: ", false)?;
    let pass = kv_downloader::prompt::prompt("Password: ", true)?;

    keystore::Keystore::login(&user, &pass)?;

    let config = driver::Config {
        domain: "www.karaoke-version.com".to_string(),
        headless: false,
        download_path: None,
        idle_browser_timeout: Duration::from_secs(300),
    };
    let driver = driver::Driver::new(config);
    driver.sign_in(&user, &pass)?;

    Ok(())
}

fn start_download(
    song_url: &str,
    count_in: bool,
    transpose: i8,
    selected_tracks: Vec<String>,
) -> Result<()> {
    let credentials = keystore::Keystore::get_credentials()
        .map_err(|_| anyhow!("Must login first"))?;

    let config = driver::Config {
        domain: extract_domain_from_url(song_url)
            .ok_or_else(|| anyhow!("Missing domain from url"))?,
        headless: false,
        download_path: None,
        idle_browser_timeout: Duration::from_secs(300),
    };
    let driver = driver::Driver::new(config);

    if driver.progress.is_same_url(song_url)? {
        let completed = driver.progress.get_completed_tracks()?;
        if !completed.is_empty() {
            tracing::info!(
                "Resuming previous download. Already completed {} tracks:",
                completed.len()
            );
            for track in &completed {
                tracing::info!("  ✓ {}", track);
            }
        }
    } else if !driver.progress.get_completed_tracks()?.is_empty() {
        tracing::info!("Different song detected, clearing previous progress");
        driver.progress.clear()?;
    }

    driver.sign_in(&credentials.user, &credentials.password)?;

    let download_options = tasks::download_song::DownloadOptions {
        count_in,
        transpose,
        selected_tracks: if selected_tracks.is_empty() {
            None
        } else {
            Some(selected_tracks)
        },
    };
    driver.download_song(song_url, download_options)?;

    Ok(())
}

fn extract_domain_from_url(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|h| h.to_string()))
}

fn is_authenticated() -> bool {
    match cookie_value() {
        Ok(cookie) => cookie.contains("|u-i:"),
        Err(_) => false,
    }
}

fn cookie_value() -> Result<String> {
    if let Ok(value) = std::env::var("KV_SESSION_COOKIE") {
        if value.trim().is_empty() {
            return Err(anyhow!("KV_SESSION_COOKIE is empty"));
        }
        return Ok(value);
    }

    keystore::Keystore::get_auth_cookie_value()
}

fn fetch_songs(cookie: &str) -> Result<Vec<Song>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("kv-downloader-tui")
        .build()?;

    let res = client
        .get("https://www.karaoke-version.com/my/download.html")
        .header("Cookie", format!("karaoke-version={}", cookie))
        .send()?;

    let body = res.text()?;
    let doc = Html::parse_document(&body);

    let row_selector = Selector::parse("table.my-downloaded-files tr.vam").unwrap();
    let song_selector = Selector::parse("td.my-downloaded-files__song a").unwrap();
    let artist_selector = Selector::parse("td.my-downloaded-files__artist a").unwrap();

    let mut songs = Vec::new();
    for row in doc.select(&row_selector) {
        let song_el = row.select(&song_selector).next();
        let artist_el = row.select(&artist_selector).next();

        if let (Some(song_el), Some(artist_el)) = (song_el, artist_el) {
            let title = normalize_text(song_el.text());
            let artist = normalize_text(artist_el.text());
            let href = song_el.value().attr("href").unwrap_or("");
            if title.is_empty() || href.is_empty() {
                continue;
            }
            let url = Url::parse("https://www.karaoke-version.com")
                .and_then(|base| base.join(href))
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string());

            songs.push(Song { title, artist, url });
        }
    }

    if songs.is_empty() {
        return Err(anyhow!("No songs found. Are you logged in?"));
    }

    Ok(songs)
}

fn fetch_song_details(cookie: &str, song_url: &str) -> Result<SongDetails> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("kv-downloader-tui")
        .build()?;

    let res = client
        .get(song_url)
        .header("Cookie", format!("karaoke-version={}", cookie))
        .send()?;

    let body = res.text()?;
    let doc = Html::parse_document(&body);

    let track_selector = Selector::parse("div.track").unwrap();
    let caption_selector = Selector::parse(".track__caption").unwrap();
    let precount_selector = Selector::parse("#precount").unwrap();
    let audio_info_selector = Selector::parse("#audio-infos p").unwrap();

    let mut tracks = Vec::new();
    for track in doc.select(&track_selector) {
        let caption = match track.select(&caption_selector).next() {
            Some(caption) => caption,
            None => continue,
        };

        let segments: Vec<&str> = caption.text().collect();
        let has_precount = caption.select(&precount_selector).next().is_some();
        let name = if has_precount {
            segments
                .iter()
                .rev()
                .find(|s| !s.trim().is_empty())
                .map(|s| normalize_text(std::iter::once(*s)))
                .unwrap_or_default()
        } else {
            normalize_text(segments.iter().copied())
        };
        if name.is_empty() {
            continue;
        }

        tracks.push(name);
    }

    if tracks.is_empty() {
        return Err(anyhow!("No tracks found on song page"));
    }

    let base_key = extract_base_key(&doc, &audio_info_selector);

    Ok(SongDetails { tracks, base_key })
}

fn normalize_text<'a, I>(segments: I) -> String
where
    I: Iterator<Item = &'a str>,
{
    let joined = segments.collect::<Vec<_>>().join(" ");
    joined
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn update_download_state(app: &mut App) {
    let Some(download) = app.download.as_mut() else {
        return;
    };

    let progress = kv_downloader::download_progress::DownloadProgress::new();
    let completed = progress
        .get_completed_tracks()
        .unwrap_or_default()
        .into_iter()
        .map(|name| normalize_track_name(&name))
        .collect::<std::collections::HashSet<_>>();

    for track in &mut download.tracks {
        let normalized = normalize_track_name(&track.name);
        if completed.contains(&normalized) {
            track.status = TrackStatus::Done;
        }
    }

    let latest = latest_processing_track(app.logs.clone());
    if let Some(current) = latest {
        for track in &mut download.tracks {
            if track.status == TrackStatus::Pending
                && normalize_track_name(&track.name) == normalize_track_name(&current)
            {
                track.status = TrackStatus::Downloading;
            } else if track.status == TrackStatus::Downloading
                && normalize_track_name(&track.name) != normalize_track_name(&current)
            {
                track.status = TrackStatus::Pending;
            }
        }
    }

    if let Some(handle) = download.handle.as_ref() {
        if handle.is_finished() {
            let result = download.handle.take().unwrap().join();
            match result {
                Ok(Ok(())) => {
                    for track in &mut download.tracks {
                        if track.status == TrackStatus::Pending
                            || track.status == TrackStatus::Downloading
                        {
                            track.status = TrackStatus::Done;
                        }
                    }
                    download.done = Some(Ok(()));
                    download.duration = Some(download.started_at.elapsed());
                    app.status = "Download complete".to_string();
                    app.done_menu_index = 0;
                    app.screen = Screen::DownloadDone;
                }
                Ok(Err(err)) => {
                    for track in &mut download.tracks {
                        if track.status == TrackStatus::Pending
                            || track.status == TrackStatus::Downloading
                        {
                            track.status = TrackStatus::Failed;
                        }
                    }
                    download.done = Some(Err(err.to_string()));
                    app.status = "Download failed".to_string();
                }
                Err(_) => {
                    download.done = Some(Err("Download thread panicked".to_string()));
                    app.status = "Download failed".to_string();
                }
            }
        }
    }
}

fn handle_download_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Up => {
            app.log_scroll = app.log_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            app.log_scroll = app.log_scroll.saturating_add(1);
        }
        KeyCode::Esc => {
            if app.download.as_ref().and_then(|d| d.done.as_ref()).is_some() {
                app.screen = Screen::Menu;
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_done_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Up => {
            if app.done_menu_index > 0 {
                app.done_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.done_menu_index + 1 < app.done_menu_items.len() {
                app.done_menu_index += 1;
            }
        }
        KeyCode::Enter => {
            let selected = app
                .done_menu_items
                .get(app.done_menu_index)
                .cloned()
                .unwrap_or_default();
            match selected.as_str() {
                "Open in File Explorer" => {
                    if let Some(download) = &app.download {
                        let _ = open_in_file_explorer(&download.download_dir);
                    }
                }
                "Back to Songs" => {
                    app.screen = Screen::Songs;
                }
                "Quit" => return Ok(true),
                _ => {}
            }
        }
        KeyCode::Esc => {
            app.screen = Screen::Songs;
        }
        _ => {}
    }
    Ok(false)
}

fn latest_processing_track(logs: Arc<Mutex<LogBuffer>>) -> Option<String> {
    let Ok(buf) = logs.lock() else {
        return None;
    };
    for line in buf.lines.iter().rev() {
        if let Some(idx) = line.find("Processing track") {
            if let Some(start) = line[idx..].find('\'') {
                let rest = &line[idx + start + 1..];
                if let Some(end) = rest.find('\'') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }
    None
}

fn normalize_track_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TrackStatus {
    Pending,
    Downloading,
    Done,
    Failed,
}

#[derive(Clone, Debug)]
struct TrackStatusItem {
    name: String,
    status: TrackStatus,
}

struct DownloadState {
    song: Song,
    tracks: Vec<TrackStatusItem>,
    handle: Option<JoinHandle<Result<()>>>,
    done: Option<Result<(), String>>,
    started_at: Instant,
    duration: Option<Duration>,
    download_dir: std::path::PathBuf,
}

struct LogBuffer {
    lines: Vec<String>,
    partial: String,
    max: usize,
}

enum LoadingKind {
    Songs,
    Tracks { song: Song },
}

struct LoadingState {
    message: String,
    kind: LoadingKind,
    receiver: std::sync::mpsc::Receiver<LoadingResult>,
}

enum LoadingResult {
    Songs(Result<Vec<Song>, String>),
    Tracks(Result<SongDetails, String>),
}

struct ConfirmState {
    message: Vec<String>,
    selected: usize,
    payload: ConfirmPayload,
}

enum ConfirmPayload {
    StartDownload {
        song: Song,
        selected_tracks: Vec<String>,
        intro_click: bool,
        key_shift: i8,
    },
}

impl LogBuffer {
    fn new(max: usize) -> Self {
        Self {
            lines: Vec::new(),
            partial: String::new(),
            max,
        }
    }

    fn push_line(&mut self, line: String) {
        if line.trim().is_empty() {
            return;
        }
        self.lines.push(line);
        if self.lines.len() > self.max {
            let excess = self.lines.len() - self.max;
            self.lines.drain(0..excess);
        }
    }
}

struct LogWriter {
    buffer: Arc<Mutex<LogBuffer>>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let mut guard = self
            .buffer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log buffer poisoned"))?;
        guard.partial.push_str(&text);
        while let Some(pos) = guard.partial.find('\n') {
            let line = guard.partial.drain(..=pos).collect::<String>();
            guard.push_line(line.trim_end().to_string());
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct LogWriterFactory {
    buffer: Arc<Mutex<LogBuffer>>,
}

impl<'a> MakeWriter<'a> for LogWriterFactory {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.buffer.clone(),
        }
    }
}

struct LogScope {
    _buffer: Arc<Mutex<LogBuffer>>,
}

impl LogScope {
    fn new(buffer: Arc<Mutex<LogBuffer>>) -> Self {
        if let Ok(mut guard) = buffer.lock() {
            guard.push_line("---- download started ----".to_string());
        }
        Self { _buffer: buffer }
    }
}

fn setup_tracing(buffer: Arc<Mutex<LogBuffer>>) {
    let writer = LogWriterFactory { buffer };
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(true)
        .try_init();
}

fn parse_ansi_line(input: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current = Style::default();
    let mut buf = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            if !buf.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buf), current));
            }
            let mut code = String::new();
            while let Some(c) = chars.next() {
                if c == 'm' {
                    break;
                }
                code.push(c);
            }
            apply_sgr(&code, &mut current);
        } else {
            buf.push(ch);
        }
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, current));
    }

    Line::from(spans)
}

fn apply_sgr(code: &str, style: &mut Style) {
    if code.is_empty() {
        *style = Style::default();
        return;
    }

    for part in code.split(';') {
        if let Ok(num) = part.parse::<u16>() {
            match num {
                0 => *style = Style::default(),
                1 => *style = style.add_modifier(Modifier::BOLD),
                2 => *style = style.add_modifier(Modifier::DIM),
                22 => *style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
                30..=37 => *style = style.fg(Color::Indexed((num - 30) as u8)),
                39 => *style = style.fg(Color::Reset),
                40..=47 => *style = style.bg(Color::Indexed((num - 40) as u8)),
                49 => *style = style.bg(Color::Reset),
                90..=97 => *style = style.fg(Color::Indexed((num - 90 + 8) as u8)),
                100..=107 => *style = style.bg(Color::Indexed((num - 100 + 8) as u8)),
                _ => {}
            }
        }
    }
}

fn start_loading_songs(app: &mut App, cookie: String) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = fetch_songs(&cookie).map_err(|e| e.to_string());
        let _ = tx.send(LoadingResult::Songs(result));
    });
    app.loading = Some(LoadingState {
        message: "Loading your songs...".to_string(),
        kind: LoadingKind::Songs,
        receiver: rx,
    });
}

fn start_loading_tracks(app: &mut App, cookie: String, song: Song) {
    let song_url = song.url.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = fetch_song_details(&cookie, &song_url).map_err(|e| e.to_string());
        let _ = tx.send(LoadingResult::Tracks(result));
    });
    app.loading = Some(LoadingState {
        message: format!("Loading tracks for {}...", song.title),
        kind: LoadingKind::Tracks { song },
        receiver: rx,
    });
}

fn update_loading_state(app: &mut App) {
    let Some(loading) = app.loading.as_mut() else {
        return;
    };

    match loading.receiver.try_recv() {
        Ok(result) => {
            let kind = match &loading.kind {
                LoadingKind::Songs => LoadingKind::Songs,
                LoadingKind::Tracks { song } => LoadingKind::Tracks { song: song.clone() },
            };
            app.loading = None;
            match (kind, result) {
                (LoadingKind::Songs, LoadingResult::Songs(Ok(songs))) => {
                    app.songs = songs;
                    app.songs_state = ListState::default();
                    if !app.songs.is_empty() {
                        app.songs_state.select(Some(0));
                    }
                    app.screen = Screen::Songs;
                }
                (LoadingKind::Songs, LoadingResult::Songs(Err(err))) => {
                    app.status = format!("Failed to load songs: {}", err);
                }
                (LoadingKind::Tracks { song }, LoadingResult::Tracks(Ok(details))) => {
                    app.tracks = details
                        .tracks
                        .into_iter()
                        .map(|name| Track {
                            name,
                            selected: true,
                        })
                        .collect();
                    app.tracks_state = ListState::default();
                    if !app.tracks.is_empty() {
                        app.tracks_state.select(Some(0));
                    }
                    app.current_song = Some(song);
                    app.intro_click = false;
                    app.key_shift = 0;
                    app.base_key = details.base_key;
                    app.detail_focus = DetailFocus::Tracks;
                    app.screen = Screen::SongDetail;
                }
                (LoadingKind::Tracks { .. }, LoadingResult::Tracks(Err(err))) => {
                    app.status = format!("Failed to load tracks: {}", err);
                }
                _ => {
                    app.status = "Unexpected loading result".to_string();
                }
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            app.loading = None;
            app.status = "Loading failed (channel closed)".to_string();
        }
    }
}

fn render_loading_overlay(frame: &mut ratatui::Frame, app: &App) {
    let Some(loading) = &app.loading else {
        return;
    };
    let area = centered_rect(60, 20, frame.size());
    let spinner = ["|", "/", "-", "\\"][app.spinner_index % 4];
    let text = vec![
        Line::from(Span::styled(
            "Please wait",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            format!("{} {}", spinner, loading.message),
            Style::default().fg(Color::White),
        )),
    ];

    frame.render_widget(Clear, area);
    let block = Block::default()
        .title("Working")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let panel = Paragraph::new(Text::from(text))
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);
    let vertical = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1]);
    vertical[1]
}

fn render_confirm_overlay(frame: &mut ratatui::Frame, app: &App) {
    let Some(confirm) = &app.confirm else {
        return;
    };
    let area = centered_rect(70, 30, frame.size());
    frame.render_widget(Clear, area);

    let mut lines = vec![Line::from(Span::styled(
        "Please confirm",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));
    for line in &confirm.message {
        lines.push(Line::from(Span::raw(line)));
    }

    lines.push(Line::from(""));
    let yes_style = if confirm.selected == 0 {
        Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };
    let no_style = if confirm.selected == 1 {
        Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    };
    lines.push(Line::from(vec![
        Span::styled(" YES ", yes_style),
        Span::raw("   "),
        Span::styled(" NO ", no_style),
    ]));

    let panel = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title("Confirm")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn handle_confirm_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    let Some(confirm) = &mut app.confirm else {
        return Ok(false);
    };
    match key.code {
        KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
            confirm.selected = (confirm.selected + 1) % 2;
        }
        KeyCode::Esc => {
            app.confirm = None;
        }
        KeyCode::Enter => {
            let confirm = app.confirm.take().unwrap();
            match confirm.selected {
                0 => {
                    let ConfirmPayload::StartDownload {
                        song,
                        selected_tracks,
                        intro_click,
                        key_shift,
                    } = confirm.payload;
                    let download_dir =
                        default_download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                    let download_tracks = selected_tracks
                        .iter()
                        .map(|name| TrackStatusItem {
                            name: name.clone(),
                            status: TrackStatus::Pending,
                        })
                        .collect::<Vec<_>>();
                    let song_url = song.url.clone();
                    let logs = app.logs.clone();
                    let handle = std::thread::spawn(move || {
                        let _guard = LogScope::new(logs);
                        start_download(&song_url, intro_click, key_shift, selected_tracks)
                    });
                    app.download = Some(DownloadState {
                        song,
                        tracks: download_tracks,
                        handle: Some(handle),
                        done: None,
                        started_at: Instant::now(),
                        duration: None,
                        download_dir,
                    });
                    app.screen = Screen::DownloadStatus;
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(false)
}

fn extract_base_key(doc: &Html, selector: &Selector) -> Option<String> {
    for node in doc.select(selector) {
        let text = normalize_text(node.text());
        if let Some(rest) = text.strip_prefix("In the same key as the original:") {
            let key = rest.trim().split_whitespace().last().unwrap_or_default();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }
    None
}

fn format_key_display(base_key: Option<&str>, shift: i8) -> String {
    match base_key {
        Some(base) => {
            let transposed = transpose_key(base, shift);
            if shift == 0 {
                format!("0 ({})", transposed)
            } else {
                format!("{:+} ({})", shift, transposed)
            }
        }
        None => format!("{:+}", shift),
    }
}

fn transpose_key(base_key: &str, shift: i8) -> String {
    if shift == 0 {
        return base_key.to_string();
    }

    let base = base_key.trim();
    let (use_flats, normalized) = normalize_key(base);

    let sharp_scale = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let flat_scale = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];

    let scale = if use_flats { &flat_scale } else { &sharp_scale };
    let idx = scale.iter().position(|k| *k == normalized);
    if let Some(idx) = idx {
        let shift = (shift as i32).rem_euclid(12) as usize;
        scale[(idx + shift) % scale.len()].to_string()
    } else {
        base_key.to_string()
    }
}

fn normalize_key(value: &str) -> (bool, &str) {
    let upper = value.trim();
    let use_flats = upper.contains('b');
    let normalized = match upper {
        "Cb" => "B",
        "B#" => "C",
        "Db" => "Db",
        "C#" => "C#",
        "Eb" => "Eb",
        "D#" => "D#",
        "Fb" => "E",
        "E#" => "F",
        "Gb" => "Gb",
        "F#" => "F#",
        "Ab" => "Ab",
        "G#" => "G#",
        "Bb" => "Bb",
        "A#" => "A#",
        "C" | "D" | "E" | "F" | "G" | "A" | "B" => upper,
        _ => upper,
    };
    (use_flats, normalized)
}

fn default_download_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let download_dir = home.join("Downloads");
    Some(download_dir)
}

fn open_in_file_explorer(path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .status()
            .map_err(|e| anyhow!(e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|e| anyhow!(e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .status()
            .map_err(|e| anyhow!(e))?;
    }
    Ok(())
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{}m {}s", minutes, seconds)
}
