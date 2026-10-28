use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use subtle::ConstantTimeEq;

pub const LEGACY_PUBLIC_HMAC_SECRET: &[u8] =
    b"poverello-v0.24.6-auto-converge-engine-authority-key";

#[derive(Debug)]
pub enum StateStoreError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    Crypto(String),
    InvalidRunId(String),
    InvalidMac,
    LegacyKeyRejected,
    SequenceRollback { expected: u64, found: u64 },
    UnsupportedPlatform,
}

impl fmt::Display for StateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Serialization(e) => write!(f, "Serialization error: {e}"),
            Self::Crypto(msg) => write!(f, "Cryptographic protection error: {msg}"),
            Self::InvalidRunId(run_id) => write!(f, "Invalid run ID '{run_id}'"),
            Self::InvalidMac => write!(f, "Authentication failed: invalid HMAC signature"),
            Self::LegacyKeyRejected => write!(
                f,
                "Rejected legacy public key signature: state must be re-initialized"
            ),
            Self::SequenceRollback { expected, found } => {
                write!(
                    f,
                    "State sequence violation: expected {expected}, found {found}"
                )
            }
            Self::UnsupportedPlatform => write!(f, "Unsupported platform security boundary"),
        }
    }
}

impl std::error::Error for StateStoreError {}

const MAX_RUN_ID_BYTES: usize = 128;

/// Validates a run ID as a portable, single path component before it is used in any path.
pub fn validate_auto_converge_run_id(run_id: &str) -> Result<String, StateStoreError> {
    let bytes = run_id.as_bytes();
    let valid_chars = bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_');
    let starts_with_alphanumeric = bytes.first().is_some_and(u8::is_ascii_alphanumeric);
    let ends_with_alphanumeric = bytes.last().is_some_and(u8::is_ascii_alphanumeric);
    let upper = run_id.to_ascii_uppercase();
    let reserved_device_name = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 1
                    && suffix
                        .as_bytes()
                        .first()
                        .is_some_and(|digit| (b'1'..=b'9').contains(digit))
            })
        });

    if run_id.is_empty()
        || run_id.len() > MAX_RUN_ID_BYTES
        || !valid_chars
        || !starts_with_alphanumeric
        || !ends_with_alphanumeric
        || reserved_device_name
    {
        return Err(StateStoreError::InvalidRunId(run_id.to_string()));
    }

    Ok(run_id.to_string())
}

impl From<std::io::Error> for StateStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for StateStoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}

/// Pure resolver that computes the dedicated engine authority directory path
/// without creating directories or accessing the filesystem (§62.4.3).
pub fn resolve_engine_authority_dir_path(
    appdata: Option<&Path>,
    user_profile_or_home: Option<&Path>,
) -> Result<PathBuf, StateStoreError> {
    let base = if let Some(appdata) = appdata {
        appdata.to_path_buf()
    } else if let Some(home) = user_profile_or_home {
        home.join(".poverello")
    } else {
        return Err(StateStoreError::Crypto(
            "Unable to resolve persistent user profile directory for engine authority".into(),
        ));
    };
    Ok(base.join("poverello").join("engine_authority"))
}

#[cfg(test)]
thread_local! {
    static TEST_ENGINE_AUTHORITY_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_test_engine_authority_dir(dir: Option<PathBuf>) {
    TEST_ENGINE_AUTHORITY_DIR.with(|cell| {
        *cell.borrow_mut() = dir;
    });
}

#[cfg(test)]
pub fn get_test_engine_authority_dir() -> Option<PathBuf> {
    TEST_ENGINE_AUTHORITY_DIR.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub struct TestEngineAuthorityGuard {
    prev: Option<PathBuf>,
}

#[cfg(test)]
impl TestEngineAuthorityGuard {
    pub fn enter(dir: PathBuf) -> Self {
        let prev = TEST_ENGINE_AUTHORITY_DIR.with(|cell| {
            let p = cell.borrow().clone();
            *cell.borrow_mut() = Some(dir);
            p
        });
        Self { prev }
    }
}

#[cfg(test)]
impl Drop for TestEngineAuthorityGuard {
    fn drop(&mut self) {
        TEST_ENGINE_AUTHORITY_DIR.with(|cell| {
            *cell.borrow_mut() = self.prev.clone();
        });
    }
}

/// Resolves the dedicated engine authority directory under AppData, outside any workspace.
pub fn get_engine_authority_dir() -> Result<PathBuf, StateStoreError> {
    #[cfg(test)]
    {
        if let Some(dir) = get_test_engine_authority_dir() {
            std::fs::create_dir_all(&dir)?;
            return Ok(dir);
        }
    }
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    let dir = resolve_engine_authority_dir_path(appdata.as_deref(), home.as_deref())?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Resolves the dedicated runs directory under AppData, strictly engine-owned.
pub fn get_engine_runs_dir(run_id: &str) -> Result<PathBuf, StateStoreError> {
    let run_id = validate_auto_converge_run_id(run_id)?;
    let base = get_engine_authority_dir()?
        .parent()
        .unwrap_or(Path::new("."))
        .join("engine_runs");
    let dir = engine_run_directory_path(&base, &run_id)?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Builds one validated per-run path without accessing the filesystem.
pub fn engine_run_directory_path(
    engine_runs_root: &Path,
    run_id: &str,
) -> Result<PathBuf, StateStoreError> {
    let validated_run_id = validate_auto_converge_run_id(run_id)?;
    Ok(engine_runs_root.join(validated_run_id))
}

#[cfg(windows)]
#[repr(C)]
struct DATA_BLOB {
    cbData: u32,
    pbData: *mut u8,
}

#[cfg(windows)]
extern "system" {
    fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[cfg(windows)]
fn protect_key_bytes(raw: &[u8]) -> Result<Vec<u8>, StateStoreError> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Security::Cryptography::CryptProtectData;

    let mut in_blob = DATA_BLOB {
        cbData: raw.len() as u32,
        pbData: raw.as_ptr() as *mut u8,
    };
    let mut out_blob = DATA_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &mut in_blob as *mut _ as _,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut out_blob as *mut _ as _,
        )
    };

    if ok == 0 {
        return Err(StateStoreError::Crypto("CryptProtectData failed".into()));
    }

    let protected =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
    unsafe {
        LocalFree(out_blob.pbData as _);
    }
    Ok(protected)
}

#[cfg(windows)]
fn unprotect_key_bytes(protected: &[u8]) -> Result<Vec<u8>, StateStoreError> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Security::Cryptography::CryptUnprotectData;

    let mut in_blob = DATA_BLOB {
        cbData: protected.len() as u32,
        pbData: protected.as_ptr() as *mut u8,
    };
    let mut out_blob = DATA_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &mut in_blob as *mut _ as _,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut out_blob as *mut _ as _,
        )
    };

    if ok == 0 {
        return Err(StateStoreError::Crypto("CryptUnprotectData failed".into()));
    }

    let raw =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
    unsafe {
        LocalFree(out_blob.pbData as _);
    }
    Ok(raw)
}

