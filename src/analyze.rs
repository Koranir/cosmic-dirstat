use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::OsStr,
    fs::{self, Metadata},
    hash::{Hash, Hasher},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub discovered: u64,
    pub scanned: u64,
    pub current_path: PathBuf,
    pub percentage: f64,
}

#[derive(Default)]
pub struct Context {
    seen_files: RefCell<HashSet<FileId>>,
    errors: RefCell<Vec<ScanError>>,
    progress: RefCell<ProgressReporter>,
    cancelled: Arc<AtomicBool>,
}

impl Context {
    pub fn with_progress(
        cancelled: Arc<AtomicBool>,
        on_progress: impl FnMut(ScanProgress) + Send + 'static,
    ) -> Self {
        Self {
            cancelled,
            progress: RefCell::new(ProgressReporter::new(Some(Box::new(on_progress)))),
            ..Self::default()
        }
    }

    pub fn errors(&self) -> Vec<ScanError> {
        self.errors.borrow().clone()
    }

    fn record_error(&self, path: PathBuf, error: impl ToString) {
        self.errors.borrow_mut().push(ScanError {
            path,
            message: error.to_string(),
        });
    }

    fn count_allocated_size_once(&self, metadata: &Metadata) -> u64 {
        if metadata.nlink() <= 1 {
            return allocated_size(metadata);
        }

        let file_id = FileId::from(metadata);
        if self.seen_files.borrow_mut().insert(file_id) {
            allocated_size(metadata)
        } else {
            0
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn check_cancelled(&self) -> std::io::Result<()> {
        if self.is_cancelled() {
            Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "scan cancelled",
            ))
        } else {
            Ok(())
        }
    }

    fn discovered(&self, path: &Path) {
        self.progress.borrow_mut().discovered(path);
    }

    fn scanned(&self, path: &Path) {
        self.progress.borrow_mut().scanned(path);
    }

    fn step_percentage(&self, percentage_by: f64) {
        self.progress.borrow_mut().step_percentage(percentage_by);
    }
}

type ProgressCallback = Box<dyn FnMut(ScanProgress) + Send>;

#[derive(Default)]
struct ProgressReporter {
    discovered: u64,
    scanned: u64,
    percentage: f64,
    events_since_emit: u64,
    on_progress: Option<ProgressCallback>,
}

impl ProgressReporter {
    fn new(on_progress: Option<ProgressCallback>) -> Self {
        Self {
            on_progress,
            ..Self::default()
        }
    }

    fn discovered(&mut self, path: &Path) {
        self.discovered += 1;
        self.emit_throttled(path);
    }

    fn scanned(&mut self, path: &Path) {
        self.scanned += 1;
        self.emit_throttled(path);
    }

    fn emit_throttled(&mut self, path: &Path) {
        self.events_since_emit += 1;

        if self.events_since_emit >= 128 || self.scanned >= self.discovered {
            self.events_since_emit = 0;
            self.emit(path);
        }
    }

    fn step_percentage(&mut self, percentage_by: f64) {
        self.percentage += percentage_by;
    }

