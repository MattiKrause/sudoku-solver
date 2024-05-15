use crate::solver_base::FlatIndex;

pub trait WorkQueue {
    fn new() -> Self;
    fn pop(&mut self) -> Option<FlatIndex>;
}

#[allow(clippy::module_name_repetitions)]
pub struct BitMaskWorkQueue(pub u128);

impl BitMaskWorkQueue {
    pub fn remove(&mut self, v: FlatIndex) {
        let rem_mask = !(1u128 << v.as_idx());
        self.0 &= rem_mask;
    }
    #[cfg(test)]
    pub fn push(&mut self, v: FlatIndex) {
        self.0 |= 1u128 << (v.get() as u128);
    }
}

impl WorkQueue for BitMaskWorkQueue {
    fn new() -> Self {
        Self(0)
    }

    fn pop(&mut self) -> Option<FlatIndex> {
        if self.0 == 0 {
            return None;
        }
        let next = self.0.trailing_zeros();
        self.0 ^= 1u128 << next;
        u8::try_from(next)
            .ok()
            .and_then(FlatIndex::new)
    }
}

#[cfg(test)]
mod test {
    use crate::solver_base::FlatIndex;
    use crate::work_queue::{BitMaskWorkQueue, WorkQueue};

    fn test_push_pop<WQ: WorkQueue, F>(mut push: F) where for<'a> F: FnMut(&'a mut WQ, FlatIndex) {
        let mut wq = WQ::new();
        let mut values = [0, 3, 24, 66, 26, 80, 1, 42, 33].map(|v| FlatIndex::new(v).unwrap());
        let mut popped = Vec::new();
        for v in values {
            push(&mut wq, v);
        }
        while let Some(v) = wq.pop() {
            popped.push(v);
        }
        for _ in 0..1000 {
            assert!(wq.pop().is_none());
        }
        values.sort_by_key(|v| v.as_idx());
        popped.sort_by_key(|v| v.as_idx());
        assert_eq!(values.as_slice(), popped.as_slice());
    }

    #[test]
    fn test_bitmask_wq() {
        test_push_pop(BitMaskWorkQueue::push);
    }
}