#[cfg(not(windows))]
fn protect_key_bytes(_raw: &[u8]) -> Result<Vec<u8>, StateStoreError> {
    Err(StateStoreError::UnsupportedPlatform)
}

#[cfg(not(windows))]
fn unprotect_key_bytes(_protected: &[u8]) -> Result<Vec<u8>, StateStoreError> {
    Err(StateStoreError::UnsupportedPlatform)
}

pub struct EngineAuthorityKey {
    key_bytes: Vec<u8>,
}

impl EngineAuthorityKey {
    /// Loads or creates an OS-protected persistent engine key outside the workspace.
    pub fn load_or_create() -> Result<Self, StateStoreError> {
        let auth_dir = get_engine_authority_dir()?;
        let key_path = auth_dir.join("engine_authority.key");

        if key_path.exists() {
            let protected = std::fs::read(&key_path)?;
            let raw = unprotect_key_bytes(&protected)?;
            if raw.len() == 32 {
                return Ok(Self { key_bytes: raw });
            } else {
                return Err(StateStoreError::Crypto(format!(
                    "Corrupt engine key length: expected 32 bytes, found {}",
                    raw.len()
                )));
            }
        }

        // Generate 32 bytes from CSPRNG
        let mut raw = vec![0u8; 32];
        OsRng.fill_bytes(&mut raw);

        let protected = protect_key_bytes(&raw)?;
        let nonce = uuid::Uuid::new_v4();
        let tmp_path = auth_dir.join(format!("engine_authority.key.{nonce}.tmp"));
        std::fs::write(&tmp_path, &protected)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
        }

        // Atomic create/rename. If key_path now exists due to race, read and return that valid key.
        if let Err(e) = std::fs::rename(&tmp_path, &key_path) {
            let _ = std::fs::remove_file(&tmp_path);
            if key_path.exists() {
                let protected = std::fs::read(&key_path)?;
                let existing = unprotect_key_bytes(&protected)?;
                if existing.len() == 32 {
                    return Ok(Self {
                        key_bytes: existing,
                    });
                }
            }
            return Err(StateStoreError::Io(e));
        }

        Ok(Self { key_bytes: raw })
    }

    /// Explicit constructor for tests with isolated key bytes.
    pub fn from_raw_bytes(raw: Vec<u8>) -> Self {
        Self { key_bytes: raw }
    }

    pub fn compute_hmac(&self, payload: &[u8]) -> String {
        let mut key_pad = [0x36u8; 64];
        let mut opad = [0x5cu8; 64];
        for (i, &b) in self.key_bytes.iter().take(64).enumerate() {
            key_pad[i] ^= b;
            opad[i] ^= b;
        }

        let mut inner = Sha256::new();
        inner.update(&key_pad);
        inner.update(payload);
        let inner_hash = inner.finalize();

        let mut outer = Sha256::new();
        outer.update(&opad);
        outer.update(&inner_hash);
        let mac = outer.finalize();

        format!("{:x}", mac)
    }

    pub fn verify_hmac(&self, payload: &[u8], claimed_mac: &str) -> Result<(), StateStoreError> {
        if claimed_mac.len() != 64 || !claimed_mac.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StateStoreError::InvalidMac);
        }

        // First check if the claimed MAC was generated with the legacy public constant
        let legacy_key = Self {
            key_bytes: LEGACY_PUBLIC_HMAC_SECRET.to_vec(),
        };
        let legacy_mac = legacy_key.compute_hmac(payload);
        if legacy_mac.len() == claimed_mac.len() {
            let matches: bool = legacy_mac.as_bytes().ct_eq(claimed_mac.as_bytes()).into();
            if matches {
                return Err(StateStoreError::LegacyKeyRejected);
            }
        }

        let expected_mac = self.compute_hmac(payload);
        if expected_mac.len() != claimed_mac.len() {
            return Err(StateStoreError::InvalidMac);
        }
        let matches: bool = expected_mac.as_bytes().ct_eq(claimed_mac.as_bytes()).into();
        if matches {
            Ok(())
        } else {
            Err(StateStoreError::InvalidMac)
        }
    }
}
