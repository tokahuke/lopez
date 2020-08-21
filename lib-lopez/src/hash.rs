use siphasher::sip::SipHasher24;
use std::hash::Hasher;

/// The default non-cryptographic hash used (trusty SipHash24).
pub fn hash(url: &str) -> i64 {
    let mut hasher = SipHasher24::new();
    hasher.write(url.as_bytes());

    hasher.finish() as i64
}