    fn emit(&mut self, path: &Path) {
        let Some(on_progress) = &mut self.on_progress else {
            return;
        };

        on_progress(ScanProgress {
            discovered: self.discovered,
            scanned: self.scanned,
            current_path: path.to_owned(),
            percentage: self.percentage,
        });
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedDir {
    pub children: Vec<AnalyzedItem>,
    pub path: PathBuf,
    pub size: u64,
    pub self_size: u64,
    pub num_symlinks: u64,
    pub num_files: u64,
    pub num_dirs: u64,
}

#[derive(Debug, Clone)]
pub struct AnalyzedFile {
    pub hardlink_count: u64,
    pub size: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AnalyzedSymlink {
    pub hardlink_count: u64,
    pub size: u64,
    pub path: PathBuf,
    pub link: PathBuf,
}

#[derive(Debug, Clone)]
pub enum AnalyzedItem {
    Dir(AnalyzedDir),
    File(AnalyzedFile),
    Symlink(AnalyzedSymlink),
}

impl AnalyzedItem {
    pub const fn size(&self) -> u64 {
        match self {
            Self::Dir(d) => d.size,
            Self::File(f) => f.size,
            Self::Symlink(s) => s.size,
        }
    }

    pub fn name(&self) -> Option<&OsStr> {
        match self {
            Self::Dir(d) => d.path.file_name(),
            Self::File(f) => f.path.file_name(),
            Self::Symlink(s) => s.path.file_name(),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Dir(d) => &d.path,
            Self::File(f) => &f.path,
            Self::Symlink(s) => &s.path,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq)]
struct FileId {
    dev: u64,
    ino: u64,
}

impl From<&Metadata> for FileId {
    fn from(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

impl PartialEq for FileId {
    fn eq(&self, other: &Self) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }
}

impl Hash for FileId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dev.hash(state);
        self.ino.hash(state);
    }
}

pub fn analyze_dir(dir: &Path, ctx: &Context) -> std::io::Result<AnalyzedDir> {
    ctx.discovered(dir);
    let result = Scanner { ctx }.scan_dir(dir, 100.0);
    ctx.scanned(dir);
    result
}

struct Scanner<'a> {
    ctx: &'a Context,
}

impl Scanner<'_> {
    fn scan_dir(&self, dir: &Path, percentage_budget: f64) -> std::io::Result<AnalyzedDir> {
        self.ctx.check_cancelled()?;

        let metadata = fs::symlink_metadata(dir)?;
        let self_size = allocated_size(&metadata);

        let mut child_dirs = 0;
        let entries = fs::read_dir(dir)?
            .filter_map(|entry| match entry {
                Ok(entry) => {
                    let path = entry.path();
                    self.ctx.discovered(&path);

                    match entry.file_type() {
                        Ok(file_type) => {
                            if file_type.is_dir() {
                                child_dirs += 1;
                            }
                            Some((file_type, path))
                        }
                        Err(error) => {
                            self.ctx.record_error(path, error);
                            self.ctx.scanned(dir);

                            None
                        }
                    }
                }
                Err(error) => {
                    self.ctx.record_error(dir.to_owned(), error);
                    None
                }
            })
            .collect::<Vec<_>>();

        let (dir_budget, file_budget) = if entries.is_empty() {
            self.ctx.step_percentage(percentage_budget);
            (0.0, 0.0)
        } else {
            let child_files = entries.len() - child_dirs;

            if child_dirs == 0 {
                (0.0, percentage_budget / child_files as f64)
            } else if child_files == 0 {
                (percentage_budget / child_dirs as f64, 0.0)
            } else {
                let dir_influence = child_dirs as f64 * 20.0;
                let dir_ratio = dir_influence / (dir_influence + child_files as f64);
                let dir_budget = percentage_budget * dir_ratio;
                let file_budget = percentage_budget - dir_budget;
                (
                    dir_budget / child_dirs as f64,
                    file_budget / child_files as f64,
                )
            }
        };

        let mut children = Vec::new();
        let mut num_symlinks = 0;
        let mut num_files = 0;
        let mut num_dirs = 0;
        for (file_type, path) in entries {
            self.ctx.check_cancelled()?;

            if file_type.is_dir() {
                let analyzed = match self.scan_dir(&path, dir_budget) {
                    Ok(analyzed) => analyzed,
                    Err(error) => {
                        self.ctx.record_error(path, error);
                        self.ctx.scanned(dir);
                        continue;
                    }
                };

                num_symlinks += analyzed.num_symlinks;
                num_dirs += analyzed.num_dirs + 1;
                num_files += analyzed.num_files;
                children.push(AnalyzedItem::Dir(analyzed));
                self.ctx.scanned(&path);
                continue;
            }

            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    self.ctx.record_error(path, error);
                    self.ctx.scanned(dir);
                    continue;
                }
            };
            let hardlink_count = metadata.nlink();
            let size = self.ctx.count_allocated_size_once(&metadata);

            if file_type.is_symlink() {
                let link = match fs::read_link(&path) {
                    Ok(link) => link,
                    Err(error) => {
                        self.ctx.record_error(path, error);
                        self.ctx.scanned(dir);
                        continue;
                    }
                };

                num_symlinks += 1;
                self.ctx.scanned(&path);
                children.push(AnalyzedItem::Symlink(AnalyzedSymlink {
                    hardlink_count,
                    size,
                    path,
                    link,
                }));
            } else {
                num_files += 1;
                self.ctx.scanned(&path);
                children.push(AnalyzedItem::File(AnalyzedFile {
                    hardlink_count,
                    size,
                    path,
                }));
            }

            self.ctx.step_percentage(file_budget);
        }

