use core::arch::x86_64::*;

pub trait DbgDmp<C: Default + Copy, const N: usize>: Sized {
    unsafe fn store(self, arr: &mut [C; N]);
    unsafe fn load(arr: [C; N]) -> Self;
    fn dmp_arr(self) -> [C; N] {
        let mut arr = [C::default(); N];
        unsafe { self.store(&mut arr) };
        arr
    }
}

macro_rules! dbg_dmp_impl {
    ($t: ident, $t2: ident,  $c: ident, $n: literal) => {
        impl DbgDmp<$c, $n> for concat_idents!(__, $t, i) {
            unsafe fn store(self, arr: &mut [$c; $n]) {
                use core::arch::x86_64::*;
                concat_idents!(_m, $t2, _storeu_ep, $c)(arr.as_mut_ptr(), self);
            }
            unsafe fn load(arr: [$c; $n]) -> Self {
                use core::arch::x86_64::*;
                concat_idents!(_m, $t2, _loadu_ep, $c)(arr.as_ptr())
            }
        }
    };
    ($t: ident,  $c: ident, $n: literal) => {
        dbg_dmp_impl!($t, $t, $c, $n);
    };
}
dbg_dmp_impl!(m128, m, i8, 16);
dbg_dmp_impl!(m128, m, i16, 8);
dbg_dmp_impl!(m128, m, i32, 4);
dbg_dmp_impl!(m256, i8, 32);
dbg_dmp_impl!(m256, i32, 8);
dbg_dmp_impl!(m512, i32, 16);