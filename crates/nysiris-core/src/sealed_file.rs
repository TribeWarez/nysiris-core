use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chacha20poly1305::{
    aead::{Aead, Payload},
    Key, KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;

use crate::error::{CoreError, Result};

const MAGIC: &[u8; 5] = b"NYSF1";

/// Seal arbitrary bytes into a self-contained blob: magic + nonce + ciphertext.
pub fn seal_bytes(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("sealed-file encryption failed".into()))?;

    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Open a sealed blob. Fails closed on a bad magic, short input, wrong key, or
/// mismatched associated data.
pub fn open_bytes(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < MAGIC.len() + 24 {
        return Err(CoreError::Crypto("sealed file is too short".into()));
    }
    if &sealed[..MAGIC.len()] != MAGIC {
        return Err(CoreError::Crypto("sealed file has a bad magic".into()));
    }
    let nonce_start = MAGIC.len();
    let ct_start = nonce_start + 24;
    let nonce = &sealed[nonce_start..ct_start];
    let ciphertext = &sealed[ct_start..];
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("sealed file could not be opened".into()))
}

/// Write a sealed blob atomically.
///
/// The new contents are written to a temporary sibling, flushed to disk, and
/// then moved into place. The previous contents are retained as `<name>.bak`
/// so a crash between the two renames can be recovered by [`read_sealed`].
pub fn write_sealed(path: &Path, key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<()> {
    let sealed = seal_bytes(key, aad, plaintext)?;
    write_atomic(path, &sealed)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(parent)?;

    let temp = temporary_path(path);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&temp);
        return Err(err.into());
    }
    drop(file);

    let backup = backup_path(path);
    if path.exists() {
        let _ = std::fs::remove_file(&backup);
        if let Err(err) = std::fs::rename(path, &backup) {
            let _ = std::fs::remove_file(&temp);
            return Err(err.into());
        }
    }
    if let Err(err) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        if backup.exists() {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(err.into());
    }

    restrict_permissions(path);
    sync_directory(parent);
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut random = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut random);
    let mut name = OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        u64::from_le_bytes(random)
    ));
    path.with_file_name(name)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.file_name().unwrap_or_default());
    name.push(".bak");
    path.with_file_name(name)
}

fn sync_directory(path: &Path) {
    #[cfg(unix)]
    {
        if let Ok(directory) = std::fs::File::open(path) {
            let _ = directory.sync_all();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Read and open a sealed blob written by [`write_sealed`].
///
/// If the primary file is missing or unreadable and a `<name>.bak` sibling
/// validates with the same key/AAD, that backup is used and copied back into
/// place. A wrong key never succeeds because both copies fail to open.
pub fn read_sealed(path: &Path, key: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
    let primary = read_sealed_file(path, key, aad);
    if primary.is_ok() {
        return primary;
    }

    let backup = backup_path(path);
    if !backup.is_file() {
        return primary;
    }
    match read_sealed_file(&backup, key, aad) {
        Ok(bytes) => {
            // Best-effort restore; callers still get the recovered plaintext.
            let _ = std::fs::copy(&backup, path);
            restrict_permissions(path);
            Ok(bytes)
        }
        Err(_) => primary,
    }
}

fn read_sealed_file(path: &Path, key: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
    match std::fs::read(path) {
        Ok(sealed) => open_bytes(key, aad, &sealed),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_bytes_round_trip_and_reject_wrong_aad() {
        let key = [7u8; 32];
        let sealed = seal_bytes(&key, b"label", b"hello").unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"hello"));
        assert_eq!(open_bytes(&key, b"label", &sealed).unwrap(), b"hello");
        assert!(open_bytes(&key, b"other", &sealed).is_err());
    }

    #[test]
    fn atomic_write_keeps_previous_generation_and_recovers_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("contacts.enc");
        let key = [7u8; 32];

        write_sealed(&path, &key, b"contacts", b"first").unwrap();
        write_sealed(&path, &key, b"contacts", b"second").unwrap();
        assert_eq!(read_sealed(&path, &key, b"contacts").unwrap(), b"second");
        assert!(path.with_file_name("contacts.enc.bak").is_file());

        std::fs::remove_file(&path).unwrap();
        assert_eq!(read_sealed(&path, &key, b"contacts").unwrap(), b"first");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            std::fs::read(path.with_file_name("contacts.enc.bak")).unwrap()
        );
    }

    #[test]
    fn wrong_key_still_fails_with_a_backup_present() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("groups.enc");
        write_sealed(&path, &[7u8; 32], b"groups", b"secret").unwrap();
        write_sealed(&path, &[7u8; 32], b"groups", b"newer").unwrap();
        assert!(read_sealed(&path, &[8u8; 32], b"groups").is_err());
    }

    #[test]
    fn missing_file_error_is_preserved() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("identity.enc");
        let err = read_sealed(&path, &[7u8; 32], b"identity").unwrap_err();
        assert!(matches!(err, CoreError::Io(err) if err.kind() == std::io::ErrorKind::NotFound));
    }
}
