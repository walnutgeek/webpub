use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::Node;

/// Storage error type
#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Serialization(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "IO error: {}", e),
            StorageError::Sqlite(e) => write!(f, "SQLite error: {}", e),
            StorageError::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Server storage with sharded SQLite databases for chunks
/// and a central index database for sites, snapshots, and tokens.
pub struct Storage {
    base_path: PathBuf,
    index: Mutex<Connection>,
    chunk_dbs: Mutex<HashMap<u8, Connection>>,
}

impl Storage {
    /// Open or create storage at the given path
    pub fn open(path: &Path) -> Result<Self> {
        // Create base directory if needed
        fs::create_dir_all(path)?;

        // Create chunks directory
        let chunks_path = path.join("chunks");
        fs::create_dir_all(&chunks_path)?;

        // Open/create index database
        let index_path = path.join("index.db");
        let index = Connection::open(&index_path)?;

        // Initialize index schema
        index.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sites (
                id INTEGER PRIMARY KEY,
                hostname TEXT UNIQUE NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY,
                site_id INTEGER NOT NULL REFERENCES sites(id),
                tree_data BLOB NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                is_current INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_snapshots_site ON snapshots(site_id);

            CREATE TABLE IF NOT EXISTS tokens (
                id INTEGER PRIMARY KEY,
                token TEXT UNIQUE NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        Ok(Storage {
            base_path: path.to_path_buf(),
            index: Mutex::new(index),
            chunk_dbs: Mutex::new(HashMap::new()),
        })
    }

    /// Get the chunk database connection for a given hash prefix
    fn get_chunk_db(&self, prefix: u8) -> Result<()> {
        let mut dbs = self.chunk_dbs.lock().unwrap();
        if !dbs.contains_key(&prefix) {
            let db_path = self
                .base_path
                .join("chunks")
                .join(format!("{:02x}.db", prefix));
            let conn = Connection::open(&db_path)?;
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS chunks (
                    hash BLOB PRIMARY KEY,
                    data BLOB NOT NULL
                )
                "#,
                [],
            )?;
            dbs.insert(prefix, conn);
        }
        Ok(())
    }

    /// Store a chunk
    pub fn store_chunk(&self, hash: &[u8; 32], data: &[u8]) -> Result<()> {
        let prefix = hash[0];
        self.get_chunk_db(prefix)?;

        let dbs = self.chunk_dbs.lock().unwrap();
        let conn = dbs.get(&prefix).unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO chunks (hash, data) VALUES (?1, ?2)",
            params![hash.as_slice(), data],
        )?;

        Ok(())
    }

