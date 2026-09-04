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

    let cache_path = dirs_next::cache_dir()
        .ok_or(ArcadeJanitorError::CacheDirectoryUnavailable)?
        .join("arcadejanitor")
        .join("mame.xml");
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
    let directory = dirs_next::cache_dir()
        .ok_or(ArcadeJanitorError::CacheDirectoryUnavailable)?
        .join("arcadejanitor");
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
    let response = reqwest::blocking::get(url)
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
        thread,
    };

    use super::*;

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
        let directory =
            std::env::temp_dir().join(format!("arcadejanitor-mame-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let cached_path = directory.join("mame.xml");
        fs::write(&cached_path, "<mame/>").unwrap();

        assert_eq!(
            resolve_mame_xml_path_in_cache(&cached_path).unwrap(),
            cached_path
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reuses_an_existing_cached_catver() {
        let directory =
            std::env::temp_dir().join(format!("arcadejanitor-catver-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let cached_path = directory.join("catver.ini");
        fs::write(&cached_path, "[Category]\npacman=Maze / Chase\n").unwrap();

        assert_eq!(
            resolve_catver_in_cache(None, &cached_path).unwrap().source,
            CatverSource::Cache
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn downloads_and_validates_catver_before_caching() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let bytes_read = stream.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..bytes_read])
                    .unwrap()
                    .starts_with("GET / HTTP/1.1")
            );

            let body = "[Category]\npacman=Maze / Chase\n";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let directory = std::env::temp_dir().join(format!(
            "arcadejanitor-catver-download-test-{}",
            std::process::id()
        ));
        let cached_path = directory.join("catver.ini");

        download_catver_from(&cached_path, &url).unwrap();

        assert_eq!(
            fs::read_to_string(&cached_path).unwrap(),
            "[Category]\npacman=Maze / Chase\n"
        );
        server.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn replaces_an_existing_cache_file() {
        let directory = std::env::temp_dir().join(format!(
            "arcadejanitor-cache-replace-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let cached_path = directory.join("mame.xml");
        fs::write(&cached_path, "old").unwrap();

        cache_mame_xml(&cached_path, "<mame/>").unwrap();

        assert_eq!(fs::read_to_string(&cached_path).unwrap(), "<mame/>");
        assert_eq!(
            fs::read_dir(&directory).unwrap().count(),
            1,
            "temporary or backup files were not cleaned up"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_invalid_downloaded_catver_without_caching_it() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).unwrap();
            let body = "[Category]\n";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");

        assert!(matches!(
            download_catver_from(&cached_path, &url),
            Err(ArcadeJanitorError::InvalidCatverDownload)
        ));
        assert!(!cached_path.exists());
        server.join().unwrap();
    }

    #[test]
    fn reports_download_failures() {
        let directory = tempfile::tempdir().unwrap();
        let cached_path = directory.path().join("catver.ini");

        let error = download_catver_from(&cached_path, "http://127.0.0.1:1").unwrap_err();
        assert!(matches!(error, ArcadeJanitorError::CatverDownload { .. }));
    }
}
