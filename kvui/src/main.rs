use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, terminal};
use kv_core::{driver, keystore, tasks};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use scraper::{Html, Selector};
use std::fs::File;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;
use url::Url;

mod catalog;

#[derive(Clone, Debug)]
struct Song {
    title: String,
    artist: String,
    url: String,
    first_seen: i64,
    purchase_date: i64,
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
enum SongsMode {
    Browse,
    ResetProgress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SongSort {
    DateDescending,
    Artist,
    Title,
}

impl SongSort {
    fn next(self) -> Self {
        match self {
            Self::DateDescending => Self::Artist,
            Self::Artist => Self::Title,
            Self::Title => Self::DateDescending,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DateDescending => "Recently added",
            Self::Artist => "Artist",
            Self::Title => "Title",
        }
    }
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
    all_songs: Vec<Song>,
    songs_state: ListState,
    songs_mode: SongsMode,
    song_sort: SongSort,
    song_filter: String,
    editing_song_filter: bool,
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
    log_follow: bool,
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
            all_songs: Vec::new(),
            songs_state: ListState::default(),
            songs_mode: SongsMode::Browse,
            song_sort: SongSort::DateDescending,
            song_filter: String::new(),
            editing_song_filter: false,
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
            log_follow: true,
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
                "Resync Songs List".to_string(),
                "Reset Song Progress".to_string(),
                "Reset All Download Progress".to_string(),
                "Logout".to_string(),
                "Quit".to_string(),
            ]
        } else {
            vec![
                "Browse My Songs".to_string(),
                "Resync Songs List".to_string(),
                "Reset Song Progress".to_string(),
                "Reset All Download Progress".to_string(),
                "Login".to_string(),
                "Quit".to_string(),
            ]
        };
        self.menu_index = 0;
    }
}

fn demo_songs() -> Vec<Song> {
    vec![
        Song {
            title: "Don't You (Forget About Me)".to_string(),
            artist: "Simple Minds".to_string(),
            url: "https://example.com/simple-minds".to_string(),
            first_seen: 5,
            purchase_date: 20260206,
        },
        Song {
            title: "What I Like About You".to_string(),
            artist: "The Romantics".to_string(),
            url: "https://example.com/the-romantics".to_string(),
            first_seen: 4,
            purchase_date: 20251117,
        },
        Song {
            title: "Don't Look Back in Anger".to_string(),
            artist: "Oasis".to_string(),
            url: "https://example.com/oasis".to_string(),
            first_seen: 3,
            purchase_date: 20251127,
        },
        Song {
            title: "Livin' on a Prayer".to_string(),
            artist: "Bon Jovi".to_string(),
            url: "https://example.com/bon-jovi".to_string(),
            first_seen: 2,
            purchase_date: 20250831,
        },
        Song {
            title: "Use Somebody".to_string(),
            artist: "Kings of Leon".to_string(),
            url: "https://example.com/kings-of-leon".to_string(),
            first_seen: 1,
            purchase_date: 20250831,
        },
    ]
}

