use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{CleanMameError, Result, errors::io_error, parsers::catver::parse_catver_str};

pub const CATVER_DOWNLOAD_URL: &str =
    "https://raw.githubusercontent.com/AntoPISA/MAME_SupportFiles/main/catver.ini/catver.ini";

pub fn resolve_mame_xml_path(
    explicit_path: Option<&Path>,
    executable: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(path) = explicit_path {
        return Ok(path.to_path_buf());
    }

    let cache_path = dirs_next::cache_dir()
        .ok_or(CleanMameError::CacheDirectoryUnavailable)?
        .join("cleanmame")
        .join("mame.xml");
    if let Some(executable) = executable {
        extract_mame_xml(executable, &cache_path)?;
        Ok(cache_path)
    } else {
        resolve_mame_xml_path_in_cache(&cache_path)
    }
}

fn resolve_mame_xml_path_in_cache(cached_path: &Path) -> Result<PathBuf> {
    if cached_path.is_file() {
        return Ok(cached_path.to_path_buf());
    }

    Err(CleanMameError::MameXmlUnavailable)
}

fn extract_mame_xml(executable: &Path, cached_path: &Path) -> Result<()> {
    let output = Command::new(executable)
        .arg("-listxml")
        .output()
        .map_err(|source| io_error(executable, source))?;
    if !output.status.success() {
        return Err(CleanMameError::MameExecution {
            executable: executable.to_path_buf(),
            status: output.status.to_string(),
        });
    }
    let xml = String::from_utf8(output.stdout).map_err(CleanMameError::MameEncoding)?;
    crate::parsers::mame_xml::parse_mame_xml_str(&xml)?;
    cache_mame_xml(cached_path, &xml)?;
    Ok(())
}

fn cache_mame_xml(cached_path: &Path, content: &str) -> Result<()> {
    let parent = cached_path
        .parent()
        .ok_or(CleanMameError::CacheDirectoryUnavailable)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let temporary_path = cached_path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, content).map_err(|source| io_error(&temporary_path, source))?;
    match fs::rename(&temporary_path, cached_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists && cached_path.is_file() => {
            fs::remove_file(&temporary_path).map_err(|source| io_error(&temporary_path, source))
        }
        Err(source) => Err(io_error(cached_path, source)),
    }
}

pub fn resolve_catver_path(explicit_path: Option<&Path>) -> Result<PathBuf> {
    let cache_dir = dirs_next::cache_dir().ok_or(CleanMameError::CacheDirectoryUnavailable)?;
    resolve_catver_path_in_cache(
        explicit_path,
        &cache_dir.join("cleanmame").join("catver.ini"),
    )
}

fn resolve_catver_path_in_cache(
    explicit_path: Option<&Path>,
    cached_path: &Path,
) -> Result<PathBuf> {
    if let Some(path) = explicit_path {
        return Ok(path.to_path_buf());
    }
    if cached_path.is_file() {
        return Ok(cached_path.to_path_buf());
    }

    download_catver(cached_path)?;
    Ok(cached_path.to_path_buf())
}

fn download_catver(cached_path: &Path) -> Result<()> {
    download_catver_from(cached_path, CATVER_DOWNLOAD_URL)
}

fn download_catver_from(cached_path: &Path, url: &str) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|source| CleanMameError::CatverDownload {
            url: url.to_string(),
            source,
        })?;
    let content = response
        .bytes()
        .map_err(|source| CleanMameError::CatverDownload {
            url: url.to_string(),
            source,
        })?;
    let content = String::from_utf8(content.to_vec()).map_err(CleanMameError::CatverEncoding)?;
    if parse_catver_str(&content)?.is_empty() {
        return Err(CleanMameError::InvalidCatverDownload);
    }

    let parent = cached_path
        .parent()
        .ok_or(CleanMameError::CacheDirectoryUnavailable)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let temporary_path = cached_path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, content).map_err(|source| io_error(&temporary_path, source))?;
    match fs::rename(&temporary_path, cached_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists && cached_path.is_file() => {
            fs::remove_file(&temporary_path).map_err(|source| io_error(&temporary_path, source))
        }
        Err(source) => Err(io_error(cached_path, source)),
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
            resolve_catver_path_in_cache(Some(explicit_path), cached_path).unwrap(),
            explicit_path
        );
    }

    #[test]
    fn prefers_an_explicit_mame_xml_path() {
        let explicit_path = Path::new("custom-mame.xml");
        let cached_path = Path::new("cached-mame.xml");

        assert!(matches!(
            resolve_mame_xml_path_in_cache(cached_path),
            Err(CleanMameError::MameXmlUnavailable)
        ));
        assert_eq!(
            resolve_mame_xml_path(Some(explicit_path), None).unwrap(),
            explicit_path
        );
    }

    #[test]
    fn reuses_an_existing_cached_mame_xml() {
        let directory =
            std::env::temp_dir().join(format!("cleanmame-mame-test-{}", std::process::id()));
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
            std::env::temp_dir().join(format!("cleanmame-catver-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let cached_path = directory.join("catver.ini");
        fs::write(&cached_path, "[Category]\npacman=Maze / Chase\n").unwrap();

        assert_eq!(
            resolve_catver_path_in_cache(None, &cached_path).unwrap(),
            cached_path
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
            "cleanmame-catver-download-test-{}",
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
}
