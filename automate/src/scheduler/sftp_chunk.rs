//! Chunked SFTP transfer with session reuse.
//!
//! A whole-file transfer encodes the file into a single WebSocket frame. That
//! frame is bounded by `max_message_size` (16 MiB) and, because `Vec<u8>` is
//! serialized by serde_json into an array of decimal numbers, its payload is
//! inflated 3~4x. Together those two facts make any file above 4 MiB fail.
//!
//! Chunked transfer splits the file into fixed size pieces, so it is neither
//! bound by the frame limit nor does it hold the whole file in memory. Rebuilding
//! the SSH connection for every chunk would dominate the runtime (~3 MiB/s
//! measured), so sessions are keyed by `session_id` and reuse
//! `(SSH connection, SFTP session, remote file handle)`.
//!
//! Edge cases handled by session reuse:
//!
//! 1. **Out of order writes**: the offset is taken from the client and written
//!    with `seek`, so a retry or a session rebuild never corrupts ranges that
//!    were already acknowledged;
//! 2. **Broken connections**: a failed read or write drops the session and
//!    returns a retryable error; the client resumes from its own offset;
//! 3. **Idle reaping**: sessions unused for longer than [`SESSION_IDLE_TIMEOUT`]
//!    are closed, so a client that disappears does not leak handles;
//! 4. **Session cap**: above [`MAX_SESSIONS`] the least recently used session is
//!    evicted;
//! 5. **Concurrency**: every session owns a lock, so chunks of one session are
//!    strictly serialized.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use russh_sftp::client::SftpSession as RawSftpSession;
use russh_sftp::client::fs::File as SftpFile;
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, MutexGuard};

use crate::ssh::{AuthData, ConnectParams2, Session};

/// Size of a single chunk in bytes.
pub const SFTP_CHUNK_SIZE: usize = 512 * 1024;

/// How long an unused session is kept before it is closed.
pub const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Upper bound of live sessions; the least recently used one is evicted.
pub const MAX_SESSIONS: usize = 32;

/// The russh handle does not implement `PartialEq` while `MsgReqKind` requires
/// it, so comparisons between handles are explicitly ignored.
pub struct SftpHandle(pub RawSftpSession);

impl PartialEq for SftpHandle {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for SftpHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SftpHandle")
    }
}

/// Reusable state of one transfer.
struct Transfer {
    /// The underlying SSH connection must be kept alive or the SFTP channel dies.
    _ssh: Session,
    sftp: Arc<RawSftpSession>,
    /// Upload: offset where the last acknowledged write ended, informational.
    received: u64,
    /// Upload: currently open remote file handle.
    upload_file: Option<SftpFile>,
    /// Download: currently open remote file handle, reused to avoid re-opening
    /// the file for every chunk.
    download_file: Option<SftpFile>,
    last_used: Instant,
}

/// Establish one SSH + SFTP session.
pub async fn open(
    user: &str,
    auth: AuthData,
    port: u16,
) -> Result<(Session, RawSftpSession)> {
    let ssh = Session::connect2(ConnectParams2 {
        user: user.to_string(),
        auth,
        addrs: ("127.0.0.1", port),
    })
    .await?;
    let sftp = ssh.sftp_client().await?;
    Ok((ssh, sftp))
}

type Registry = Mutex<HashMap<String, Arc<Mutex<Transfer>>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Take a session, creating it when absent; also reaps idle sessions and
/// enforces the session cap.
async fn acquire(
    session_id: &str,
    user: &str,
    auth: AuthData,
    port: u16,
) -> Result<Arc<Mutex<Transfer>>> {
    {
        let mut map = registry().lock().await;

        // Reap sessions that have been idle for too long.
        map.retain(|_, v| {
            v.try_lock()
                .map(|g| g.last_used.elapsed() < SESSION_IDLE_TIMEOUT)
                .unwrap_or(true)
        });

        if let Some(existing) = map.get(session_id) {
            return Ok(existing.clone());
        }
    }

    let (ssh, sftp) = open(user, auth, port).await?;
    let transfer = Arc::new(Mutex::new(Transfer {
        _ssh: ssh,
        sftp: Arc::new(sftp),
        received: 0,
        upload_file: None,
        download_file: None,
        last_used: Instant::now(),
    }));

    let mut map = registry().lock().await;
    // Evict the least recently used idle session once the cap is reached.
    if map.len() >= MAX_SESSIONS {
        while map.len() >= MAX_SESSIONS {
            let victim = map
                .iter()
                .filter(|(_, v)| v.try_lock().is_ok())
                .min_by_key(|(_, v)| {
                    v.try_lock()
                        .map(|g| g.last_used)
                        .unwrap_or_else(|_| Instant::now())
                })
                .map(|(k, _)| k.clone());
            match victim {
                Some(k) => {
                    map.remove(&k);
                }
                None => break,
            }
        }
    }
    map.insert(session_id.to_string(), transfer.clone());

    Ok(transfer)
}

