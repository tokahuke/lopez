use std::path::Path;
use std::{fs, io};

pub fn prepare_fs<P: AsRef<Path>>(base: P) -> Result<(), io::Error> {
    const STRUCTURE: &[&str] = &["logs", "runs", "failed"];

    fs::create_dir_all(base.as_ref())?;

    for dir_path in STRUCTURE {
        fs::create_dir_all(base.as_ref().join(dir_path))?;
    }

    Ok(())
}
