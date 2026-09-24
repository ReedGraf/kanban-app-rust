use jiff::{Timestamp, civil::Date};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SortMode {
    #[default]
    Manual,
    Title,
    DueDate,
}

impl SortMode {
    pub const ALL: [SortMode; 3] = [SortMode::Manual, SortMode::Title, SortMode::DueDate];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Manual => "Manual",
            SortMode::Title => "Title",
            SortMode::DueDate => "Due date",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ThemeChoice {
    System,
    #[default]
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Dark, ThemeChoice::Light];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "Follow system",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Card {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due: Option<Date>,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Column {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub sort: SortMode,
    /// Accent color shown as the column's border.
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl Column {
    /// Indices into `cards` in display order, according to the column's sort mode.
    /// Sorting is a view only, so the manual order is never lost.
    pub fn display_order(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.cards.len()).collect();
        match self.sort {
            SortMode::Manual => {}
            SortMode::Title => idx.sort_by_key(|&i| self.cards[i].title.to_lowercase()),
            // Cards without a due date go last.
            SortMode::DueDate => idx.sort_by_key(|&i| (self.cards[i].due.is_none(), self.cards[i].due)),
        }
        idx
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Archived {
    pub card: Card,
    pub column: String,
    pub archived_at: Timestamp,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Board {
    #[serde(default)]
    pub theme: ThemeChoice,
    pub next_id: u64,
    pub columns: Vec<Column>,
    #[serde(default)]
    pub archive: Vec<Archived>,
}

impl Default for Board {
    fn default() -> Self {
        let mut b = Board { theme: ThemeChoice::Dark, next_id: 1, columns: vec![], archive: vec![] };
        for t in ["To do", "In progress", "Done"] {
            b.add_column(t);
        }
        b
    }
}

impl Board {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id - 1
    }

    pub fn add_column(&mut self, title: &str) {
        let id = self.new_id();
        self.columns.push(Column { id, title: title.into(), sort: SortMode::Manual, color: None, cards: vec![] });
    }

    /// Returns (column index, card index) of a card.
    pub fn find(&self, card_id: u64) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(ci, c)| {
            c.cards.iter().position(|k| k.id == card_id).map(|i| (ci, i))
        })
    }

    pub fn take(&mut self, card_id: u64) -> Option<(usize, Card)> {
        let (ci, i) = self.find(card_id)?;
        Some((ci, self.columns[ci].cards.remove(i)))
    }

    pub fn archive_card(&mut self, card_id: u64) {
        if let Some((ci, card)) = self.take(card_id) {
            let column = self.columns[ci].title.clone();
            self.archive.insert(0, Archived { card, column, archived_at: Timestamp::now() });
        }
    }

    /// Put an archived card back into the column it came from (or the first column).
    pub fn restore(&mut self, archive_idx: usize) {
        if self.columns.is_empty() {
            self.add_column("To do");
        }
        let a = self.archive.remove(archive_idx);
        let ci = self.columns.iter().position(|c| c.title == a.column).unwrap_or(0);
        self.columns[ci].cards.push(a.card);
    }

    pub fn path() -> PathBuf {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("kanban").join("board.json")
    }

    pub fn load() -> Board {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                // Don't clobber a file we failed to parse; keep a copy next to it.
                eprintln!("kanban: could not parse {}: {e}", path.display());
                let _ = std::fs::copy(&path, path.with_extension("json.bak"));
                Board::default()
            }),
            Err(_) => Board::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            // Write to a temp file then rename, so a crash never leaves a half-written board.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
            std::fs::rename(tmp, &path)
        };
        if let Err(e) = write() {
            eprintln!("kanban: failed to save {}: {e}", path.display());
        }
    }
}
