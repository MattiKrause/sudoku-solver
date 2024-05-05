use core::arch::x86_64::*;
use core::ops::{Add, Div, Mul, Rem};


const fn comp_col() -> [[u8; 8]; 9] {
    let mut res = [[0; 8]; 9];
    let mut i = 0;
    while i < 9 {
        let mut wr_i = 0;
        let mut resi = [0; 8];
        let mut j = 0;
        while j < 9 {
            if j != i {
                resi[wr_i] = j as u8;
                wr_i += 1;
            }
            j += 1;
        }
        res[i] = resi;
        i += 1;
    }
    res
}
pub static LN_CHECK_PERM: [[u8; 8]; 9] = comp_col();
pub static QUAD_INDEX: [i32; 9] = [0, 1, 2, 9, 10, 11, 18, 19, 20];
static MIS_SET: [u16; 9] = [
    0b111_111_110, 0b111_111_101, 0b111_111_011, 0b111_110_111,
    0b111_101_111, 0b111_011_111, 0b110_111_111, 0b101_111_111, 0b011_111_111
];

pub fn line_col_from_i<T: Div<Output = T> + Rem<Output = T> + From<u8> + Copy>(i: T) -> (T, T) {
    (i / T::from(9), i % T::from(9))
}

pub fn i_from_line_col<T: Add<Output = T> + Mul<Output = T> + From<u8>>((line, col): (T, T)) -> T {
    line * T::from(9) + col
}

pub fn comp_qline_qcol<T: Div<Output=T> + From<u8>>((line, col): (T, T)) -> (T,  T) {
    (line / T::from(3), col / T::from(3))
}

pub fn qline_qcol_from_qi<T: Div<Output = T> + Rem<Output = T> + From<u8> + Copy>(qi: T) -> (T,  T) {
    (qi / T::from(3), qi % T::from(3))
}

pub fn qi_from_qline_qcol<T: Add<Output=T> + Mul<Output = T> + From<u8>>((qline, qcol): (T, T)) -> T {
    qline * T::from(3) + qcol
}

pub fn comp_quad_offset<T: Add<Output = T> + Mul<Output = T> + From<u8>>((qline, qcol): (T, T)) -> T {
    //qline * 3 computes the start line of a  quadrant
    i_from_line_col((qline * T::from(3), qcol * T::from(3)))
}

pub fn comp_mis_set(line: u8, col: u8) -> u32 {
    let mis_line = MIS_SET[line as usize] as u32;
    let mis_col = MIS_SET[col as usize] as u32;
    mis_line << 9 | mis_col
}

pub unsafe fn load_quad_i() -> __m512i {
    _mm512_maskz_loadu_epi32(0b111_111_111, QUAD_INDEX.as_ptr())
}

pub fn comp_rem_mask(val: i32) ->  i32 {
    !(1 << val)
}

/// accumulate the first 6 triplets and pull the results into the first 6 slots.
/// The numbers should be 8 bit
pub unsafe fn three_accum(mut vals: __m256i) -> __m256i {
    vals = _mm256_maskz_expand_epi8(0b111_111_111_111_111_0_111, vals);
    // shift all values one unit towards the start of the vec and add, now we have 1 + 2, 3, *
    let mut vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    // shift another unit, now we have, 1 + 2 + 3, *, *
    vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    _mm256_maskz_compress_epi8(0b001001001_001001_0_001, vals)
}

/// computes lane indices that need to be checked,  return in format (\[col;8\], (\[line; 8\]))
pub unsafe fn compute_lane_indices(line: u8, col: u8) -> __m512i {
    //col indices computed as line * 9 ü col_offset
    let col_i = {
        let col_offset =  _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[col as usize].as_ptr() as *const i8);
        let line_base = _mm_set1_epi8((line * 9) as i8);
        let col_i = _mm_add_epi8(col_offset, line_base);
        _mm256_cvtepi8_epi32(col_i)
    };
    //line indices computed as (line_offset * 9) + col
    let line_i = {
        let line_offset = _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[line as usize].as_ptr() as *const i8);
        let line_offset = _mm256_cvtepi8_epi32(line_offset);
        let nine = _mm256_set1_epi32(9);
        let line_offset9 = _mm256_mullo_epi32(line_offset, nine);
        let col = _mm256_set1_epi32(col as i32);
        _mm256_add_epi32(line_offset9, col)
    };
    let col_i = _mm512_zextsi256_si512(col_i);
    let lane_i = _mm512_inserti64x4::<1>(col_i, line_i);
    lane_i
}

/// computes the indices of the count variables that are affected by a change at (line, col)
/// indices are
pub unsafe fn comp_cnt_indices(line: u8, col: u8) -> __m256i {
    static IDX012: [i32; 3] = [0, 1, 2];
    let (qline, qcol) = comp_qline_qcol((line, col));
    let vec012 = _mm_maskz_loadu_epi32(0b111, IDX012.as_ptr());
    let line_base = _mm_set1_epi32((qline * 3) as i32);
    let three_vec = _mm_set1_epi32(3);
    //qline * 3 + (0, 1, 2)
    let col_vec = _mm_add_epi32(line_base, vec012);
    let line_offset = _mm_mullo_epi32(three_vec, vec012);
    let qcol_vec = _mm_set1_epi32(qcol as i32);
    //(0, 1, 2) * 3 + qcol
    let line_vec = _mm_add_epi32(line_offset, qcol_vec);
    let col_vec = _mm256_zextsi128_si256(col_vec);
    let both_vec = _mm256_inserti128_si256::<1>(col_vec, line_vec);
    // the relevant results are stored in the first three slots of each vector, if we simply
    // concatenate them, then the 4th slot still contains garbage. This is why we compress them.
    _mm256_maskz_compress_epi32(0b1110111, both_vec)
}

pub unsafe fn splat_i32x16(i: i32) -> __m512i {
    _mm512_set1_epi32(i)
}