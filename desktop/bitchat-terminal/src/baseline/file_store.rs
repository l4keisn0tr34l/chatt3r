//! Opt-in, receive-only quarantine store. Never uses the untrusted wire name as a path.
use crate::file_packet::FilePayload;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_SAVES_PER_RUN: usize = 16;

pub struct IncomingFiles {
    directory: PathBuf,
    saved: usize,
}

impl IncomingFiles {
    /// Require an existing real directory. Never silently create a download
    /// directory or follow a final-component symlink supplied by a CLI flag.
    pub fn new(directory: &Path) -> io::Result<Self> {
        let meta = fs::symlink_metadata(directory)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "receive path must be an existing non-symlink directory",
            ));
        }
        Ok(Self {
            directory: directory.canonicalize()?,
            saved: 0,
        })
    }

    pub fn save(&mut self, file: &FilePayload) -> io::Result<(PathBuf, [u8; 32])> {
        if self.saved >= MAX_SAVES_PER_RUN {
            return Err(io::Error::other("incoming file limit reached for this run"));
        }
        if file.content.is_empty() || file.content.len() > crate::file_packet::MAX_FILE_BYTES {
            return Err(io::Error::other("incoming file exceeds size budget"));
        }
        let valid_type = match file.mime_type.as_deref() {
            Some("image/jpeg") => file.content.starts_with(&[0xff, 0xd8, 0xff]),
            Some("image/png") => file.content.starts_with(b"\x89PNG\r\n\x1a\n"),
            Some("image/gif") => {
                file.content.starts_with(b"GIF87a") || file.content.starts_with(b"GIF89a")
            }
            Some("image/webp") => {
                file.content.len() >= 12
                    && file.content.starts_with(b"RIFF")
                    && &file.content[8..12] == b"WEBP"
            }
            Some("application/octet-stream") => true, // untrusted bytes, saved only as .bin; never opened
            _ => false,
        };
        if !valid_type {
            return Err(io::Error::other(
                "unsupported or mismatched incoming file MIME",
            ));
        }
        // Refuse directory substitution between configuration and save. This
        // is best-effort, not an OS-level openat2 sandbox for hostile local users.
        if self.directory.symlink_metadata()?.file_type().is_symlink() {
            return Err(io::Error::other("receive directory became a symlink"));
        }
        let digest: [u8; 32] = Sha256::digest(&file.content).into();
        for _ in 0..3 {
            let random: [u8; 16] = rand::random();
            let path = self
                .directory
                .join(format!("received-{}.bin", hex::encode(random)));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = match options.open(&path) {
                Ok(output) => output,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            };
            let result = output
                .write_all(&file.content)
                .and_then(|_| output.sync_all());
            drop(output);
            if let Err(error) = result {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
            self.saved += 1;
            return Ok((path, digest));
        }
        Err(io::Error::other("incoming filename collisions"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_directory_random_name_private_file_and_mime_check() {
        let dir = std::env::temp_dir().join(format!(
            "chatt3r-store-test-{}",
            hex::encode(rand::random::<[u8; 16]>())
        ));
        fs::create_dir(&dir).unwrap();
        let mut store = IncomingFiles::new(&dir).unwrap();
        let file = FilePayload {
            file_name: Some("../../bad.png".into()),
            mime_type: Some("image/png".into()),
            content: b"\x89PNG\r\n\x1a\nbody".to_vec(),
        };
        let (path, hash) = store.save(&file).unwrap();
        assert_eq!(path.parent(), Some(dir.as_path()));
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("received-"));
        assert_eq!(path.extension().unwrap(), "bin");
        assert_eq!(fs::read(&path).unwrap(), file.content);
        assert_eq!(hash, Sha256::digest(&file.content).as_slice());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let bad = FilePayload {
            mime_type: Some("image/jpeg".into()),
            ..file
        };
        assert!(store.save(&bad).is_err());
        assert!(IncomingFiles::new(&path).is_err());
        #[cfg(unix)]
        {
            let link = dir.with_extension("link");
            std::os::unix::fs::symlink(&dir, &link).unwrap();
            assert!(IncomingFiles::new(&link).is_err());
            fs::remove_file(link).unwrap();
        }
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
