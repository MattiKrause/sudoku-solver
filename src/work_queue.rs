#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
use core::arch::x86_64::__m256i;
#[cfg(all(target_arch = "x86", target_feature = "avx512f"))]
use core::arch::x86::{__m512i, __m256i};
use crate::solver_base::FlatIndex;

pub trait WorkQueue {
    fn new() -> Self;
    fn pop(&mut self) -> Option<FlatIndex>;
}

pub struct ListWorkQueue<T: Sized, const LEN: usize> {
    owned_start: Box<[T; LEN]>,
    current_len: usize,
}

impl<T: Default + Copy, const N: usize> ListWorkQueue<T, N> {
    pub unsafe fn write_ptr(&mut self) -> *mut T {
        #[cfg(debug_assertions)]
        self.validate_state();
        self.owned_start.get_unchecked_mut(self.current_len)
    }

    #[cfg(debug_assertions)]
    fn validate_state(&self) {
        /*let addr_start = self.owned_start as usize;
        let addr_current = self.current as usize;
        let offset_bytes = addr_current.checked_sub(addr_start).expect("ListWorkQueue state invalid: current pointer offset from start pointer is negative");
        let offset = offset_bytes / std::mem::size_of::<T>();
        assert!(offset <= N);*/
        assert!(self.current_len <= N);
    }

    pub unsafe fn extend_by_unchecked(&mut self, len: usize) {
        self.current_len += len;
        #[cfg(debug_assertions)]
        self.validate_state();
    }

    pub unsafe fn push_unchecked(&mut self, t: T) {
        self.owned_start[self.current_len] = t;
        self.extend_by_unchecked(1);
    }

    pub fn len(&mut self) -> usize {
        #[cfg(debug_assertions)]
        self.validate_state();
        self.current_len
    }

    pub fn as_slice(&mut self) -> &mut [T] {
        #[cfg(debug_assertions)]
        {
            &mut self.owned_start[..self.current_len]
        }
        #[cfg(not(debug_assertions))]
        unsafe {
            self.owned_start.as_mut_slice().get_unchecked_mut(0..self.current_len)
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx512f"))]
impl<const N: usize> ListWorkQueue<u16, N> {
    #[inline(always)]
    pub unsafe fn write_simd256u16(&mut self, reg: __m256i, mask: u16) -> u32 {
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        let write_cnt = mask.count_ones();
        _mm256_mask_compressstoreu_epi16(self.write_ptr() as *mut u8, mask, reg);
        self.extend_by_unchecked(write_cnt as usize);
        write_cnt
    }
}

impl<T: Default + Copy + std::fmt::Debug> WorkQueue for ListWorkQueue<T, 81> where u8: TryFrom<T> {
    fn new() -> Self {
        Self {
            owned_start: Box::new([T::default(); 81]),
            current_len: 0,
        }
    }

    fn pop(&mut self) -> Option<FlatIndex> {
        let new_len = self.current_len.checked_sub(1)?;
        let value = self.owned_start[self.current_len];
        self.current_len = new_len;
        let idx = u8::try_from(value)
            .ok()
            .and_then(FlatIndex::new)
            .unwrap_or_else(|| panic!("{value:?} is not a valid index"));
        return Some(idx);
    }
}

/*pub struct WorkQueue81<T> {
    pub wq: ListWorkQueue<T, 81>,
    owned: Box<[T; 81]>
}

impl <T: Default + Copy + std::fmt::Debug> WorkQueue for WorkQueue81<T> where u8: TryFrom<T> {
    fn new() -> Self {
        let mut owned: Box<[T; 81]> = Box::new([T::default(); 81]);
        let wq = ListWorkQueue::new_from(owned.as_mut_ptr());
        WorkQueue81 {
            wq,
            owned
        }
    }

    fn pop(&mut self) -> Option<FlatIndex> {
        let idx = self.wq.pop()?;
        let idx = u8::try_from(idx)
            .ok()
            .and_then(FlatIndex::new)
            .unwrap_or_else(|| panic!("failed to convert {idx:?} to FlatIndex!"));
        Some(idx)
    }
}*/

pub struct BitMaskWorkQueue(pub u128);

impl BitMaskWorkQueue {
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
    use crate::work_queue::{BitMaskWorkQueue, ListWorkQueue, WorkQueue};

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

    #[test]
    fn test_list_wq() {
        test_push_pop(|wq, v| unsafe {
            ListWorkQueue::push_unchecked(wq, v.get() as u16);
        })
    }
}