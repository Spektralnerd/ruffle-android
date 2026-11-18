use ruffle_core::backend::storage::StorageBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use std::path::PathBuf;
use url::Url;

/// The parts of SharedObject (saved game) names that Ruffle derives from the movie URL (see
/// `SharedObject.getLocal()` in Ruffle): `{host}/{local path}/{name}`, where the local path is
/// the path of the movie (the default), or a prefix of it given by the movie, often "/".
/// E.g. `example.com` and `games/game.swf` for `https://example.com/games/game.swf?v=2`.
fn shared_object_host_and_path(movie_url: &str) -> Option<(String, String)> {
    let url = Url::parse(movie_url).ok()?;
    let path = url.path();
    let path = path.strip_prefix('/').unwrap_or(path);
    let path = path.strip_suffix('/').unwrap_or(path);
    let host = if url.scheme() == "file" {
        "localhost"
    } else {
        url.host_str().unwrap_or_default()
    };
    Some((host.to_owned(), path.to_owned()))
}

/// How to find the saves a movie made under a URL it had before.
pub struct LegacyNames {
    /// (New, old) name prefix for saves made with the default local path (the movie's path).
    path: (String, String),
    /// (New, old) name prefix for saves made with the local path "/".
    root: (String, String),
}

impl LegacyNames {
    pub fn new(movie_url: &str, old_movie_url: &str) -> Option<Self> {
        let (host, path) = shared_object_host_and_path(movie_url)?;
        let (old_host, old_path) = shared_object_host_and_path(old_movie_url)?;
        Some(Self {
            path: (format!("{host}/{path}/"), format!("{old_host}/{old_path}/")),
            root: (format!("{host}//"), format!("{old_host}//")),
        })
    }

    fn old_name(&self, name: &str) -> Option<String> {
        // (Other local paths would have to be a prefix of both the old and the new path.)
        [&self.path, &self.root].iter().find_map(|names| {
            let (prefix, old_prefix) = *names;
            let rest = name.strip_prefix(prefix.as_str())?;
            Some(format!("{old_prefix}{rest}"))
        })
    }
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
    legacy_names: Option<LegacyNames>,
}

impl AndroidStorageBackend {
    pub fn new(path: PathBuf, legacy_names: Option<LegacyNames>) -> Self {
        Self {
            inner: DiskStorageBackend::new(path),
            legacy_names,
        }
    }

    fn sanitize(name: &str) -> &str {
        // A leading slash (possible for URLs without a host) would make it an absolute path.
        name.trim_start_matches('/')
    }

    fn legacy_name(&self, name: &str) -> Option<String> {
        self.legacy_names.as_ref()?.old_name(name)
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
