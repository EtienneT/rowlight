//! Finder and desktop open events use file URLs, including escaped filenames.

use std::path::PathBuf;

pub fn file_from_url(value: &str) -> Option<PathBuf> {
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_events_decode_spaces_unicode_and_percent_signs() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("résultats 100%.parquet");
        let url = url::Url::from_file_path(&path).unwrap();
        assert_eq!(file_from_url(url.as_str()), Some(path));
        assert!(file_from_url("https://example.com/file.parquet").is_none());
        assert!(file_from_url("not a URL").is_none());
    }
}