        children.sort_unstable_by_key(|item| std::cmp::Reverse(item.size()));

        let children_size = children.iter().map(AnalyzedItem::size).sum::<u64>();
        let size = self_size + children_size;

        Ok(AnalyzedDir {
            children,
            size,
            self_size,
            path: dir.to_owned(),
            num_symlinks,
            num_files,
            num_dirs,
        })
    }
}

fn allocated_size(metadata: &Metadata) -> u64 {
    metadata.blocks().saturating_mul(512)
}

pub struct PartitionElement<'a> {
    pub placement: treemap::Rect,
    pub size: u64,
    pub item: Option<&'a AnalyzedItem>,
}

impl treemap::Mappable for PartitionElement<'_> {
    fn size(&self) -> f64 {
        self.size as f64
    }

    fn bounds(&self) -> &treemap::Rect {
        &self.placement
    }

    fn set_bounds(&mut self, bounds: treemap::Rect) {
        self.placement = bounds;
    }
}

pub fn partition(space: (f64, f64), min: f64, dir: &AnalyzedDir) -> Vec<PartitionElement<'_>> {
    if dir.size == 0 || space.0 <= 0.0 || space.1 <= 0.0 {
        return Vec::new();
    }

    let scale = dir.size as f64 / (space.0 * space.1);
    let min_area = (min * scale) as u64;
    let end_index = dir
        .children
        .iter()
        .enumerate()
        .find(|(_, item)| item.size() < min_area)
        .map(|(index, _)| index);

    let visible_children = end_index.unwrap_or(dir.children.len());
    let mut items = Vec::with_capacity(visible_children + usize::from(end_index.is_some()));
    let mut accum = 0;

    for item in &dir.children[..visible_children] {
        items.push(PartitionElement {
            placement: treemap::Rect::default(),
            size: item.size(),
            item: Some(item),
        });
        accum += item.size();
    }

    let remainder = dir.size.saturating_sub(accum);
    if remainder > 0 && (end_index.is_some() || dir.self_size > 0) {
        items.push(PartitionElement {
            placement: treemap::Rect::default(),
            size: remainder,
            item: None,
        });
    }

    let layout = treemap::TreemapLayout::new();
    let bounds = treemap::Rect::from_points(0.0, 0.0, space.0, space.1);
    layout.layout_items(&mut items, bounds);

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardlinks_are_counted_once() {
        let root = std::env::temp_dir().join(format!("cosmic-dirstat-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();

        let first = root.join("first");
        let second = root.join("second");
        fs::write(&first, b"hello").unwrap();
        fs::hard_link(&first, &second).unwrap();

        let context = Context::default();
        let analyzed = analyze_dir(&root, &context).unwrap();
        let file_sizes = analyzed
            .children
            .iter()
            .filter_map(|item| match item {
                AnalyzedItem::File(file) => Some(file.size),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(file_sizes.len(), 2);
        assert_eq!(file_sizes.iter().filter(|size| **size > 0).count(), 1);

        fs::remove_dir_all(root).unwrap();
    }
}