    /// Get a chunk by hash
    pub fn get_chunk(&self, hash: &[u8; 32]) -> Result<Option<Vec<u8>>> {
        let prefix = hash[0];
        self.get_chunk_db(prefix)?;

        let dbs = self.chunk_dbs.lock().unwrap();
        let conn = dbs.get(&prefix).unwrap();

        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT data FROM chunks WHERE hash = ?1",
                params![hash.as_slice()],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result)
    }

    /// Check which chunks from a list exist in storage
    pub fn has_chunks(&self, hashes: &[[u8; 32]]) -> Result<Vec<[u8; 32]>> {
        let mut found = Vec::new();

        // Check each hash in order to maintain input order
        for hash in hashes {
            let prefix = hash[0];
            self.get_chunk_db(prefix)?;

            let dbs = self.chunk_dbs.lock().unwrap();
            let conn = dbs.get(&prefix).unwrap();

            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM chunks WHERE hash = ?1",
                    params![hash.as_slice()],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);

            if exists {
                found.push(*hash);
            }
        }

        Ok(found)
    }

    /// Generate and add a new token
    pub fn add_token(&self) -> Result<String> {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        let token = hex::encode(bytes);

        let index = self.index.lock().unwrap();
        index.execute("INSERT INTO tokens (token) VALUES (?1)", params![&token])?;

        Ok(token)
    }

    /// Verify if a token is valid
    pub fn verify_token(&self, token: &str) -> Result<bool> {
        let index = self.index.lock().unwrap();
        let exists: bool = index
            .query_row(
                "SELECT 1 FROM tokens WHERE token = ?1",
                params![token],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        Ok(exists)
    }

    /// Revoke a token
    pub fn revoke_token(&self, token: &str) -> Result<()> {
        let index = self.index.lock().unwrap();
        index.execute("DELETE FROM tokens WHERE token = ?1", params![token])?;
        Ok(())
    }

    /// List all tokens
    pub fn list_tokens(&self) -> Result<Vec<String>> {
        let index = self.index.lock().unwrap();
        let mut stmt = index.prepare("SELECT token FROM tokens")?;
        let tokens: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tokens)
    }

    /// Get or create a site ID
    fn get_or_create_site(&self, hostname: &str) -> Result<i64> {
        let index = self.index.lock().unwrap();

        // Try to find existing site
        let existing: Option<i64> = index
            .query_row(
                "SELECT id FROM sites WHERE hostname = ?1",
                params![hostname],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new site
        index.execute(
            "INSERT INTO sites (hostname) VALUES (?1)",
            params![hostname],
        )?;

        Ok(index.last_insert_rowid())
    }

    /// Create a new snapshot for a site
    pub fn create_snapshot(&self, hostname: &str, tree: &Node) -> Result<i64> {
        let site_id = self.get_or_create_site(hostname)?;

        // Serialize tree
        let tree_data =
            rmp_serde::to_vec(tree).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let index = self.index.lock().unwrap();

        // Unset current for all existing snapshots of this site
        index.execute(
            "UPDATE snapshots SET is_current = 0 WHERE site_id = ?1",
            params![site_id],
        )?;

        // Insert new snapshot as current
        index.execute(
            "INSERT INTO snapshots (site_id, tree_data, is_current) VALUES (?1, ?2, 1)",
            params![site_id, tree_data],
        )?;

        Ok(index.last_insert_rowid())
    }

    /// Get the current snapshot for a site
    pub fn get_current_snapshot(&self, hostname: &str) -> Result<Option<(i64, Node)>> {
        let index = self.index.lock().unwrap();

        let result: Option<(i64, Vec<u8>)> = index
            .query_row(
                r#"
                SELECT s.id, s.tree_data
                FROM snapshots s
                JOIN sites si ON s.site_id = si.id
                WHERE si.hostname = ?1 AND s.is_current = 1
                "#,
                params![hostname],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match result {
            Some((id, tree_data)) => {
                let tree: Node = rmp_serde::from_slice(&tree_data)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some((id, tree)))
            }
            None => Ok(None),
        }
    }

    /// List all snapshots for a site
    pub fn list_snapshots(&self, hostname: &str) -> Result<Vec<(i64, bool, String)>> {
        let index = self.index.lock().unwrap();

        let mut stmt = index.prepare(
            r#"
            SELECT s.id, s.is_current, s.created_at
            FROM snapshots s
            JOIN sites si ON s.site_id = si.id
            WHERE si.hostname = ?1
            ORDER BY s.id DESC
            "#,
        )?;

        let snapshots: Vec<(i64, bool, String)> = stmt
            .query_map(params![hostname], |row| {
                Ok((row.get(0)?, row.get::<_, i32>(1)? != 0, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    /// Set a specific snapshot as current
    pub fn set_current_snapshot(&self, hostname: &str, snapshot_id: i64) -> Result<bool> {
        let index = self.index.lock().unwrap();

        // Get site id
        let site_id: Option<i64> = index
            .query_row(
                "SELECT id FROM sites WHERE hostname = ?1",
                params![hostname],
                |row| row.get(0),
            )
            .optional()?;

        let Some(site_id) = site_id else {
            return Ok(false);
        };

        // Check if snapshot exists for this site
        let exists: bool = index
            .query_row(
                "SELECT 1 FROM snapshots WHERE id = ?1 AND site_id = ?2",
                params![snapshot_id, site_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if !exists {
            return Ok(false);
        }

        // Unset current for all snapshots
        index.execute(
            "UPDATE snapshots SET is_current = 0 WHERE site_id = ?1",
            params![site_id],
        )?;

        // Set specified snapshot as current
        index.execute(
            "UPDATE snapshots SET is_current = 1 WHERE id = ?1",
            params![snapshot_id],
        )?;

        Ok(true)
    }
}
