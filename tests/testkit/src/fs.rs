use std::fs;
use std::path::Path;

pub trait PathExt {
    fn rm(&self);
    fn rm_rf(&self);
    fn rm_contents(&self);
    fn ensure_empty(&self);
    fn mkdir_p(&self);
}

impl PathExt for Path {
    // delete a file, returning silently if it doesn't exist
    fn rm(&self) {
        match fs::remove_file(self) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                // On Windows, readonly files need permission fixup before
                // removal. Git often clones files as readonly.
                #[cfg(windows)]
                {
                    let mut p = ok_or_panic!(self.metadata()).permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    p.set_readonly(false);
                    ok_or_panic! { fs::set_permissions(self, p) };
                    fs::remove_file(self).unwrap_or_else(|e| {
                        panic!("failed to remove file {}: {}", self.display(), e)
                    });
                }
                #[cfg(not(windows))]
                {
                    panic!("failed to remove file {}: {}", self.display(), e);
                }
            }
        }
    }

    /* Technically there is a potential race condition, but we don't
     * care all that much for our tests
     */
    fn rm_rf(&self) {
        match remove_dir_all::remove_dir_all(self) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => panic!("failed to remove dir {}: {}", self.display(), e),
        }
    }

    // remove directory contents but not the directory itself
    fn rm_contents(&self) {
        remove_dir_all::remove_dir_contents(self)
            .unwrap_or_else(|e| panic!("failed to remove dir contents {}: {}", self.display(), e));
    }

    // ensure the directory is created and empty
    fn ensure_empty(&self) {
        self.mkdir_p();
        self.rm_contents();
    }

    // create all paths up to the input path
    fn mkdir_p(&self) {
        fs::create_dir_all(self)
            .unwrap_or_else(|e| panic!("failed to mkdir_p {}: {}", self.display(), e))
    }
}
