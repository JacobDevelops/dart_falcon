//! Bounded ingestion for configuration and package/workspace metadata.

use std::io::{self, Read};
use std::path::Path;

/// Maximum bytes accepted for one configuration or package/workspace metadata file.
/// This is an ingestion budget, independent of Dart's compact source coordinates.
pub const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

fn check_size(size: u64, limit: u64) -> io::Result<()> {
    if size > limit {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("metadata is {size} bytes; maximum supported metadata size is {limit} bytes"),
        ))
    } else {
        Ok(())
    }
}

fn read_bounded(reader: impl Read, size: u64, limit: u64) -> io::Result<Vec<u8>> {
    check_size(size, limit)?;
    let mut contents = Vec::new();
    // Do not reserve based on untrusted metadata: the file may shrink or be special.
    reader.take(limit + 1).read_to_end(&mut contents)?;
    check_size(contents.len() as u64, limit)?;
    Ok(contents)
}

/// Read raw metadata bytes, including invalid UTF-8, for exact change detection.
///
/// # Errors
/// Returns I/O errors or `InvalidData` when the size exceeds [`MAX_METADATA_BYTES`].
/// Checks the opened file before reading and bounds reads if it grows afterwards.
pub fn read_metadata_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    read_bounded(file, size, MAX_METADATA_BYTES)
}

/// Read UTF-8 configuration or package/workspace metadata within the byte budget.
///
/// # Errors
/// Returns bounded-read errors or `InvalidData` for invalid UTF-8.
pub fn read_metadata_text(path: &Path) -> io::Result<String> {
    String::from_utf8(read_metadata_bytes(path)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_is_inclusive_and_growth_cannot_return_truncated_content() {
        assert_eq!(read_bounded(&b"1234"[..], 4, 4).unwrap(), b"1234");
        assert_eq!(read_bounded(&b"12"[..], 4, 4).unwrap(), b"12");
        let error = read_bounded(&b"123456"[..], 1, 4).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("metadata is 5 bytes"));
    }

    #[test]
    fn oversized_metadata_is_rejected_without_reading() {
        struct MustNotRead;
        impl Read for MustNotRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("oversized metadata must be rejected before reading");
            }
        }
        assert!(read_bounded(MustNotRead, 5, 4).is_err());
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(u64::from(u32::MAX) + 1).unwrap();
        assert!(read_metadata_bytes(file.path()).is_err());
        assert!(
            crate::deserialize_config(file.path())
                .unwrap_err()
                .to_string()
                .contains("maximum supported metadata size")
        );
        std::fs::write(file.path(), b"{}").unwrap();
        assert!(crate::deserialize_config(file.path()).is_ok());
    }

    #[test]
    fn fingerprints_keep_invalid_utf8_but_parsers_reject_it() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), [0xff, 0]).unwrap();
        assert_eq!(read_metadata_bytes(file.path()).unwrap(), [0xff, 0]);
        assert_eq!(
            read_metadata_text(file.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
