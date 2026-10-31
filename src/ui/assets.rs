//! Embedded assets for GPUI's `svg()`: the line icons in `assets/icons/ui/`
//! (PRD 13.5, Icons). Paths are `icons/ui/<name>.svg`.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[$((
            concat!("icons/ui/", $name, ".svg"),
            include_bytes!(concat!("../../assets/icons/ui/", $name, ".svg")),
        )),*];
    };
}

icons!(
    "back",
    "chev",
    "doc",
    "folder",
    "fwd",
    "grid",
    "inspector",
    "lock",
    "map",
    "rescan",
    "search",
    "sidebar",
    "sparkle",
    "tray",
    "x",
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_file_is_embedded() {
        let files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/ui"))
            .expect("icon folder")
            .count();
        assert_eq!(ICONS.len(), files);
    }
}
