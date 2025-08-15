use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Result as IoResult, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Size-based rolling file appender that rotates logs when they exceed a configured size.
///
/// This appender creates log files with the pattern: `{prefix}.log`, `{prefix}.1.log`, etc.
/// When the current log file exceeds max_size_bytes:
/// 1. Current file is closed
/// 2. Existing files are rotated: .9.log -> delete, .8.log -> .9.log, etc.
/// 3. Current file becomes .1.log
/// 4. New current file is created
///
/// Only keeps up to `max_files` total files.
pub struct SizeBasedAppender {
    directory: PathBuf,
    file_prefix: String,
    #[cfg(test)]
    pub max_size_bytes: u64,
    #[cfg(not(test))]
    max_size_bytes: u64,
    max_files: usize,
    current_file: Arc<Mutex<Option<BufWriter<File>>>>,
    current_size: Arc<AtomicU64>,
}

impl SizeBasedAppender {
    /// Create a new size-based appender.
    ///
    /// # Arguments
    /// * `directory` - Directory to store log files
    /// * `file_prefix` - Prefix for log file names (e.g., "gate" creates "gate.log")
    /// * `max_size_mb` - Maximum size in MB before rotation (e.g., 10)
    /// * `max_files` - Maximum number of rotated files to keep (e.g., 10)
    ///
    /// # Returns
    /// * `Result<Self>` - New appender or error if directory cannot be created
    pub fn new(
        directory: PathBuf,
        file_prefix: String,
        max_size_mb: u64,
        max_files: usize,
    ) -> IoResult<Self> {
        // Ensure directory exists
        std::fs::create_dir_all(&directory)?;

        // Calculate size in bytes
        let max_size_bytes = max_size_mb * 1024 * 1024;

        // Get current file size if it exists
        let current_path = directory.join(format!("{}.log", file_prefix));
        let current_size = if current_path.exists() {
            current_path.metadata()?.len()
        } else {
            0
        };

        Ok(Self {
            directory,
            file_prefix,
            max_size_bytes,
            max_files,
            current_file: Arc::new(Mutex::new(None)),
            current_size: Arc::new(AtomicU64::new(current_size)),
        })
    }

    /// Check if we need to rotate and do it if necessary
    fn rotate_if_needed(&self) -> IoResult<()> {
        if self.current_size.load(Ordering::Relaxed) >= self.max_size_bytes {
            self.rotate_files()?;
        }
        Ok(())
    }

    /// Perform file rotation
    fn rotate_files(&self) -> IoResult<()> {
        // Close current file
        *self.current_file.lock().unwrap() = None;

        // Rotate existing numbered files: .9.log -> delete, .8.log -> .9.log, etc.
        for i in (1..self.max_files).rev() {
            let old_path = self
                .directory
                .join(format!("{}.{}.log", self.file_prefix, i));
            let new_path = self
                .directory
                .join(format!("{}.{}.log", self.file_prefix, i + 1));

            if old_path.exists() {
                if i + 1 >= self.max_files {
                    // Delete the oldest file (beyond max_files)
                    std::fs::remove_file(old_path)?;
                } else {
                    // Rotate to next number
                    std::fs::rename(old_path, new_path)?;
                }
            }
        }

        // Rotate current file to .1.log
        let current_path = self.directory.join(format!("{}.log", self.file_prefix));
        let rotated_path = self.directory.join(format!("{}.1.log", self.file_prefix));

        if current_path.exists() {
            std::fs::rename(current_path, rotated_path)?;
        }

        // Reset size counter
        self.current_size.store(0, Ordering::Relaxed);

        Ok(())
    }

    /// Get or create the current log file writer
    fn get_or_create_writer(&self) -> IoResult<()> {
        let mut file_guard = self.current_file.lock().unwrap();

        if file_guard.is_none() {
            let path = self.directory.join(format!("{}.log", self.file_prefix));
            let file = OpenOptions::new().create(true).append(true).open(path)?;
            *file_guard = Some(BufWriter::new(file));
        }

        Ok(())
    }
}

impl Write for SizeBasedAppender {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        // Check if we need to rotate before writing
        self.rotate_if_needed()?;

        // Ensure we have a writer
        self.get_or_create_writer()?;

        // Write the data
        let mut file_guard = self.current_file.lock().unwrap();
        let writer = file_guard.as_mut().unwrap();
        let written = writer.write(buf)?;
        writer.flush()?;

        // Update size counter
        self.current_size
            .fetch_add(written as u64, Ordering::Relaxed);

        Ok(written)
    }

    fn flush(&mut self) -> IoResult<()> {
        let mut file_guard = self.current_file.lock().unwrap();
        if let Some(ref mut writer) = *file_guard {
            writer.flush()?;
        }
        Ok(())
    }
}

// Make it safe to use across threads
unsafe impl Send for SizeBasedAppender {}
unsafe impl Sync for SizeBasedAppender {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_appender_creation() {
        let temp_dir = TempDir::new().unwrap();
        let appender = SizeBasedAppender::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
            1, // 1MB
            5, // 5 files
        )
        .unwrap();

        assert_eq!(appender.max_files, 5);
        assert_eq!(appender.max_size_bytes, 1024 * 1024);
    }

    #[test]
    fn test_basic_write() {
        let temp_dir = TempDir::new().unwrap();
        let mut appender = SizeBasedAppender::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
            1, // 1MB
            5, // 5 files
        )
        .unwrap();

        let data = b"Hello, world!\n";
        let written = appender.write(data).unwrap();
        assert_eq!(written, data.len());

        appender.flush().unwrap();

        // Check file was created
        let log_file = temp_dir.path().join("test.log");
        assert!(log_file.exists());

        let content = std::fs::read_to_string(log_file).unwrap();
        assert_eq!(content, "Hello, world!\n");
    }

    #[test]
    fn test_size_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let mut appender = SizeBasedAppender::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
            0, // Set to 0MB so max_size_bytes becomes 0, then override
            3, // 3 files max
        )
        .unwrap();

        // Override max_size_bytes for testing (1KB)
        appender.max_size_bytes = 1024;

        // Write enough data to trigger rotation
        let data = vec![b'A'; 1024]; // 1KB of data
        appender.write(&data).unwrap();
        appender.flush().unwrap();

        // Should have rotated, so we have test.log and test.1.log
        assert!(temp_dir.path().join("test.log").exists());

        // Write more data to trigger another rotation
        let data = vec![b'B'; 1024];
        appender.write(&data).unwrap();
        appender.flush().unwrap();

        // Should now have test.log, test.1.log, and test.2.log
        assert!(temp_dir.path().join("test.log").exists());
        assert!(temp_dir.path().join("test.1.log").exists());
    }
}
