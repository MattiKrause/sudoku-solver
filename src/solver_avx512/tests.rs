use std::arch::x86_64::*;
use crate::solver_avx512::dbg_dmp::DbgDmp;
use crate::solver_avx512::mask_current_quadrant;

const fn unit_idx<const N: usize>() -> [i32; N] {
    let mut res = [0; N];
    for i in 0..N {
        res[i] = i as i32;
    }
    res
}

#[test]
fn test_three_accum() {
    let arr = [13_i8, 2, 1, 6_, 7, 3, 0_, 0, 0, 1__, 2, 3, 9_, 8 ,4, 20_, 20, 40];
    let mut vals = [0; 32];
    vals[..18].copy_from_slice(&arr);
    let test_case = unsafe { __m256i::load(vals) };
    let res = unsafe { self::super::three_accum(test_case) };
    let expected = [16i8, 16, 0, 6, 21, 80];
    assert_eq!(&expected, &res.dmp_arr()[..6])
}

#[test]
fn test_mask_current_quadrant() {
    let mut arr_old = [0b0; 16];
    for i in 0..16 {
        arr_old[i] = (i as i32 * 97) | 0b1000
    }
    let mut arr = arr_old.clone();
    let mut unit_idx = unit_idx::<16>();
    unsafe {
        let rem_mask = _mm512_set1_epi32(0b1111_0111);
        let unit_idx = _mm512_loadu_epi32(unit_idx.as_ptr());
        mask_current_quadrant(0, rem_mask, arr_old.as_mut_ptr(), unit_idx);
    }
    assert!(arr_old[0..9].iter().all(|x| *x & 0b1000 == 0));
}