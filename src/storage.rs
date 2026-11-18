use ruffle_core::backend::storage::StorageBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use std::path::PathBuf;
use url::Url;

/// Where Ruffle puts the SharedObjects (saved games) of a movie: the part of their names that
/// Ruffle derives from the movie URL (see `SharedObject.getLocal()` in Ruffle), e.g.
/// `example.com/games/game.swf/` for `https://example.com/games/game.swf?v=2`.
pub fn shared_object_prefix(movie_url: &str) -> Option<String> {
    let url = Url::parse(movie_url).ok()?;
    let path = url.path();
    let path = path.strip_prefix('/').unwrap_or(path);
    let path = path.strip_suffix('/').unwrap_or(path);
    let host = if url.scheme() == "file" {
        "localhost"
    } else {
        url.host_str().unwrap_or_default()
    };
    Some(format!("{host}/{path}/"))
}

/// SharedObject storage in the app's private files directory.
///
/// Wraps `DiskStorageBackend` (which uses the SharedObject names as relative file paths) with:
/// - names that can never point outside of the storage directory,
/// - a fallback to the names a movie's saves had before it got its current URL, so saves made
///   by older versions of the app (which used the changing `content://` URL of the file) are
///   still found.
pub struct AndroidStorageBackend {
    inner: DiskStorageBackend,
    /// Name prefix of the current movie, and the prefix its saves used to have.
    legacy_prefix: Option<(String, String)>,
}

impl AndroidStorageBackend {
    pub fn new(path: PathBuf, legacy_prefix: Option<(String, String)>) -> Self {
        Self {
            inner: DiskStorageBackend::new(path),
            legacy_prefix,
        }
    }

    fn sanitize(name: &str) -> &str {
        // A leading slash (possible for URLs without a host) would make it an absolute path.
        name.trim_start_matches('/')
    }

    fn legacy_name(&self, name: &str) -> Option<String> {
        let (prefix, legacy_prefix) = self.legacy_prefix.as_ref()?;
        let rest = name.strip_prefix(prefix.as_str())?;
        Some(format!("{legacy_prefix}{rest}"))
    }
}

impl StorageBackend for AndroidStorageBackend {
    fn get(&self, name: &str) -> Option<Vec<u8>> {
        let name = Self::sanitize(name);
        self.inner.get(name).or_else(|| {
            let legacy_name = self.legacy_name(name)?;
            self.inner.get(Self::sanitize(&legacy_name))
        })
    }

    fn put(&mut self, name: &str, value: &[u8]) -> bool {
        self.inner.put(Self::sanitize(name), value)
    }

    fn remove_key(&mut self, name: &str) {
        let name = Self::sanitize(name);
        if let Some(legacy_name) = self.legacy_name(name) {
            self.inner.remove_key(Self::sanitize(&legacy_name));
        }
        self.inner.remove_key(name);
    }
}
