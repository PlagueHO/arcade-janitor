use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{ArcadeJanitorError, Result, errors::io_error, parsers::catver::parse_catver_str};

pub const CATVER_DOWNLOAD_URL: &str =
    "https://raw.githubusercontent.com/AntoPISA/MAME_SupportFiles/main/catver.ini/catver.ini";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataSourceTarget {
    Mame,
    Catver,
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedCachePaths {
    pub mame_xml: PathBuf,
    pub catver: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MameXmlSource {
    ExplicitPath,
    ExtractedFrom(PathBuf),
    Cache,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMameXml {
    pub path: PathBuf,
    pub source: MameXmlSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatverSource {
    ExplicitPath,
    Cache,
    Downloaded(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCatver {
    pub path: PathBuf,
    pub source: CatverSource,
}

pub fn resolve_mame_xml_path(
    explicit_path: Option<&Path>,
    executable: Option<&Path>,
) -> Result<PathBuf> {
    Ok(resolve_mame_xml(explicit_path, executable)?.path)
}

pub fn resolve_mame_xml(
    explicit_path: Option<&Path>,
    executable: Option<&Path>,
) -> Result<ResolvedMameXml> {
    if let Some(path) = explicit_path {
        return Ok(ResolvedMameXml {
            path: path.to_path_buf(),
            source: MameXmlSource::ExplicitPath,
        });
    }

    let cache_path = managed_cache_paths()?.mame_xml;
    if let Some(executable) = executable {
        extract_mame_xml(executable, &cache_path)?;
        Ok(ResolvedMameXml {
            path: cache_path,
            source: MameXmlSource::ExtractedFrom(executable.to_path_buf()),
        })
    } else {
        Ok(ResolvedMameXml {
            path: resolve_mame_xml_path_in_cache(&cache_path)?,
            source: MameXmlSource::Cache,
        })
    }
}

pub fn refresh_mame_xml(executable: &Path) -> Result<ResolvedMameXml> {
    let paths = managed_cache_paths()?;
    extract_mame_xml(executable, &paths.mame_xml)?;
    Ok(ResolvedMameXml {
        path: paths.mame_xml,
        source: MameXmlSource::ExtractedFrom(executable.to_path_buf()),
    })
}

fn resolve_mame_xml_path_in_cache(cached_path: &Path) -> Result<PathBuf> {
    if cached_path.is_file() {
        return Ok(cached_path.to_path_buf());
    }

    Err(ArcadeJanitorError::MameXmlUnavailable)
}

fn extract_mame_xml(executable: &Path, cached_path: &Path) -> Result<()> {
    let output = Command::new(executable)
        .arg("-listxml")
        .output()
        .map_err(|source| io_error(executable, source))?;
    if !output.status.success() {
        return Err(ArcadeJanitorError::MameExecution {
            executable: executable.to_path_buf(),
            status: output.status.to_string(),
        });
    }
    let xml = String::from_utf8(output.stdout).map_err(ArcadeJanitorError::MameEncoding)?;
    if crate::parsers::mame_xml::parse_mame_xml_str(&xml)?.is_empty() {
        return Err(ArcadeJanitorError::Xml(
            "MAME -listxml output did not contain any machine entries".to_string(),
        ));
    }
    cache_mame_xml(cached_path, &xml)?;
    Ok(())
}

fn cache_mame_xml(cached_path: &Path, content: &str) -> Result<()> {
    let parent = cached_path
        .parent()
        .ok_or(ArcadeJanitorError::CacheDirectoryUnavailable)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let temporary_path = cached_path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, content).map_err(|source| io_error(&temporary_path, source))?;
    replace_cached_file(&temporary_path, cached_path)
}

pub fn resolve_catver_path(explicit_path: Option<&Path>) -> Result<PathBuf> {
    Ok(resolve_catver(explicit_path)?.path)
}

pub fn resolve_catver(explicit_path: Option<&Path>) -> Result<ResolvedCatver> {
    resolve_catver_in_cache(explicit_path, &managed_cache_paths()?.catver)
}

pub fn refresh_catver() -> Result<ResolvedCatver> {
    let path = managed_cache_paths()?.catver;
    download_catver(&path)?;
    Ok(ResolvedCatver {
        path,
        source: CatverSource::Downloaded(CATVER_DOWNLOAD_URL.to_string()),
    })
}

pub fn managed_cache_paths() -> Result<ManagedCachePaths> {
    let directory = match std::env::var_os("ARCADEJANITOR_CACHE_DIR") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => dirs_next::cache_dir()
            .ok_or(ArcadeJanitorError::CacheDirectoryUnavailable)?
            .join("arcadejanitor"),
    };
    Ok(ManagedCachePaths {
        mame_xml: directory.join("mame.xml"),
        catver: directory.join("catver.ini"),
    })
}

pub fn plan_clear_managed_cache(target: MetadataSourceTarget) -> Result<Vec<CacheEntry>> {
    let paths = managed_cache_paths()?;
    let selected = match target {
        MetadataSourceTarget::Mame => vec![paths.mame_xml],
        MetadataSourceTarget::Catver => vec![paths.catver],
        MetadataSourceTarget::All => vec![paths.mame_xml, paths.catver],
    };
    selected
        .into_iter()
        .map(|path| {
            let exists = path
                .try_exists()
                .map_err(|source| io_error(&path, source))?;
            Ok(CacheEntry { path, exists })
        })
        .collect()
}

pub fn clear_managed_cache(target: MetadataSourceTarget) -> Result<Vec<CacheEntry>> {
    let entries = plan_clear_managed_cache(target)?;
    for entry in &entries {
        if entry.exists {
            fs::remove_file(&entry.path).map_err(|source| io_error(&entry.path, source))?;
        }
    }
    Ok(entries)
}

fn resolve_catver_in_cache(
    explicit_path: Option<&Path>,
    cached_path: &Path,
) -> Result<ResolvedCatver> {
    if let Some(path) = explicit_path {
        return Ok(ResolvedCatver {
            path: path.to_path_buf(),
            source: CatverSource::ExplicitPath,
        });
    }
    if cached_path.is_file() {
        return Ok(ResolvedCatver {
            path: cached_path.to_path_buf(),
            source: CatverSource::Cache,
        });
    }

    download_catver(cached_path)?;
    Ok(ResolvedCatver {
        path: cached_path.to_path_buf(),
        source: CatverSource::Downloaded(CATVER_DOWNLOAD_URL.to_string()),
    })
}

fn download_catver(cached_path: &Path) -> Result<()> {
    download_catver_from(cached_path, CATVER_DOWNLOAD_URL)
}

fn download_catver_from(cached_path: &Path, url: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|source| ArcadeJanitorError::CatverDownload {
            url: url.to_string(),
            source,
        })?;
    download_catver_using(cached_path, url, &client)
}

fn download_catver_using(
    cached_path: &Path,
    url: &str,
    client: &reqwest::blocking::Client,
) -> Result<()> {
    let response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|source| ArcadeJanitorError::CatverDownload {
            url: url.to_string(),
            source,
        })?;
    let content = response
        .bytes()
        .map_err(|source| ArcadeJanitorError::CatverDownload {
            url: url.to_string(),
            source,
        })?;
    let content =
        String::from_utf8(content.to_vec()).map_err(ArcadeJanitorError::CatverEncoding)?;
    if parse_catver_str(&content)?.is_empty() {
        return Err(ArcadeJanitorError::InvalidCatverDownload);
    }

    let parent = cached_path
        .parent()
        .ok_or(ArcadeJanitorError::CacheDirectoryUnavailable)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let temporary_path = cached_path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, content).map_err(|source| io_error(&temporary_path, source))?;
    replace_cached_file(&temporary_path, cached_path)
}

fn replace_cached_file(temporary_path: &Path, cached_path: &Path) -> Result<()> {
    match fs::rename(temporary_path, cached_path) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() != ErrorKind::AlreadyExists => {
            return Err(io_error(cached_path, error));
        }
        Err(_) => {}
    }

    let backup_path = cached_path.with_extension(format!("bak-{}", std::process::id()));
    fs::rename(cached_path, &backup_path).map_err(|source| io_error(cached_path, source))?;
    match fs::rename(temporary_path, cached_path) {
        Ok(()) => fs::remove_file(&backup_path).map_err(|source| io_error(&backup_path, source)),
        Err(source) => {
            let _ = fs::rename(&backup_path, cached_path);
            Err(io_error(cached_path, source))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    struct LocalHttpServer {
        url: String,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl LocalHttpServer {
        fn new(status: &str, body: &[u8]) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let status = status.to_string();
            let body = body.to_vec();
            let stop = Arc::new(AtomicBool::new(false));
            let server_stop = Arc::clone(&stop);
            let thread = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline && !server_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            stream
                                .set_write_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let mut request = Vec::new();
                            while !request.ends_with(b"\r\n\r\n") {
                                let mut byte = [0];
                                assert_eq!(stream.read(&mut byte).unwrap(), 1);
                                request.push(byte[0]);
                                assert!(request.len() < 8192, "oversized HTTP request");
                            }
                            assert!(request.starts_with(b"GET / HTTP/1.1\r\n"));
                            write!(stream,
                                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()).unwrap();
                            stream.write_all(&body).unwrap();
                            return;
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("local HTTP accept failed: {error}"),
                    }
                }
            });
            Self {
                url,
                stop,
                thread: Some(thread),
            }
        }

        fn finish(mut self) {
            self.thread
                .take()
                .unwrap()
                .join()
                .expect("local HTTP server failed");
        }

        fn download(&self, cached_path: &Path) -> Result<()> {
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            download_catver_using(cached_path, &self.url, &client)
        }
    }

    impl Drop for LocalHttpServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take()
                && thread.join().is_err()
            {
                eprintln!("local HTTP server failed during cleanup");
            }
        }
    }

    #[test]
    fn prefers_an_explicit_catver_path() {
        let explicit_path = Path::new("custom-catver.ini");
        let cached_path = Path::new("cached-catver.ini");

        assert_eq!(
            resolve_catver_in_cache(Some(explicit_path), cached_path)
                .unwrap()
                .source,
            CatverSource::ExplicitPath
        );
    }

    #[test]
    fn prefers_an_explicit_mame_xml_path() {
        let explicit_path = Path::new("custom-mame.xml");
        let cached_path = Path::new("cached-mame.xml");

        assert!(matches!(
            resolve_mame_xml_path_in_cache(cached_path),
            Err(ArcadeJanitorError::MameXmlUnavailable)
        ));
        assert_eq!(
            resolve_mame_xml(Some(explicit_path), None).unwrap().source,
            MameXmlSource::ExplicitPath
        );
    }

    #[test]
    fn reuses_an_existing_cached_mame_xml() {
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("mame.xml");
        fs::write(&cached_path, "<mame/>").unwrap();

        assert_eq!(
            resolve_mame_xml_path_in_cache(&cached_path).unwrap(),
            cached_path
        );
    }

    #[test]
    fn reuses_an_existing_cached_catver() {
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");
        fs::write(&cached_path, "[Category]\npacman=Maze / Chase\n").unwrap();

        assert_eq!(
            resolve_catver_in_cache(None, &cached_path).unwrap().source,
            CatverSource::Cache
        );
    }

    #[test]
    fn downloads_and_validates_catver_before_caching() {
        let body = b"[Category]\npacman=Maze / Chase\n";
        let server = LocalHttpServer::new("200 OK", body);
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");
        server.download(&cached_path).unwrap();

        assert_eq!(
            fs::read_to_string(&cached_path).unwrap(),
            "[Category]\npacman=Maze / Chase\n"
        );
        server.finish();
        fs::write(&cached_path, "[Category]\nold=Old\n").unwrap();
        let server = LocalHttpServer::new("200 OK", body);
        server.download(&cached_path).unwrap();
        assert_eq!(fs::read(&cached_path).unwrap(), body);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        server.finish();
    }

    #[test]
    fn replaces_an_existing_cache_file() {
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("mame.xml");
        fs::write(&cached_path, "old").unwrap();

        cache_mame_xml(&cached_path, "<mame/>").unwrap();

        assert_eq!(fs::read_to_string(&cached_path).unwrap(), "<mame/>");
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            1,
            "temporary or backup files were not cleaned up"
        );
    }

    #[test]
    fn rejects_invalid_downloaded_catver_without_caching_it() {
        let server = LocalHttpServer::new("200 OK", b"[Category]\n");
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");

        assert!(matches!(
            server.download(&cached_path),
            Err(ArcadeJanitorError::InvalidCatverDownload)
        ));
        assert!(!cached_path.exists());
        server.finish();
    }

    #[test]
    fn reports_download_failures() {
        let server = LocalHttpServer::new("503 Service Unavailable", b"retry later");
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");

        let error = server.download(&cached_path).unwrap_err();
        assert!(matches!(error, ArcadeJanitorError::CatverDownload { .. }));
        assert!(!cached_path.exists());
        server.finish();
    }

    #[test]
    fn failed_catver_refresh_preserves_the_existing_cache() {
        for (status, body) in [
            ("503 Service Unavailable", b"retry later".as_slice()),
            ("200 OK", b"[Category]\n".as_slice()),
            ("200 OK", b"\xff".as_slice()),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let cached_path = directory.path().join("catver.ini");
            let original = b"[Category]\npacman=Maze / Chase\n";
            fs::write(&cached_path, original).unwrap();
            fs::write(directory.path().join("notes.txt"), b"preserve me").unwrap();
            let server = LocalHttpServer::new(status, body);
            let error = server.download(&cached_path).unwrap_err();
            match status {
                "503 Service Unavailable" => {
                    assert!(matches!(error, ArcadeJanitorError::CatverDownload { .. }))
                }
                _ if body == b"\xff" => {
                    assert!(matches!(error, ArcadeJanitorError::CatverEncoding(_)))
                }
                _ => assert!(matches!(error, ArcadeJanitorError::InvalidCatverDownload)),
            }
            assert_eq!(fs::read(&cached_path).unwrap(), original);
            assert_eq!(
                resolve_catver_in_cache(None, &cached_path).unwrap().source,
                CatverSource::Cache
            );
            assert_eq!(
                fs::read(directory.path().join("notes.txt")).unwrap(),
                b"preserve me"
            );
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
            server.finish();
        }
    }
}