/// Drop a session (on error or completion); its handles are closed with it.
pub async fn drop_session(session_id: &str) {
    registry().lock().await.remove(session_id);
}

async fn lock_transfer<'a>(
    transfer: &'a Arc<Mutex<Transfer>>,
) -> MutexGuard<'a, Transfer> {
    let mut guard = transfer.lock().await;
    guard.last_used = Instant::now();
    guard
}

/// Upload: write one chunk at the given offset.
///
/// The offset is authoritative and comes from the client, so a chunk can be
/// retried or resumed after the session was reaped.
pub async fn write_chunk(
    session_id: &str,
    user: &str,
    auth: AuthData,
    port: u16,
    filepath: &str,
    offset: u64,
    data: Vec<u8>,
) -> Result<u64> {
    let transfer = acquire(session_id, user, auth, port).await?;
    let mut t = lock_transfer(&transfer).await;
    let len = data.len() as u64;

    // Self healing after a broken connection or a reaped session: open the file
    // according to the offset declared by the client. offset == 0 creates a new
    // file (CREATE|TRUNCATE), anything else opens it for writing without
    // truncating (CREATE|WRITE), so a client can always resume from an
    // acknowledged offset.
    if t.upload_file.is_none() {
        let file = if offset == 0 {
            t.sftp.create(filepath).await?
        } else {
            t.sftp
                .open_with_flags(
                    filepath,
                    OpenFlags::CREATE | OpenFlags::WRITE,
                )
                .await?
        };
        t.upload_file = Some(file);
    }

    let write_ret = {
        let file = t.upload_file.as_mut().expect("upload file opened");
        async {
            file.seek(std::io::SeekFrom::Start(offset)).await?;
            file.write_all(&data).await?;
            file.flush().await?;
            Ok::<(), anyhow::Error>(())
        }
        .await
    };

    match write_ret {
        Ok(()) => {
            t.received = offset + len;
            Ok(len)
        }
        Err(e) => {
            // The session is no longer trustworthy: drop it and let the client
            // retry from its own offset.
            drop(t);
            drop_session(session_id).await;
            Err(e)
        }
    }
}

/// Upload finished: verify the remote size and release the session.
pub async fn finish_upload(
    session_id: &str,
    user: &str,
    auth: AuthData,
    port: u16,
    filepath: &str,
    total_size: u64,
) -> Result<u64> {
    let transfer = acquire(session_id, user, auth, port).await?;
    let size = {
        let mut t = lock_transfer(&transfer).await;
        // Close the write handle so the length is flushed before stat.
        t.upload_file = None;
        t.sftp.metadata(filepath).await?.len()
    };

    drop_session(session_id).await;

    if size != total_size {
        anyhow::bail!(
            "remote file size mismatch: expected {}, got {}",
            total_size,
            size
        );
    }

    Ok(size)
}

/// Download: query the remote size and establish a reusable session.
pub async fn download_stat(
    session_id: &str,
    user: &str,
    auth: AuthData,
    port: u16,
    filepath: &str,
) -> Result<u64> {
    let transfer = acquire(session_id, user, auth, port).await?;
    let t = lock_transfer(&transfer).await;
    let meta = t.sftp.metadata(filepath).await;
    match meta {
        Ok(m) => Ok(m.len()),
        Err(e) => {
            drop(t);
            drop_session(session_id).await;
            Err(e.into())
        }
    }
}

/// Download: read the requested range.
pub async fn read_chunk(
    session_id: &str,
    user: &str,
    auth: AuthData,
    port: u16,
    filepath: &str,
    offset: u64,
    len: u32,
) -> Result<Vec<u8>> {
    let transfer = acquire(session_id, user, auth, port).await?;
    let mut t = lock_transfer(&transfer).await;

    // Open the remote file on first read and reuse it for the session.
    if t.download_file.is_none() {
        t.download_file = Some(t.sftp.open(filepath).await?);
    }

    let read_ret = {
        let file = t.download_file.as_mut().expect("download file opened");
        async {
            file.seek(std::io::SeekFrom::Start(offset)).await?;
            let mut buf = vec![0u8; len as usize];
            let mut filled = 0usize;
            while filled < buf.len() {
                let n = file.read(&mut buf[filled..]).await?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            buf.truncate(filled);
            Ok::<Vec<u8>, anyhow::Error>(buf)
        }
        .await
    };

    match read_ret {
        Ok(buf) => Ok(buf),
        Err(e) => {
            drop(t);
            drop_session(session_id).await;
            Err(e)
        }
    }
}

/// Download finished: release the session.
pub async fn finish_download(session_id: &str) {
    drop_session(session_id).await;
}

/// Number of live sessions (for tests).
pub async fn session_count() -> usize {
    registry().lock().await.len()
}
