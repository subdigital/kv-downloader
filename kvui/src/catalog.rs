use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

use crate::Song;

pub struct SongCatalog {
    connection: Connection,
}

impl SongCatalog {
    pub fn open() -> Result<Self> {
        let directory = dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kv-downloader");
        std::fs::create_dir_all(&directory)?;
        let connection = Connection::open(directory.join("songs.sqlite3"))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS songs (
                 url TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 artist TEXT NOT NULL,
                 first_seen INTEGER NOT NULL,
                 last_seen INTEGER NOT NULL,
                 purchase_date INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS songs_artist_title
                 ON songs(artist COLLATE NOCASE, title COLLATE NOCASE);",
        )?;
        let _ = connection.execute(
            "ALTER TABLE songs ADD COLUMN purchase_date INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Ok(Self { connection })
    }

    pub fn contains(&self, url: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM songs WHERE url = ?1)",
            [url],
            |row| row.get(0),
        )?)
    }

    pub fn upsert(&self, songs: &[Song]) -> Result<()> {
        let now = unix_timestamp();
        let mut statement = self.connection.prepare(
            "INSERT INTO songs(url, title, artist, first_seen, last_seen, purchase_date)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5)
             ON CONFLICT(url) DO UPDATE SET
                 title = excluded.title,
                 artist = excluded.artist,
                 last_seen = excluded.last_seen,
                 purchase_date = excluded.purchase_date",
        )?;
        for (index, song) in songs.iter().enumerate() {
            statement.execute(params![
                song.url,
                song.title,
                song.artist,
                now - index as i64,
                song.purchase_date
            ])?;
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Song>> {
        let mut statement = self.connection.prepare(
            "SELECT title, artist, url, first_seen, purchase_date
             FROM songs
             ORDER BY purchase_date DESC, first_seen DESC, artist COLLATE NOCASE, title COLLATE NOCASE",
        )?;
        let songs = statement
            .query_map([], |row| {
                Ok(Song {
                    title: row.get(0)?,
                    artist: row.get(1)?,
                    url: row.get(2)?,
                    first_seen: row.get(3)?,
                    purchase_date: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(songs)
    }

    pub fn clear(&self) -> Result<()> {
        self.connection.execute("DELETE FROM songs", [])?;
        Ok(())
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
