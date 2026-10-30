//! Source ingestion limits shared by disk readers and editor buffers.

use std::io::{self, Read};
use std::path::Path;

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
