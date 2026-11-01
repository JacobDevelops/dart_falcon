//! Source ingestion limits shared by disk readers and editor buffers.

use std::io::{self, Read};
use std::path::Path;

/// Acquisition of a required source, distinct from legitimate absence.
#[derive(Debug)]
pub enum SourceAcquisition {
    Acquired(String),
    Oversized(io::Error),
    Failed(io::Error),
}

/// A required source could not be read.
#[derive(Debug, Clone)]
pub struct SourceFailure {
    pub path: std::path::PathBuf,
    pub source: std::sync::Arc<io::Error>,
}

impl std::fmt::Display for SourceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "failed to analyze file {}: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for SourceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl SourceAcquisition {
    /// Return acquired text or retain the failing source path and underlying I/O error.
    ///
    /// # Errors
    /// Returns an error for unreadable or oversized sources.
    pub fn required(self, path: &Path) -> Result<String, SourceFailure> {
        match self {
            Self::Acquired(source) => Ok(source),
            Self::Oversized(source) | Self::Failed(source) => Err(SourceFailure {
                path: path.to_path_buf(),
                source: std::sync::Arc::new(source),
            }),
        }
    }

    /// Read a directive target, preserving genuine failures rather than legitimate absence.
    ///
    /// # Errors
    /// Returns genuine read failures, but not missing or non-file targets.
    pub fn companion(self, path: &Path) -> Result<Option<String>, SourceFailure> {
        match self {
            Self::Failed(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::IsADirectory
                        | io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(None)
            }
            acquisition => acquisition.required(path).map(Some),
        }
    }
}

/// Acquire bounded source text while preserving oversized and ordinary read failures.
#[must_use]
pub fn acquire_source(path: &Path) -> SourceAcquisition {
    match read_source(path) {
        Ok(source) => SourceAcquisition::Acquired(source),
        Err(error) if is_source_too_large(&error) => SourceAcquisition::Oversized(error),
        Err(error) => SourceAcquisition::Failed(error),
    }
}

/// Package roots, or containing directories for loose requested sources.
#[must_use]
pub fn project_source_roots<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
) -> Vec<std::path::PathBuf> {
    let paths: Vec<_> = paths
        .into_iter()
        .map(|path| {
            crate::canonical_or_lexical(
                &std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            )
        })
        .collect();
    let mut directories: Vec<_> = paths.iter().filter_map(|path| path.parent()).collect();
    directories.sort_unstable();
    directories.dedup();
    let mut roots: Vec<_> = directories
        .into_iter()
        .map(|directory| {
            directory
                .ancestors()
                .find(|root| root.join("pubspec.yaml").is_file())
                .unwrap_or(directory)
                .to_path_buf()
        })
        .collect();
    roots.sort_unstable();
    roots.dedup();
    roots
}

#[derive(Debug)]
struct SourceTooLarge(u64);

impl std::fmt::Display for SourceTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "source is {} bytes; maximum supported source size is {} bytes",
            self.0,
            u32::MAX
        )
    }
}

impl std::error::Error for SourceTooLarge {}

/// Distinguish rejected source sizes from other read errors, including invalid UTF-8.
#[must_use]
pub fn is_source_too_large(error: &io::Error) -> bool {
    error.get_ref().is_some_and(
        <dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static>::is::<
            SourceTooLarge,
        >,
    )
}

/// Validate a UTF-8 byte length before constructing compact syntax coordinates.
/// Exactly `u32::MAX` bytes are supported; larger sources are invalid input.
pub fn check_source_size(bytes: u64) -> io::Result<()> {
    if bytes > u64::from(u32::MAX) {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            SourceTooLarge(bytes),
        ))
    } else {
        Ok(())
    }
}

/// Read source text, rejecting oversized files before allocating their contents.
/// The bounded read and final check also cover files growing after metadata lookup.
///
/// # Errors
/// Returns an error for unreadable files, invalid UTF-8, oversized sources, or failed allocation.
pub fn read_source(path: &Path) -> io::Result<String> {
    let file = std::fs::File::open(path)?;
    let bytes = file.metadata()?.len();
    check_source_size(bytes)?;
    let mut source = String::new();
    source.try_reserve_exact(usize::try_from(bytes).map_err(io::Error::other)?)?;
    file.take(u64::from(u32::MAX) + 1)
        .read_to_string(&mut source)?;
    check_source_size(source.len() as u64)?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_coordinate_byte_limit_is_inclusive() {
        assert!(check_source_size(0).is_ok());
        assert!(check_source_size(u64::from(u32::MAX)).is_ok());
        let error = check_source_size(u64::from(u32::MAX) + 1).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("4294967295"));
        assert!(is_source_too_large(&error));
        assert!(!is_source_too_large(&io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid UTF-8",
        )));
    }

    #[test]
    fn oversized_buffer_has_a_normal_syntax_diagnostic() {
        let diagnostic = crate::parallel::source_size_diagnostic(
            Path::new("main.dart"),
            u64::from(u32::MAX) + 1,
        )
        .unwrap();
        assert_eq!(diagnostic.rule, "syntax-error");
        assert_eq!(diagnostic.span.start, 0);
        assert_eq!(diagnostic.span.end, 0);
        assert!(
            crate::parallel::source_size_diagnostic(Path::new("main.dart"), u64::from(u32::MAX),)
                .is_none()
        );
    }

    #[test]
    fn oversized_disk_source_is_rejected_before_reading() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oversized.dart");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(u64::from(u32::MAX) + 1).unwrap();
        let result = read_source(&path);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
