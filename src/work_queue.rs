#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
use core::arch::x86_64::__m256i;
#[cfg(all(target_arch = "x86", target_feature = "avx512f"))]
use core::arch::x86::{__m512i, __m256i};

pub struct WorkQueue<T: Sized> {
    start: *mut T,
    current: *mut T
}

impl <T> WorkQueue<T> {
    pub fn new_from(mem: *mut T) -> Self {
        Self {
            start: mem,
            current: mem
        }
    }
    pub unsafe fn write_ptr(&mut self) -> *mut T {
        self.current
    }

    pub unsafe fn extend_by(&mut self, len: usize) {
        self.current = self.current.add(len);
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.current == self.start {
            None
        } else {
            unsafe {
                self.current = self.current.sub(1);
                Some(std::ptr::read(self.current))
            }
        }
    }

    pub unsafe fn push(&mut self, t: T) {
        core::ptr::write(self.current, t);
        self.extend_by(1);

    }

    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx512f"))]
    #[inline(always)]
    pub unsafe fn write_simd256u16(&mut self, reg: __m256i, mask: u16) -> u32 {
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        let write_cnt = mask.count_ones();
        _mm256_mask_compressstoreu_epi16(self.write_ptr() as *mut u8, mask, reg);
        self.extend_by(write_cnt as usize);
        write_cnt
    }

    pub fn len(&mut self) -> usize {
        unsafe { self.current.offset_from(self.start as *const T) as usize}
    }

    pub fn as_slice(&mut self) -> &mut [T] {
        let len = self.len();
        unsafe { core::slice::from_raw_parts_mut(self.start, len) }
    }
}

pub struct WorkQueue81<T> {
    pub wq: WorkQueue<T>,
    owned: Box<[T; 81]>
}

impl <T: Default + Copy> WorkQueue81<T> {
    pub fn new() -> Self {
        let mut owned: Box<[T; 81]> = Box::new([T::default(); 81]);
        let wq = WorkQueue::new_from(owned.as_mut_ptr());
        WorkQueue81 {
            wq,
            owned
        }
    }
}