use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use zip::ZipWriter;
use zip::result::ZipResult;
use zip::write::{FileOptionExtension, FileOptions};
use zip_extensions::default_entry_handler::DefaultEntryHandler;
use zip_extensions::entry_handler::EntryHandler;

const IGNORE_FILENAMES: &[&str] = &[".gitignore", ".zipignore"];
const ALWAYS_IGNORE: &[&str] = &[".venv/", ".git/"];

/// EntryHandler that honors `.gitignore` and `.zipignore` files (the latter takes precedence
/// when both exist in the same directory) and always ignores `.venv/` and `.git/`.
pub struct IgnoreEntryHandler<H = DefaultEntryHandler> {
    matcher_cache: Mutex<HashMap<PathBuf, Gitignore>>,
    inner: H,
}

impl IgnoreEntryHandler<DefaultEntryHandler> {
    pub fn new() -> Self {
        Self {
            matcher_cache: Mutex::new(HashMap::new()),
            inner: DefaultEntryHandler,
        }
    }
}

impl<H> IgnoreEntryHandler<H> {
    fn build_matcher(&self, root: &Path, dir: &Path) -> io::Result<Gitignore> {
        let mut builder = GitignoreBuilder::new(root);

        for pat in ALWAYS_IGNORE {
            let _ = builder.add_line(None, pat);
        }

        let mut stack: Vec<PathBuf> = Vec::new();
        let mut cur = dir;
        loop {
            stack.push(cur.to_path_buf());
            if cur == root {
                break;
            }
            match cur.parent() {
                Some(p) => cur = p,
                None => break,
            }
        }
        stack.reverse();

        for d in stack {
            for name in IGNORE_FILENAMES {
                let f = d.join(name);
                if f.exists() {
                    let _ = builder.add(f);
                }
            }
        }

        builder
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    }

    fn matcher_for_dir(&self, root: &Path, dir: &Path) -> io::Result<Gitignore> {
        let mut cache = self.matcher_cache.lock().unwrap();
        if let Some(m) = cache.get(dir) {
            return Ok(m.clone());
        }
        let m = self.build_matcher(root, dir)?;
        cache.insert(dir.to_path_buf(), m.clone());
        Ok(m)
    }

    fn is_ignored(&self, root: &Path, path: &Path, is_dir: bool) -> bool {
        let dir = if is_dir {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        match self.matcher_for_dir(root, dir) {
            Ok(m) => m.matched_path_or_any_parents(path, is_dir).is_ignore(),
            Err(_) => false,
        }
    }
}

impl<T: FileOptionExtension, H> EntryHandler<T> for IgnoreEntryHandler<H>
where
    H: EntryHandler<T>,
{
    fn handle_entry<W: Write + io::Seek>(
        &self,
        writer: &mut ZipWriter<W>,
        root: &PathBuf,
        entry_path: &PathBuf,
        file_options: FileOptions<T>,
        buffer: &mut Vec<u8>,
    ) -> ZipResult<()> {
        let metadata = std::fs::metadata(entry_path)?;
        let is_dir = metadata.is_dir();
        if self.is_ignored(root.as_path(), entry_path.as_path(), is_dir) {
            return Ok(());
        }
        self.inner
            .handle_entry(writer, root, entry_path, file_options, buffer)
    }
}