fn demo_tracks() -> Vec<String> {
    [
        "Click",
        "Drum Kit",
        "Bass",
        "Electric Guitar (clean)",
        "Electric Guitar (crunch)",
        "Piano",
        "Synth Pad",
        "Backing Vocals",
        "Lead Vocal",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn configure_demo_screen(app: &mut App, screen: DemoScreen) {
    app.menu_items = vec![
        "Browse My Songs".to_string(),
        "Resync Songs List".to_string(),
        "Reset Song Progress".to_string(),
        "Reset All Download Progress".to_string(),
        "Logout".to_string(),
        "Quit".to_string(),
    ];
    app.status = "Ready".to_string();

    match screen {
        DemoScreen::Menu => app.screen = Screen::Menu,
        DemoScreen::Songs => {
            app.all_songs = demo_songs();
            apply_song_view(app);
            app.screen = Screen::Songs;
        }
        DemoScreen::Song => {
            let song = demo_songs().remove(0);
            app.current_song = Some(song);
            app.tracks = demo_tracks()
                .into_iter()
                .map(|name| Track {
                    name,
                    selected: true,
                })
                .collect();
            app.tracks_state.select(Some(1));
            app.base_key = Some("E".to_string());
            app.intro_click = true;
            app.detail_focus = DetailFocus::Tracks;
            app.screen = Screen::SongDetail;
        }
        DemoScreen::Download | DemoScreen::Complete => {
            let song = demo_songs().remove(0);
            let statuses = [
                TrackStatus::Done,
                TrackStatus::Done,
                TrackStatus::Done,
                TrackStatus::Done,
                TrackStatus::Done,
                TrackStatus::Downloading,
                TrackStatus::Pending,
                TrackStatus::Failed,
                TrackStatus::Pending,
            ];
            let tracks = demo_tracks()
                .into_iter()
                .zip(statuses)
                .map(|(name, status)| TrackStatusItem {
                    name,
                    attempt: if status == TrackStatus::Downloading { 2 } else { 0 },
                    status,
                })
                .collect();
            let complete = matches!(screen, DemoScreen::Complete);
            app.download = Some(DownloadState {
                song,
                count_in: true,
                transpose: 0,
                tracks,
                handle: None,
                done: complete.then_some(Err(
                    "1 track failed after retries; completed tracks were saved.".to_string(),
                )),
                started_at: Instant::now(),
                duration: complete.then_some(Duration::from_secs(154)),
                download_dir: dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join("Downloads"),
            });
            if complete {
                if let Some(download) = app.download.as_mut() {
                    for track in &mut download.tracks {
                        if track.status == TrackStatus::Downloading
                            || track.status == TrackStatus::Pending
                        {
                            track.status = TrackStatus::Done;
                        }
                    }
                }
                app.screen = Screen::DownloadDone;
            } else {
                if let Ok(mut logs) = app.logs.lock() {
                    logs.push_line("INFO  Processing track 7 'Piano'".to_string());
                    logs.push_line("INFO  Attempt 2/3 for 'Piano'".to_string());
                    logs.push_line("INFO  Waiting for 'Piano' to be prepared".to_string());
                }
                app.screen = Screen::DownloadStatus;
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "kvui", version, about = "KV Downloader terminal UI")]
struct Cli {
    #[arg(
        long,
        value_enum,
        hide = true,
        help = "Render a deterministic screen for documentation screenshots"
    )]
    demo_screen: Option<DemoScreen>,

    #[arg(
        long,
        value_name = "PATH",
        num_args = 0..=1,
        default_missing_value = "kvui-diagnostics.log",
        help = "Write detailed diagnostics to a log file"
    )]
    diagnostics: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DemoScreen {
    Menu,
    Songs,
    Song,
    Download,
    Complete,
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let cli = Cli::parse();
    run_tui(cli.diagnostics, cli.demo_screen)
}

fn run_tui(diagnostics_path: Option<PathBuf>, demo_screen: Option<DemoScreen>) -> Result<()> {
    let logs = Arc::new(Mutex::new(LogBuffer::new(1000)));
    let diagnostics = match diagnostics_path.as_deref() {
        Some(path) => Some(Arc::new(Mutex::new(File::create(path)?))),
        None => None,
    };
    setup_tracing(logs.clone(), diagnostics);
    if let Some(path) = diagnostics_path {
        tracing::info!("Writing detailed diagnostics to {}", path.display());
    }

    let mut terminal = setup_terminal()?;
    let tick_rate = Duration::from_millis(80);
    let mut last_tick = Instant::now();
    let mut app = App::new(logs);
    if let Some(screen) = demo_screen {
        configure_demo_screen(&mut app, screen);
    }

    loop {
        if demo_screen.is_none() {
            update_download_state(&mut app);
            update_loading_state(&mut app);
        }
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
    if app.confirm.is_some() {
        return handle_confirm_keys(app, key);
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
                    catalog::SongCatalog::open()?.clear()?;
                    kv_core::download_progress::DownloadProgress::new().clear()?;
                    app.songs.clear();
                    app.all_songs.clear();
                    app.song_filter.clear();
                    app.status = "Logged out; local catalog and download progress cleared".to_string();
                    app.refresh_menu();
                }
                "Browse My Songs" => {
                    app.status = "".to_string();
                    app.songs_mode = SongsMode::Browse;
                    let cached = catalog::SongCatalog::open()?.list()?;
                    if cached.is_empty() {
                        match cookie_value() {
                            Ok(cookie) => start_loading_songs(app, cookie, false),
                            Err(err) => app.status = format!("Not authenticated: {}", err),
                        }
                    } else {
                        set_song_list(app, cached);
                        if let Ok(cookie) = cookie_value() {
                            start_loading_songs(app, cookie, false);
                        }
                    }
                }
                "Resync Songs List" => {
                    app.status = "".to_string();
                    app.songs_mode = SongsMode::Browse;
                    match cookie_value() {
                        Ok(cookie) => start_loading_songs(app, cookie, true),
                        Err(err) => app.status = format!("Not authenticated: {}", err),
                    }
                }
                "Reset Song Progress" => {
                    app.status = "".to_string();
                    app.songs_mode = SongsMode::ResetProgress;
                    let progress = kv_core::download_progress::DownloadProgress::new();
                    let tracked = progress
                        .tracked_song_urls()?
                        .into_iter()
                        .collect::<std::collections::HashSet<_>>();
                    let songs = catalog::SongCatalog::open()?
                        .list()?
                        .into_iter()
                        .filter(|song| tracked.contains(&song.url))
                        .collect::<Vec<_>>();
                    if songs.is_empty() {
                        app.status = "No saved song progress to reset".to_string();
                    } else {
                        set_song_list(app, songs);
                    }
                }
                "Reset All Download Progress" => {
                    app.confirm = Some(ConfirmState {
                        message: vec![
                            "Delete the entire download progress file?".to_string(),
                            "All completed-track history in this file will be lost.".to_string(),
                        ],
                        selected: 1,
                        payload: ConfirmPayload::ResetAllProgress,
                    });
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

fn set_song_list(app: &mut App, songs: Vec<Song>) {
    app.all_songs = songs;
    apply_song_view(app);
    app.screen = Screen::Songs;
}

fn apply_song_view(app: &mut App) {
    let filter = app.song_filter.to_lowercase();
    app.songs = app
        .all_songs
        .iter()
        .filter(|song| {
            filter.is_empty()
                || song.title.to_lowercase().contains(&filter)
                || song.artist.to_lowercase().contains(&filter)
        })
        .cloned()
        .collect();
    match app.song_sort {
        SongSort::DateDescending => app.songs.sort_by(|left, right| {
            right
                .purchase_date
                .cmp(&left.purchase_date)
                .then_with(|| right.first_seen.cmp(&left.first_seen))
                .then_with(|| left.artist.to_lowercase().cmp(&right.artist.to_lowercase()))
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        }),
        SongSort::Artist => app.songs.sort_by(|left, right| {
            left.artist
                .to_lowercase()
                .cmp(&right.artist.to_lowercase())
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        }),
        SongSort::Title => app
            .songs
            .sort_by_key(|song| (song.title.to_lowercase(), song.artist.to_lowercase())),
    }
    app.songs_state = ListState::default();
    if !app.songs.is_empty() {
        app.songs_state.select(Some(0));
    }
}

fn handle_songs_keys(
    _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    if app.editing_song_filter {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => app.editing_song_filter = false,
            KeyCode::Backspace => {
                app.song_filter.pop();
                apply_song_view(app);
            }
            KeyCode::Char(character) => {
                app.song_filter.push(character);
                apply_song_view(app);
            }
            _ => {}
        }
        return Ok(false);
    }

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
        KeyCode::Char('/') => {
            app.editing_song_filter = true;
        }
        KeyCode::Char('s') => {
            app.song_sort = app.song_sort.next();
            apply_song_view(app);
        }
        KeyCode::Enter => {
            if let Some(idx) = app.songs_state.selected() {
                if let Some(song) = app.songs.get(idx).cloned() {
                    app.status = "".to_string();
                    match app.songs_mode {
                        SongsMode::Browse => match cookie_value() {
                            Ok(cookie) => start_loading_tracks(app, cookie, song),
                            Err(err) => app.status = format!("Not authenticated: {}", err),
                        },
                        SongsMode::ResetProgress => {
                            app.confirm = Some(ConfirmState {
                                message: vec![
                                    format!("Reset all saved progress for '{}' by {}?", song.title, song.artist),
                                    "All count-in and transpose variants for this song will be removed."
                                        .to_string(),
                                ],
                                selected: 1,
                                payload: ConfirmPayload::ResetSong {
                                    url: song.url,
                                    title: song.title,
                                },
                            });
                        }
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
            Constraint::Length(8),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.size());

    let logo = vec![
        Line::from(Span::styled(
            r"██╗  ██╗██╗   ██╗      ██████╗  ██████╗ ██╗    ██╗███╗   ██╗██╗      ██████╗  █████╗ ██████╗ ███████╗██████╗ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"██║ ██╔╝██║   ██║      ██╔══██╗██╔═══██╗██║    ██║████╗  ██║██║     ██╔═══██╗██╔══██╗██╔══██╗██╔════╝██╔══██╗",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"█████╔╝ ██║   ██║█████╗██║  ██║██║   ██║██║ █╗ ██║██╔██╗ ██║██║     ██║   ██║███████║██║  ██║█████╗  ██████╔╝",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"██╔═██╗ ╚██╗ ██╔╝╚════╝██║  ██║██║   ██║██║███╗██║██║╚██╗██║██║     ██║   ██║██╔══██║██║  ██║██╔══╝  ██╔══██╗",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"██║  ██╗ ╚████╔╝       ██████╔╝╚██████╔╝╚███╔███╔╝██║ ╚████║███████╗╚██████╔╝██║  ██║██████╔╝███████╗██║  ██║",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"╚═╝  ╚═╝  ╚═══╝        ╚═════╝  ╚═════╝  ╚══╝╚══╝ ╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝",
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
                Style::default().fg(Color::Reset)
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

    let base_title = match app.songs_mode {
        SongsMode::Browse => "My Songs",
        SongsMode::ResetProgress => "Choose Song to Reset Progress",
    };
    let filter = if app.song_filter.is_empty() {
        String::new()
    } else {
        format!(" | Filter: {}", app.song_filter)
    };
    let editing = if app.editing_song_filter { " [typing]" } else { "" };
    let title = format!(
        "{} | Sort: {}{}{}",
        base_title,
        app.song_sort.label(),
        filter,
        editing
    );
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[0], &mut app.songs_state);

    let help_text = match app.songs_mode {
        SongsMode::Browse => "Enter: open | /: filter | s: sort | ESC: menu",
        SongsMode::ResetProgress => "Enter: reset | /: filter | s: sort | ESC: menu",
    };
    let help = Paragraph::new(help_text)
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
                Style::default().fg(Color::Reset)
            } else {
                Style::default().fg(Color::DarkGray)
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
                    TrackStatus::Pending => Style::default().fg(Color::DarkGray),
                };
                let attempt = match track.status {
                    TrackStatus::Downloading => format!(" (attempt {}/3)", track.attempt.max(1)),
                    TrackStatus::Failed => " (failed after 3 attempts)".to_string(),
                    _ => String::new(),
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{} {}{}", marker, track.name, attempt),
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
    if app.log_follow {
        app.log_scroll = max_scroll;
    } else if app.log_scroll > max_scroll {
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

    let footer = Paragraph::new("Up/Down: scroll logs | End: follow latest | ESC: menu (after finish)")
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

    let (heading, heading_color) = match download.done.as_ref() {
        Some(Ok(())) => ("Download complete", Color::Green),
        Some(Err(_)) => ("Download finished with errors", Color::Red),
        None => ("Download finished", Color::Yellow),
    };
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        heading,
        Style::default()
            .fg(heading_color)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(Err(error)) = download.done.as_ref() {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }
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
    let completed = download
        .tracks
        .iter()
        .filter(|track| track.status == TrackStatus::Done)
        .count();
    let failed = download
        .tracks
        .iter()
        .filter(|track| track.status == TrackStatus::Failed)
        .count();
    lines.push(Line::from(format!(
        "Tracks: {} completed, {} failed",
        completed, failed
    )));
    for track in &download.tracks {
        let marker = match track.status {
            TrackStatus::Done => "✓",
            TrackStatus::Failed => "✗",
            _ => "-",
        };
        lines.push(Line::from(format!("  {} {}", marker, track.name)));
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
                Style::default().fg(Color::Reset)
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
    let user = kv_core::prompt::prompt("Username: ", false)?;
    let pass = kv_core::prompt::prompt("Password: ", true)?;

    keystore::Keystore::login(&user, &pass)?;

    let config = driver::Config {
        domain: "www.karaoke-version.com".to_string(),
        headless: false,
        download_path: None,
        idle_browser_timeout: Duration::from_secs(300),
    };
    tracing::info!("Launching Chromium for login (headless: false)");
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
    let domain = extract_domain_from_url(song_url)
        .ok_or_else(|| anyhow!("Missing domain from url"))?;
    let download_options = tasks::download_song::DownloadOptions {
        count_in,
        transpose,
        selected_tracks: if selected_tracks.is_empty() {
            None
        } else {
            Some(selected_tracks)
        },
        force_restart: false,
    };
    tracing::info!("Using HTTP downloader");
    tasks::download_song::download_song_http(song_url, download_options, None, &domain)?;
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

fn sync_song_catalog(cookie: &str, full_resync: bool) -> Result<Vec<Song>> {
    let catalog = catalog::SongCatalog::open()?;
    let mut first_url = Url::parse("https://www.karaoke-version.com/my/download.html")?;
    first_url
        .query_pairs_mut()
        .append_pair("orderField", "add_date")
        .append_pair("orderSort", "desc");

    let mut next_url = Some(first_url.to_string());
    let mut discovered = Vec::new();
    let mut visited_pages = std::collections::HashSet::new();
    while let Some(page_url) = next_url.take() {
        if !visited_pages.insert(page_url.clone()) {
            break;
        }
        tracing::info!("Syncing songs page {}", page_url);
        let (songs, next) = fetch_songs_page(cookie, &page_url)?;
        let mut reached_cached_song = false;
        for song in songs {
            if !full_resync && catalog.contains(&song.url)? {
                reached_cached_song = true;
                break;
            }
            discovered.push(song);
        }
        if reached_cached_song {
            tracing::info!("Reached previously cached song; incremental sync complete");
            break;
        }
        next_url = next;
        if visited_pages.len() >= 500 {
            return Err(anyhow!("Stopped song sync after 500 pages"));
        }
    }

    if full_resync {
        catalog.clear()?;
    }
    catalog.upsert(&discovered)?;
    let songs = catalog.list()?;
    tracing::info!(
        "Song catalog sync complete: {} discovered, {} cached total",
        discovered.len(),
        songs.len()
    );
    Ok(songs)
}

fn fetch_songs_page(cookie: &str, page_url: &str) -> Result<(Vec<Song>, Option<String>)> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("kv-downloader-tui")
        .build()?;

    let res = client
        .get(page_url)
        .header("Cookie", format!("karaoke-version={}", cookie))
        .send()?;

    let final_url = res.url().clone();
    let mut body = res.text()?;
    if is_browser_verification_page(&body) {
        tracing::warn!("Browser verification interrupted song browsing; opening Chromium");
        body = fetch_page_in_visible_browser(
            page_url,
            cookie,
            "my-downloaded-files",
        )?;
    }
    if is_login_page(&body, final_url.as_str()) {
        return Err(anyhow!(
            "Your saved session is no longer accepted. Choose Login to refresh it."
        ));
    }
    let doc = Html::parse_document(&body);

    let row_selector = Selector::parse("table.my-downloaded-files tr.vam").unwrap();
    let song_selector = Selector::parse("td.my-downloaded-files__song a").unwrap();
    let artist_selector = Selector::parse("td.my-downloaded-files__artist a").unwrap();
    let date_selector = Selector::parse("td.my-downloaded-files__date").unwrap();

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

            let purchase_date = row
                .select(&date_selector)
                .next()
                .map(|date| normalize_text(date.text()))
                .and_then(|date| parse_purchase_date(&date))
                .unwrap_or(0);
            songs.push(Song {
                title,
                artist,
                url,
                first_seen: 0,
                purchase_date,
            });
        }
    }

    if songs.is_empty() {
        write_kvui_debug_page("kvui_songs.html", &body);
        return Err(anyhow!(
            "The download page loaded, but its song list was not recognized. Set KV_HTTP_DEBUG=1 and inspect kvui_songs.html."
        ));
    }

    let next_url = find_next_songs_page(&doc, page_url);
    Ok((songs, next_url))
}

fn find_next_songs_page(doc: &Html, page_url: &str) -> Option<String> {
    let selector = Selector::parse("a[rel='next'], a.next, .pagination a, .pager a").ok()?;
    let base = Url::parse(page_url).ok()?;
    for link in doc.select(&selector) {
        let text = normalize_text(link.text()).to_lowercase();
        let class = link.value().attr("class").unwrap_or("").to_lowercase();
        let rel = link.value().attr("rel").unwrap_or("").to_lowercase();
        let is_next = rel.contains("next")
            || class.contains("next")
            || text.contains("next")
            || text.contains("read all")
            || text == ">"
            || text == "›";
        if is_next {
            if let Some(href) = link.value().attr("href") {
                if let Ok(mut url) = base.join(href) {
                    if !url.query_pairs().any(|(key, _)| key == "orderField") {
                        url.query_pairs_mut()
                            .append_pair("orderField", "add_date")
                            .append_pair("orderSort", "desc");
                    }
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

fn parse_purchase_date(value: &str) -> Option<i64> {
    let mut parts = value.split('/');
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    let year = parts.next()?.parse::<i64>().ok()?;
    let year = if year < 100 { 2000 + year } else { year };
    Some(year * 10_000 + month * 100 + day)
}

fn is_browser_verification_page(html: &str) -> bool {
    let normalized = html.to_lowercase();
    normalized.contains("cdn-cgi/challenge-platform")
        || normalized.contains("cf-challenge")
        || normalized.contains("cf-turnstile")
        || normalized.contains("__cf_chl")
        || normalized.contains("verify you are human")
        || normalized.contains("just a moment")
        || normalized.contains("fastly")
        || normalized.contains("<title>client challenge</title>")
        || normalized.contains("/_fs-ch-")
        || normalized.contains("guru meditation")
        || normalized.contains("varnish cache server")
}

fn fetch_page_in_visible_browser(
    url: &str,
    session_cookie: &str,
    expected_content: &str,
) -> Result<String> {
    let config = driver::Config {
        domain: "www.karaoke-version.com".to_string(),
        headless: false,
        download_path: None,
        idle_browser_timeout: Duration::from_secs(300),
    };
    let driver = driver::Driver::new(config);
    let tab = driver.browser.new_tab()?;
    tab.navigate_to("https://www.karaoke-version.com")?
        .wait_until_navigated()?;
    driver.set_session_cookie(&tab, session_cookie)?;
    tab.navigate_to(url)?.wait_until_navigated()?;

    tracing::warn!(
        "Complete browser verification or sign in in Chromium. Waiting for the requested page..."
    );
    let started = Instant::now();
    let mut last_navigation = Instant::now();
    loop {
        let html = tab.get_content()?;
        if html.contains(expected_content) {
            tracing::info!("Requested page loaded in Chromium");
            return Ok(html);
        }

        if started.elapsed() >= Duration::from_secs(300) {
            return Err(anyhow!("Timed out waiting for the requested page in Chromium"));
        }

        if !is_browser_verification_page(&html)
            && !is_login_page(&html, &tab.get_url())
            && last_navigation.elapsed() >= Duration::from_secs(3)
        {
            tab.navigate_to(url)?.wait_until_navigated()?;
            last_navigation = Instant::now();
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn is_login_page(html: &str, final_url: &str) -> bool {
    final_url.contains("/my/login")
        || html.contains("id=\"frm_login\"")
        || html.contains("name=\"frm_login\"")
}

fn write_kvui_debug_page(filename: &str, html: &str) {
    if std::env::var("KV_HTTP_DEBUG").ok().as_deref() != Some("1") {
        return;
    }
    if let Err(err) = std::fs::write(filename, html) {
        tracing::warn!("Failed to write {}: {}", filename, err);
    } else {
        tracing::debug!("Wrote debug page to {}", filename);
    }
}

fn fetch_song_details(cookie: &str, song_url: &str) -> Result<SongDetails> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("kv-downloader-tui")
        .build()?;

    let res = client
        .get(song_url)
        .header("Cookie", format!("karaoke-version={}", cookie))
        .send()?;

    let final_url = res.url().clone();
    let mut body = res.text()?;
    if is_browser_verification_page(&body) {
        tracing::warn!("Browser verification interrupted track loading; opening Chromium");
        body = fetch_page_in_visible_browser(song_url, cookie, "track__caption")?;
    }
    if is_login_page(&body, final_url.as_str()) {
        return Err(anyhow!(
            "Your saved session is no longer accepted. Choose Login to refresh it."
        ));
    }
    write_kvui_debug_page("kvui_song.html", &body);
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
        if name.is_empty() || is_precount_track_name(&name) {
            continue;
        }

        tracks.push(name);
    }

    if tracks.is_empty() {
        write_kvui_debug_page("kvui_song.html", &body);
        return Err(anyhow!(
            "The song page loaded, but its tracks were not recognized. Set KV_HTTP_DEBUG=1 and inspect kvui_song.html."
        ));
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

    let progress = kv_core::download_progress::DownloadProgress::new();
    let completed = if progress
        .is_same_download(&download.song.url, download.count_in, download.transpose)
        .unwrap_or(false)
    {
        progress
            .get_completed_tracks()
            .unwrap_or_default()
            .into_iter()
            .map(|name| normalize_track_name(&name))
            .collect::<std::collections::HashSet<_>>()
    } else {
        std::collections::HashSet::new()
    };

    for track in &mut download.tracks {
        let normalized = normalize_track_name(&track.name);
        if completed.contains(&normalized) {
            track.status = TrackStatus::Done;
        }
    }

    let log_progress = track_log_progress(app.logs.clone());
    for track in &mut download.tracks {
        if track.status == TrackStatus::Done {
            continue;
        }
        let normalized = normalize_track_name(&track.name);
        if log_progress.failed.contains(&normalized) {
            track.status = TrackStatus::Failed;
            track.attempt = 3;
        } else if log_progress.current.as_deref() == Some(normalized.as_str()) {
            track.status = TrackStatus::Downloading;
            track.attempt = log_progress.attempts.get(&normalized).copied().unwrap_or(1);
        } else {
            track.status = TrackStatus::Pending;
            track.attempt = log_progress.attempts.get(&normalized).copied().unwrap_or(0);
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
                        if track.status == TrackStatus::Downloading {
                            track.status = TrackStatus::Failed;
                        }
                    }
                    tracing::error!("Download failed: {}", err);
                    download.done = Some(Err(err.to_string()));
                    download.duration = Some(download.started_at.elapsed());
                    app.status = format!("Download finished with errors: {}", err);
                    app.done_menu_index = 0;
                    app.screen = Screen::DownloadDone;
                }
                Err(_) => {
                    tracing::error!("Download thread panicked");
                    download.done = Some(Err("Download thread panicked".to_string()));
                    download.duration = Some(download.started_at.elapsed());
                    app.status = "Download failed: download thread panicked".to_string();
                    app.done_menu_index = 0;
                    app.screen = Screen::DownloadDone;
                }
            }
        }
    }
}

fn handle_download_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Up => {
            app.log_follow = false;
            app.log_scroll = app.log_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            app.log_follow = false;
            app.log_scroll = app.log_scroll.saturating_add(1);
        }
        KeyCode::End => {
            app.log_follow = true;
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

struct TrackLogProgress {
    current: Option<String>,
    attempts: std::collections::HashMap<String, u8>,
    failed: std::collections::HashSet<String>,
}

fn track_log_progress(logs: Arc<Mutex<LogBuffer>>) -> TrackLogProgress {
    let mut progress = TrackLogProgress {
        current: None,
        attempts: std::collections::HashMap::new(),
        failed: std::collections::HashSet::new(),
    };
    let Ok(buf) = logs.lock() else {
        return progress;
    };

    for line in &buf.lines {
        if line.contains("Processing track") {
            if let Some(track) = quoted_track_name(line) {
                progress.current = Some(normalize_track_name(&track));
            }
        }
        if let Some(index) = line.find("Attempt ") {
            let attempt = line[index + "Attempt ".len()..]
                .split('/')
                .next()
                .and_then(|value| value.parse::<u8>().ok());
            if let (Some(attempt), Some(track)) = (attempt, quoted_track_name(line)) {
                let track = normalize_track_name(&track);
                progress.attempts.insert(track.clone(), attempt);
                progress.current = Some(track);
            }
        }
        if line.contains("Giving up on") {
            if let Some(track) = quoted_track_name(line) {
                progress.failed.insert(normalize_track_name(&track));
            }
        }
    }
    progress
}

fn quoted_track_name(line: &str) -> Option<String> {
    let start = line.find('\'')?;
    let rest = &line[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

fn is_precount_track_name(value: &str) -> bool {
    let normalized = value.to_lowercase();
    normalized.contains("intro count") || normalized.contains("precount")
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
    attempt: u8,
}

struct DownloadState {
    song: Song,
    count_in: bool,
    transpose: i8,
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
    ResetSong {
        url: String,
        title: String,
    },
    ResetAllProgress,
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
    diagnostics: Option<Arc<Mutex<File>>>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let mut guard = self
            .buffer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log buffer poisoned"))?;
        guard.partial.push_str(&text);
        let mut completed_lines = Vec::new();
        while let Some(pos) = guard.partial.find('\n') {
            let line = guard.partial.drain(..=pos).collect::<String>();
            let line = line.trim_end().to_string();
            guard.push_line(line.clone());
            completed_lines.push(line);
        }
        drop(guard);

        if let Some(diagnostics) = &self.diagnostics {
            let mut file = diagnostics
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "diagnostics file poisoned"))?;
            for line in completed_lines {
                writeln!(file, "{}", strip_ansi_codes(&line))?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(diagnostics) = &self.diagnostics {
            diagnostics
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "diagnostics file poisoned"))?
                .flush()?;
        }
        Ok(())
    }
}

struct LogWriterFactory {
    buffer: Arc<Mutex<LogBuffer>>,
    diagnostics: Option<Arc<Mutex<File>>>,
}

impl<'a> MakeWriter<'a> for LogWriterFactory {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.buffer.clone(),
            diagnostics: self.diagnostics.clone(),
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

fn setup_tracing(buffer: Arc<Mutex<LogBuffer>>, diagnostics: Option<Arc<Mutex<File>>>) {
    let detailed = diagnostics.is_some();
    let writer = LogWriterFactory {
        buffer,
        diagnostics,
    };
    let filter = if detailed {
        EnvFilter::new("info,kvui=debug,kv_core=debug")
    } else {
        EnvFilter::new("info")
    };
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(true)
        .with_env_filter(filter)
        .try_init();
}

fn strip_ansi_codes(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
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

fn start_loading_songs(app: &mut App, cookie: String, full_resync: bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = sync_song_catalog(&cookie, full_resync).map_err(|e| e.to_string());
        let _ = tx.send(LoadingResult::Songs(result));
    });
    app.loading = Some(LoadingState {
        message: if full_resync {
            "Fully resyncing your song catalog...".to_string()
        } else {
            "Syncing your song catalog...".to_string()
        },
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
                    let count = songs.len();
                    set_song_list(app, songs);
                    app.status = format!("{} songs cached locally", count);
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
            Style::default().fg(Color::Reset),
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
                0 => match confirm.payload {
                    ConfirmPayload::StartDownload {
                        song,
                        selected_tracks,
                        intro_click,
                        key_shift,
                    } => {
                        app.log_scroll = 0;
                        app.log_follow = true;
                        let download_dir = default_download_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        let download_tracks = selected_tracks
                            .iter()
                            .map(|name| TrackStatusItem {
                                name: name.clone(),
                                status: TrackStatus::Pending,
                                attempt: 0,
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
                            count_in: intro_click,
                            transpose: key_shift,
                            tracks: download_tracks,
                            handle: Some(handle),
                            done: None,
                            started_at: Instant::now(),
                            duration: None,
                            download_dir,
                        });
                        app.screen = Screen::DownloadStatus;
                    }
                    ConfirmPayload::ResetSong { url, title } => {
                        let progress = kv_core::download_progress::DownloadProgress::new();
                        if progress.clear_song(&url)? {
                            tracing::info!("Reset download progress for {}", url);
                            app.status = format!("Reset download progress for '{}'", title);
                        } else {
                            app.status = format!("No saved download progress found for '{}'", title);
                        }
                        app.screen = Screen::Menu;
                    }
                    ConfirmPayload::ResetAllProgress => {
                        let progress = kv_core::download_progress::DownloadProgress::new();
                        let path = progress.path().display().to_string();
                        progress.clear()?;
                        tracing::info!("Deleted download progress file {}", path);
                        app.status = "All download progress reset".to_string();
                    }
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

#[cfg(test)]
mod song_catalog_tests {
    use super::{find_next_songs_page, parse_purchase_date, Html};

    #[test]
    fn parses_purchase_date_for_local_sorting() {
        assert_eq!(parse_purchase_date("2/6/26"), Some(20260206));
        assert_eq!(parse_purchase_date("11/17/2025"), Some(20251117));
    }

    #[test]
    fn follows_next_page_and_preserves_date_sort() {
        let document = Html::parse_document(
            r#"<div class="pagination"><a rel="next" href="/my/download.html?page=2">Next &gt;</a></div>"#,
        );
        let next = find_next_songs_page(
            &document,
            "https://www.karaoke-version.com/my/download.html?orderField=add_date&orderSort=desc",
        )
        .unwrap();
        assert!(next.contains("page=2"));
        assert!(next.contains("orderField=add_date"));
        assert!(next.contains("orderSort=desc"));
    }
}
