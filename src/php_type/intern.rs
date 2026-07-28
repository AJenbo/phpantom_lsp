//! Hash-consing interner for [`PhpType`].
//!
//! Resolved types are massively duplicated: on a large Laravel project
//! 1.76 M live `PhpType` values collapse to ~51 K distinct forms, because
//! every `string`, `?Carbon` and `Collection<User>` is rebuilt per method,
//! per parameter and per inheriting class. Interning stores each distinct
//! form once and turns every occurrence into a pointer-sized handle.
//!
//! # Why refcounted rather than leaked
//!
//! [`Atom`](crate::atom::Atom) can leak because symbol names come from a
//! bounded set. Types do not: [`TypeKind::Literal`] carries source literals
//! and [`TypeKind::Raw`] carries free-text parse fallbacks, so a long-lived
//! server that re-analyses a file on every keystroke would grow without
//! bound. The table therefore holds [`Weak`] references and the handles hold
//! [`Arc`]s: a distinct form lives exactly as long as something refers to it.
//!
//! # Consequences for equality
//!
//! Because every `PhpType` is built through [`intern`], two structurally
//! equal types always share one allocation, so `PartialEq` is a pointer
//! comparison and `Hash` hashes the pointer. That holds even under
//! concurrent interning: `Weak::upgrade` only fails once the strong count
//! has reached zero, at which point no handle to the old allocation exists.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::{Arc, Weak};

use parking_lot::RwLock;

use super::{PhpType, TypeKind};

/// Number of independently locked shards. Interning happens on every
/// parse worker in parallel, so the table is sharded to keep the
/// uncontended read path the common case.
const SHARD_COUNT: usize = 64;

/// Minimum number of insertions into a shard before it is swept for
/// entries whose last handle has been dropped.
const SWEEP_INTERVAL: usize = 1024;

/// Hasher for the shard maps. The key is already a well-mixed 64-bit
/// content hash, so re-hashing it would be pure overhead.
#[derive(Default)]
struct PassThroughHasher(u64);

impl Hasher for PassThroughHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, _bytes: &[u8]) {
        unreachable!("the interner only ever hashes a u64 key");
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

/// Content hash of a [`TypeKind`].
///
/// Cheap by construction: child types hash as pointers, so this is
/// proportional to the node's direct arity, not to the size of the tree.
struct FxHasher(u64);

impl FxHasher {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        // Final avalanche: the shard index and the hashbrown control byte
        // both come from this value, and the multiply above leaves the low
        // bits weakly mixed.
        let mut h = self.0;
        h ^= h >> 32;
        h = h.wrapping_mul(0xd6e8_feb8_6659_fd93);
        h ^= h >> 32;
        h
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

type Buckets = HashMap<u64, Vec<Weak<TypeKind>>, BuildHasherDefault<PassThroughHasher>>;

#[derive(Default)]
struct Shard {
    buckets: Buckets,
    /// Insertions since the last sweep, used to amortise the cost of
    /// reclaiming entries whose last handle was dropped.
    since_sweep: usize,
}

fn shards() -> &'static [RwLock<Shard>] {
    static SHARDS: std::sync::OnceLock<Vec<RwLock<Shard>>> = std::sync::OnceLock::new();
    SHARDS.get_or_init(|| (0..SHARD_COUNT).map(|_| RwLock::default()).collect())
}

#[inline]
fn content_hash(kind: &TypeKind) -> u64 {
    let mut hasher = FxHasher(0);
    kind.hash(&mut hasher);
    hasher.finish()
}

/// Return the canonical handle for `kind`, allocating only if this exact
/// form is not currently live anywhere.
pub(super) fn intern(kind: TypeKind) -> PhpType {
    let hash = content_hash(&kind);
    // Shard on middle bits. `hashbrown` picks a bucket from the low bits of
    // the key and its control byte from the top seven, so taking the shard
    // from either end would leave every entry in a shard sharing those bits
    // and clustering inside the shard's own map.
    let shard = &shards()[(hash >> 32) as usize & (SHARD_COUNT - 1)];

    if let Some(existing) = shard
        .read()
        .buckets
        .get(&hash)
        .and_then(|bucket| lookup(bucket, &kind))
    {
        return PhpType(existing);
    }

    let mut shard = shard.write();
    // Another thread may have interned the same form between the two locks.
    let bucket = shard.buckets.entry(hash).or_default();
    if let Some(existing) = lookup(bucket, &kind) {
        return PhpType(existing);
    }

    let interned = Arc::new(kind);
    // Entries whose handles are gone hash to this same bucket forever, so
    // reuse the slot rather than letting the bucket grow.
    match bucket.iter_mut().find(|weak| weak.strong_count() == 0) {
        Some(slot) => *slot = Arc::downgrade(&interned),
        None => bucket.push(Arc::downgrade(&interned)),
    }

    shard.since_sweep += 1;
    if shard.since_sweep >= SWEEP_INTERVAL.max(shard.buckets.len()) {
        sweep(&mut shard);
    }

    PhpType(interned)
}

#[inline]
fn lookup(bucket: &[Weak<TypeKind>], kind: &TypeKind) -> Option<Arc<TypeKind>> {
    bucket
        .iter()
        .filter_map(Weak::upgrade)
        .find(|candidate| **candidate == *kind)
}

/// Drop table entries whose last handle has been released, freeing both the
/// weak slot and the `Arc` allocation header it was keeping alive.
fn sweep(shard: &mut Shard) {
    shard.since_sweep = 0;
    shard.buckets.retain(|_, bucket| {
        bucket.retain(|weak| weak.strong_count() > 0);
        !bucket.is_empty()
    });
}

/// Number of distinct type forms the interner currently holds a live
/// handle for. Exposed for the memory audit and for the reclamation test.
#[cfg(any(test, feature = "mem-audit"))]
pub(crate) fn interned_len() -> usize {
    shards()
        .iter()
        .map(|shard| {
            shard
                .read()
                .buckets
                .values()
                .flatten()
                .filter(|weak| weak.strong_count() > 0)
                .count()
        })
        .sum()
}
