use std::{
    ffi::OsStr,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Context {}

#[derive(Debug, Clone)]
pub struct AnalyzedDir {
    pub children: Vec<AnalyzedItem>,
    pub path: PathBuf,
    pub size: u64,
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

pub fn analyze_dir(dir: &Path, _ctx: &Context) -> std::io::Result<AnalyzedDir> {
    let entries = std::fs::read_dir(dir)?;
    let mut children = Vec::new();
    let mut num_symlinks = 0;
    let mut num_files = 0;
    let mut num_dirs = 0;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Error: {e}");
                continue;
            }
        };
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error: {e}");
                continue;
            }
        };
        let path = entry.path();

        if metadata.is_dir() {
            let analyzed = match analyze_dir(&path, _ctx) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Error: {e}");
                    continue;
                }
            };
            num_symlinks += analyzed.num_symlinks;
            num_dirs += analyzed.num_dirs + 1;
            num_files += analyzed.num_files;
            children.push(AnalyzedItem::Dir(analyzed));
        } else {
            // let name = entry.file_name();
            let hardlink_count = metadata.nlink();
            let size = metadata.blocks() * 512 / hardlink_count;
            num_files += 1;

            if metadata.is_symlink() {
                let link = match std::fs::read_link(&path) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        continue;
                    }
                };
                num_symlinks += 1;

                children.push(AnalyzedItem::Symlink(AnalyzedSymlink {
                    hardlink_count,
                    size,
                    path,
                    link,
                }));
            } else {
                children.push(AnalyzedItem::File(AnalyzedFile {
                    hardlink_count,
                    size,
                    path,
                }));
            }
        }
    }

    children.sort_unstable_by_key(|b| std::cmp::Reverse(b.size()));

    let size: u64 = children.iter().map(AnalyzedItem::size).sum();

    Ok(AnalyzedDir {
        children,
        size,
        path: dir.to_owned(),
        num_symlinks,
        num_files,
        num_dirs,
    })
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

pub fn partition(space: (f64, f64), min: f64, dir: &AnalyzedDir) -> Vec<PartitionElement> {
    let scale = dir.size as f64 / (space.0 * space.1);
    let min_area = (min * scale) as u64;
    let end_index = dir
        .children
        .iter()
        .enumerate()
        .find(|f| f.1.size() < min_area)
        .map(|f| f.0);

    let mut items = Vec::with_capacity(end_index.map_or(dir.children.len(), |f| f + 2));
    let mut accum = 0;
    for ele in &dir.children[0..end_index.unwrap_or(dir.children.len())] {
        items.push(PartitionElement {
            placement: treemap::Rect::default(),
            size: ele.size(),
            item: Some(ele),
        });
        accum += ele.size();
    }
    if end_index.is_some() {
        items.push(PartitionElement {
            placement: treemap::Rect::default(),
            size: dir.size - accum,
            item: None,
        });
    }

    let layout = treemap::TreemapLayout::new();
    // let aspect = space.0 / space.1;
    // let height = (sz / aspect).sqrt();
    let bounds = treemap::Rect::from_points(0.0, 0.0, space.0, space.1);
    layout.layout_items(&mut items, bounds);

    items
}
