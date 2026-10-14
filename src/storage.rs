use crate::{model::Layout, settings::Settings};
use serde_json::Value;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(crate::model::home()).join(".config"))
        .join("termlook")
}
pub fn read(path: &Path) -> io::Result<Value> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > 4 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Configuration exceeds 4 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
pub fn load_layout(path: &Path) -> Layout {
    read(path)
        .map(|v| Layout::from_data(&v))
        .unwrap_or_default()
}
pub fn load_settings(path: &Path) -> Settings {
    read(path)
        .map(|v| Settings::from_data(&v))
        .unwrap_or_default()
}
/// Same-directory atomic replacement; private permissions and fsync before rename.
pub fn save(path: &Path, value: &Value) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()
}
