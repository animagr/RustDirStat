use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsOpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Trash error: {0}")]
    Trash(#[from] trash::Error),
}

pub fn trash_path(path: &Path) -> Result<(), FsOpError> {
    trash::delete(path)?;
    Ok(())
}

pub fn delete_recursive(path: &Path) -> Result<usize, FsOpError> {
    let mut count = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            count += delete_recursive(&entry.path())?;
        }
        std::fs::remove_dir(path)?;
    } else {
        std::fs::remove_file(path)?;
        count = 1;
    }
    Ok(count)
}
