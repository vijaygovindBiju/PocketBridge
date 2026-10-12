use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use crate::error::PocketBridgeError;
use super::envelope::Envelope;

/// Maximum envelope JSON size: 4 MB (envelopes are small metadata).
pub const MAX_ENVELOPE_SIZE: u32 = 4 * 1024 * 1024;

/// Write a length-prefixed JSON envelope.
/// Wire format: [4-byte big-endian length][JSON bytes]
pub async fn write_envelope<W: AsyncWrite + Unpin>(
    writer: &mut W,
    envelope: &Envelope,
) -> Result<(), PocketBridgeError> {
    let json_bytes = serde_json::to_vec(envelope)?;
    let len = json_bytes.len() as u32;
    if len > MAX_ENVELOPE_SIZE {
        return Err(PocketBridgeError::MessageTooLarge {
            size: len,
            max: MAX_ENVELOPE_SIZE,
        });
    }
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&json_bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed JSON envelope.
pub async fn read_envelope<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Envelope, PocketBridgeError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_ENVELOPE_SIZE {
        return Err(PocketBridgeError::MessageTooLarge {
            size: len,
            max: MAX_ENVELOPE_SIZE,
        });
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    let envelope: Envelope = serde_json::from_slice(&buf)?;
    Ok(envelope)
}

/// Stream raw bytes through a writer in chunks.
pub async fn write_raw_stream<W: AsyncWrite + Unpin>(
    writer: &mut W,
    total_bytes: usize,
) -> Result<(), PocketBridgeError> {
    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
    let pattern_chunk = vec![0xABu8; CHUNK_SIZE];
    let mut remaining = total_bytes;

    while remaining > 0 {
        let to_write = remaining.min(CHUNK_SIZE);
        writer.write_all(&pattern_chunk[..to_write]).await?;
        remaining -= to_write;
    }
    writer.flush().await?;
    Ok(())
}

/// Stream and discard exact number of raw bytes (used during benchmark echo verification).
pub async fn drain_raw_stream<R: AsyncRead + Unpin>(
    reader: &mut R,
    total_bytes: usize,
) -> Result<(), PocketBridgeError> {
    const CHUNK_SIZE: usize = 64 * 1024;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut remaining = total_bytes;

    while remaining > 0 {
        let to_read = remaining.min(CHUNK_SIZE);
        reader.read_exact(&mut buf[..to_read]).await?;
        remaining -= to_read;
    }
    Ok(())
}
