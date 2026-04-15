//! Envelope that pairs a payload with the epoch it was produced at.
//! The App dispatcher inspects the epoch to drop stale work from previous
//! context generations.

use super::epoch::Epoch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedEvent<T> {
    payload: T,
    epoch: Epoch,
}

impl<T> VersionedEvent<T> {
    pub fn new(payload: T, epoch: Epoch) -> Self {
        Self { payload, epoch }
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_inner(self) -> T {
        self.payload
    }

    pub fn into_parts(self) -> (T, Epoch) {
        (self.payload, self.epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_payload_and_epoch() {
        let ev = VersionedEvent::new("hello".to_string(), 42);
        assert_eq!(ev.epoch(), 42);
        assert_eq!(ev.payload(), "hello");
        let (payload, epoch) = ev.into_parts();
        assert_eq!(payload, "hello");
        assert_eq!(epoch, 42);
    }

    #[test]
    fn into_inner_drops_epoch() {
        let ev = VersionedEvent::new(7i32, 99);
        assert_eq!(ev.into_inner(), 7);
    }

    #[test]
    fn different_payload_types_are_supported() {
        let s = VersionedEvent::new(vec![1, 2, 3], 1);
        assert_eq!(s.payload(), &vec![1, 2, 3]);
        let u = VersionedEvent::<()>::new((), 0);
        assert_eq!(u.epoch(), 0);
    }
}
