#[cfg(unix)]
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct InodeFilter {
    #[cfg(unix)]
    inner: HashMap<(u64, u64), u64>,
}

impl InodeFilter {
    #[cfg(unix)]
    pub fn add(&mut self, metadata: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        self.add_dev_inode((metadata.dev(), metadata.ino()), metadata.nlink())
    }

    #[cfg(windows)]
    pub fn add(&mut self, _metadata: &std::fs::Metadata) -> bool {
        // Windows hardlink detection requires unstable `windows_by_handle` feature.
        // For stable Rust, we count all files (no hardlink deduplication).
        true
    }

    #[cfg(not(any(unix, windows)))]
    pub fn add(&mut self, _metadata: &std::fs::Metadata) -> bool {
        true
    }

    #[cfg(unix)]
    pub fn add_dev_inode(&mut self, dev_inode: (u64, u64), nlinks: u64) -> bool {
        if nlinks <= 1 {
            return true;
        }

        match self.inner.get_mut(&dev_inode) {
            Some(1) => {
                self.inner.remove(&dev_inode);
                false
            }
            Some(count) => {
                *count -= 1;
                false
            }
            None => {
                self.inner.insert(dev_inode, nlinks - 1);
                true
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn it_filters_inodes() {
        let mut inodes = InodeFilter::default();

        assert!(inodes.add_dev_inode((1, 1), 2));
        assert!(inodes.add_dev_inode((2, 1), 2));
        assert!(!inodes.add_dev_inode((1, 1), 2));
        assert!(!inodes.add_dev_inode((2, 1), 2));

        assert!(inodes.add_dev_inode((1, 1), 3));
        assert!(!inodes.add_dev_inode((1, 1), 3));
        assert!(!inodes.add_dev_inode((1, 1), 3));

        assert!(inodes.add_dev_inode((1, 1), 1));
        assert!(inodes.add_dev_inode((1, 1), 1));
    }
}
