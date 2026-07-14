use std::path::PathBuf;

pub struct MemoryManager {
    pub db_path: PathBuf,
}

impl MemoryManager {
    pub fn new() -> Self {
        let db_path = Self::default_db_path();
        Self { db_path }
    }

    pub fn with_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn default_db_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".lai")
                .join("memory.db")
        } else {
            PathBuf::from("memory.db")
        }
    }

    /// Returns alisp code to initialize the memory database.
    /// This opens the DB and creates default tables if they don't exist.
    pub fn init_code(&self) -> String {
        let path = self.db_path.to_string_lossy();
        format!(
            r#"
(do
  ;; Open the memory database
  (sql-open "{path}" "default")

  ;; Create core memory tables
  (sql-execute "
    CREATE TABLE IF NOT EXISTS memories (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      category TEXT NOT NULL DEFAULT 'fact',
      key TEXT NOT NULL,
      value TEXT NOT NULL,
      context TEXT,
      importance INTEGER DEFAULT 5,
      created_at TEXT DEFAULT (datetime('now')),
      accessed_at TEXT DEFAULT (datetime('now')),
      access_count INTEGER DEFAULT 0
    )
  ")

  (sql-execute "
    CREATE TABLE IF NOT EXISTS conversations (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      role TEXT NOT NULL,
      content TEXT NOT NULL,
      topic TEXT,
      timestamp TEXT DEFAULT (datetime('now'))
    )
  ")

  (sql-execute "
    CREATE TABLE IF NOT EXISTS entities (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL UNIQUE,
      entity_type TEXT NOT NULL DEFAULT 'unknown',
      attributes TEXT,
      created_at TEXT DEFAULT (datetime('now')),
      updated_at TEXT DEFAULT (datetime('now'))
    )
  ")

  (sql-execute "
    CREATE TABLE IF NOT EXISTS knowledge (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      domain TEXT NOT NULL DEFAULT 'general',
      topic TEXT NOT NULL,
      fact TEXT NOT NULL,
      source TEXT,
      confidence REAL DEFAULT 1.0,
      created_at TEXT DEFAULT (datetime('now'))
    )
  ")

  ;; Create indexes for fast lookup
  (sql-execute "CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category)")
  (sql-execute "CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)")
  (sql-execute "CREATE INDEX IF NOT EXISTS idx_conversations_topic ON conversations(topic)")
  (sql-execute "CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)")
  (sql-execute "CREATE INDEX IF NOT EXISTS idx_knowledge_domain ON knowledge(domain)")

  (println "memory: database initialized at {path}")
)
"#,
            path = path
        )
    }
}
