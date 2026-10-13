//! A batch's plaintext: the Loro updates one push carries, one entry per
//! document. Encrypted as a whole before it leaves the device.
//!
//! ```text
//! version   u8                  currently 1
//! entries   repeated until the end:
//!   kind    u8                  0 = tree, 1 = note
//!   note    16 bytes            the note's id; only when kind is 1
//!   len     u32, little-endian
//!   update  len bytes
//! ```

use uuid::Uuid;

use crate::store::DocRef;
use crate::{CoreError, Result};

const VERSION: u8 = 1;

pub(crate) fn encode(entries: &[(DocRef, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![VERSION];
    for (doc, update) in entries {
        match doc {
            DocRef::Tree => out.push(0),
            DocRef::Note(id) => {
                out.push(1);
                out.extend_from_slice(id.as_bytes());
            }
        }
        out.extend_from_slice(&(update.len() as u32).to_le_bytes());
        out.extend_from_slice(update);
    }
    out
}

pub(crate) fn decode(batch: &[u8]) -> Result<Vec<(DocRef, Vec<u8>)>> {
    let malformed = || CoreError::Sync("malformed batch".into());
    let Some((&VERSION, mut rest)) = batch.split_first() else {
        return Err(malformed());
    };

    let mut entries = Vec::new();
    while let Some((&kind, after)) = rest.split_first() {
        let (doc, after) = match kind {
            0 => (DocRef::Tree, after),
            1 => {
                let (id, after) = after.split_first_chunk::<16>().ok_or_else(malformed)?;
                (DocRef::Note(Uuid::from_bytes(*id)), after)
            }
            _ => return Err(malformed()),
        };
        let (len, after) = after.split_first_chunk::<4>().ok_or_else(malformed)?;
        let (update, after) = after
            .split_at_checked(u32::from_le_bytes(*len) as usize)
            .ok_or_else(malformed)?;
        entries.push((doc, update.to_vec()));
        rest = after;
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let entries = vec![
            (DocRef::Tree, b"tree update".to_vec()),
            (DocRef::Note(Uuid::from_u128(7)), b"note update".to_vec()),
            (DocRef::Note(Uuid::from_u128(8)), Vec::new()),
        ];
        assert_eq!(decode(&encode(&entries)).unwrap(), entries);
    }

    #[test]
    fn rejects_truncation_and_unknown_versions() {
        let batch = encode(&[(DocRef::Note(Uuid::from_u128(7)), b"update".to_vec())]);
        // A lone version byte is a valid empty batch; any longer cut
        // splits the entry.
        for cut in 2..batch.len() {
            assert!(decode(&batch[..cut]).is_err(), "cut at {cut}");
        }
        assert!(decode(&[]).is_err());
        assert!(decode(&[2]).is_err());
    }
}
