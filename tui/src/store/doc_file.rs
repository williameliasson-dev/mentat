use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use loro::{ExportMode, LoroDoc, VersionVector};

use crate::Result;

const SNAPSHOT: &str = "snapshot.loro";
const LOG: &str = "updates.log";

/// Log entries before the log is folded into a new snapshot, which keeps
/// opening a document from replaying an ever-growing log.
const COMPACT_AFTER: usize = 256;

/// One Loro document on disk, as a directory holding:
///
/// - `snapshot.loro`: the whole document as of the last compaction.
/// - `updates.log`: every save since, each as a little-endian `u32` length
///   followed by that many bytes of Loro update.
///
/// A save only ever appends to the log, and a compaction swaps in the new
/// snapshot by rename, so a crash at any point leaves a document that opens:
/// at worst minus a half-written last entry, which opening drops.
pub(crate) struct DocFile {
    pub doc: LoroDoc,
    /// `None` keeps the document in memory only, for tests.
    dir: Option<PathBuf>,
    /// The version already on disk; a save writes everything past it.
    saved: VersionVector,
    log_entries: usize,
}

impl DocFile {
    pub fn in_memory() -> Self {
        let doc = LoroDoc::new();
        Self {
            saved: doc.oplog_vv(),
            doc,
            dir: None,
            log_entries: 0,
        }
    }

    /// Opens the document in `dir`, creating an empty one if there is none.
    pub fn open(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        let doc = LoroDoc::new();
        if let Some(snapshot) = read_if_exists(&dir.join(SNAPSHOT))? {
            doc.import(&snapshot)?;
        }

        let log = read_if_exists(&dir.join(LOG))?.unwrap_or_default();
        let (entries, intact) = parse_log(&log);
        for entry in &entries {
            doc.import(entry)?;
        }
        if intact < log.len() {
            // A crash mid-append: drop the torn tail so the next append
            // doesn't land behind it.
            OpenOptions::new()
                .write(true)
                .open(dir.join(LOG))?
                .set_len(intact as u64)?;
        }

        Ok(Self {
            saved: doc.oplog_vv(),
            log_entries: entries.len(),
            doc,
            dir: Some(dir),
        })
    }

    /// Commits pending changes and appends them to the log.
    pub fn save(&mut self) -> Result<()> {
        self.doc.commit();
        if self.doc.oplog_vv() == self.saved {
            return Ok(());
        }
        if let Some(dir) = &self.dir {
            let update = self.doc.export(ExportMode::updates(&self.saved))?;
            let mut log = OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(LOG))?;
            log.write_all(&(update.len() as u32).to_le_bytes())?;
            log.write_all(&update)?;
            log.sync_data()?;
            self.log_entries += 1;
        }
        self.saved = self.doc.oplog_vv();

        if self.log_entries >= COMPACT_AFTER {
            self.compact()?;
        }
        Ok(())
    }

    /// Deletes the document from disk.
    pub fn remove(self) -> Result<()> {
        match self.dir.map(fs::remove_dir_all) {
            Some(Err(e)) if e.kind() != ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// Folds the log into a fresh snapshot.
    ///
    /// If this dies between the rename and the truncate, the next open
    /// replays log entries the snapshot already holds, which Loro ignores.
    fn compact(&mut self) -> Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        let temp = dir.join(format!("{SNAPSHOT}.tmp"));
        let mut file = File::create(&temp)?;
        file.write_all(&self.doc.export(ExportMode::Snapshot)?)?;
        file.sync_all()?;
        fs::rename(&temp, dir.join(SNAPSHOT))?;
        // The rename must be durable before the log it replaces is emptied.
        File::open(dir)?.sync_all()?;

        File::create(dir.join(LOG))?.sync_all()?;
        self.log_entries = 0;
        Ok(())
    }
}

fn read_if_exists(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The complete entries in `log`, and how many bytes they span.
fn parse_log(log: &[u8]) -> (Vec<&[u8]>, usize) {
    let mut entries = Vec::new();
    let mut rest = log;
    while let Some((len, after)) = rest.split_first_chunk::<4>() {
        let len = u32::from_le_bytes(*len) as usize;
        let Some((entry, after)) = after.split_at_checked(len) else {
            break;
        };
        entries.push(entry);
        rest = after;
    }
    (entries, log.len() - rest.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mentat-doc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(file: &mut DocFile, text: &str) {
        let body = file.doc.get_text("body");
        body.insert(body.len_unicode(), text).unwrap();
        file.save().unwrap();
    }

    fn read(dir: &Path) -> String {
        DocFile::open(dir.to_path_buf())
            .unwrap()
            .doc
            .get_text("body")
            .to_string()
    }

    #[test]
    fn saves_survive_reopening() {
        let dir = temp_dir();
        let mut file = DocFile::open(dir.clone()).unwrap();
        write(&mut file, "hello ");
        write(&mut file, "world");
        assert_eq!(read(&dir), "hello world");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn compaction_keeps_everything_and_empties_the_log() {
        let dir = temp_dir();
        let mut file = DocFile::open(dir.clone()).unwrap();
        for _ in 0..COMPACT_AFTER {
            write(&mut file, "x");
        }
        assert_eq!(fs::metadata(dir.join(LOG)).unwrap().len(), 0);
        write(&mut file, "y");
        assert_eq!(read(&dir), format!("{}y", "x".repeat(COMPACT_AFTER)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_torn_last_entry_is_dropped() {
        let dir = temp_dir();
        let mut file = DocFile::open(dir.clone()).unwrap();
        write(&mut file, "kept");
        // Half an entry: a length promising more bytes than follow.
        let mut log = OpenOptions::new().append(true).open(dir.join(LOG)).unwrap();
        log.write_all(&100u32.to_le_bytes()).unwrap();
        log.write_all(b"torn").unwrap();
        drop(log);

        let mut reopened = DocFile::open(dir.clone()).unwrap();
        assert_eq!(reopened.doc.get_text("body").to_string(), "kept");
        // And the next save lands where the torn entry was, not behind it.
        write(&mut reopened, " more");
        assert_eq!(read(&dir), "kept more");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_nothing_writes_nothing() {
        let dir = temp_dir();
        let mut file = DocFile::open(dir.clone()).unwrap();
        file.save().unwrap();
        assert!(!dir.join(LOG).exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
