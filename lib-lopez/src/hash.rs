use siphasher::sip::SipHasher24;
use std::hash::{Hash, Hasher};

/// The default non-cryptographic hash used (trusty SipHash24).
pub fn hash<H: Hash>(thing: &H) -> i64 {
    let mut hasher = SipHasher24::new();
    thing.hash(&mut hasher);

    hasher.finish() as i64
}
