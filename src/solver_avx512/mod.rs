#![cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
use std::arch::x86_64::*;
use std::mem::MaybeUninit;
use crate::Sudoku;
use crate::solver_avx512::dbg_dmp::DbgDmp;
use crate::solver_base::{LLGeneralSudokuSolver, LLSudokuSolverImpl, LLSudokuSolverInst};
use crate::work_queue::WorkQueue;

mod dbg_dmp;
#[cfg(test)]
mod tests;

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
static LN_CHECK_PERM: [[u8; 8]; 9] = comp_col();
static A9X9: [i32; 9] = [0, 1, 2, 9, 10, 11, 18, 19, 20];

macro_rules! str_vec {
    ($vec:expr, $n: ident, $t: ident) => {
        let $n = $vec.dmp_arr();
        let _: &[$t] = &$n;
    };
}
macro_rules! dbg_vec {
    ($vec: ident, $t: ident) => {
        str_vec!($vec, x, $t);
        println!("{}:{} = {:?}", line!(), stringify!($vec), x);
    };
    ($vec: ident) => {
        dbg_vec!($vec, i32)
    }
}
macro_rules! dbg_vec_bin {
    ($vec: expr, $t: ident) => {
        str_vec!($vec, x, $t);
        print!("[{}]:{} = [", line!(), stringify!($vec));
        x.iter().for_each(|res| print!("{res:0b},"));
        println!("]");
    };
}

unsafe fn detect_1_pos(old: __m512i, new: __m512i, idx: __m512i, write_to: &mut WorkQueue<u16>) -> u16 {
    //checkck if vals changed
    let changed: u16 = _mm512_cmp_epi32_mask::<4>(old, new);
    let pos_cnt = _mm512_popcnt_epi32(new);
    let ones = _mm512_set1_epi32(1);
    let one_pos: u16 = _mm512_cmp_epi32_mask::<0>(pos_cnt, ones);
    let changed_one_pos = one_pos & changed;
    let idxu16 = _mm512_cvtepi32_epi16(idx);
    //store all indices where the content has changed, with one possible value remaining in the set
    write_to.write_simd256u16(idxu16, changed_one_pos);
    changed
}

unsafe fn reduce_entropy_for_i(src: *mut i32, idx: __m512i, rem_mask: __m512i, work_q: &mut WorkQueue<u16>) -> u16 {
    //load lane
    let vals = _mm512_i32gather_epi32::<4>(idx, src as *const u8);
    //remove val from bitset using rem_mask
    let vals_rem = _mm512_and_si512(vals, rem_mask);
    //store lane
    _mm512_i32scatter_epi32::<4>(src as *mut u8, idx, vals_rem);
    let changed = detect_1_pos(vals, vals_rem, idx, work_q);
    changed

}

/// accumulate the first 6 triplets and pull the results into the first 6 slots.
/// The numbers should be 8 bit
unsafe fn three_accum(mut vals: __m256i) -> __m256i {
    vals = _mm256_maskz_expand_epi8(0b111_111_111_111_111_0_111, vals);
    // shift all values one unit towards the start of the vec and add, now we have 1 + 2, 3, *
    let mut vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    // shift another unit, now we have, 1 + 2 + 3, *, *
    vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    _mm256_maskz_compress_epi8(0b001001001_001001_0_001, vals)
}

/// count the number of changes for each quadrant, subtract that from current number and save the result.
/// further use the new values to check if in any quadrant the number of places where the current value can
/// go, is one.
unsafe fn dec_num_count(changed: u16, mis_i_set: u32, cnt_ptr: *mut i32, cnt_idx_o: __m256i) -> ([i32; 6], u32) {
    let cnt_idx = _mm512_zextsi256_si512(cnt_idx_o);
    //load counts into the first 6 slots, the rest is thrash
    let cnts = _mm512_i32gather_epi32::<4>(cnt_idx, cnt_ptr as *const u8);
    let mis_i_set = mis_i_set as u32;
    //turn changed into simd reg
    let changed = _mm256_maskz_set1_epi8(changed as u32, 1);
    // changed covers 8 values, the zero in mis_i_set covers the missing value, thus a zero is added
    // in changed at the appropriate place
    let changed = _mm256_maskz_expand_epi8(mis_i_set, changed);
    //accumulate changes in triplets, one triplet represents one quadrant
    let sub_a = three_accum(changed);
    let sub_a = _mm512_cvtepi8_epi32(_mm256_extracti128_si256::<0>(sub_a));
    //subtract changes from current count
    let new_cnts = _mm512_sub_epi32(cnts, sub_a);
    //store new counts, only the first six slots matter
    _mm512_mask_i32scatter_epi32::<4>(cnt_ptr as *mut u8, 0b111_111, cnt_idx, new_cnts);
    let ncnts_changed: u16 = _mm512_cmp_epi32_mask::<4>(new_cnts, cnts);
    let ones = _mm512_set1_epi32(1);
    let is_one: u16 = _mm512_cmp_epi32_mask::<0>(ones, new_cnts);
    let mut one_idx = [-1i32; 6];
    // we want to further process quadrants, when the number of possible locations for the current val
    // changed to one, also only the first six matter
    let write_mask = is_one & ncnts_changed & 0b111_111;
    debug_assert!(write_mask.count_ones() <= 6);
    _mm256_mask_compressstoreu_epi32(one_idx.as_mut_ptr() as *mut u8, write_mask as u8, cnt_idx_o);
    (one_idx, write_mask.count_ones())
}

unsafe fn mask_current_quadrant(quad_offset: i32, rem_mask: __m512i, content: *mut i32, quad_i: __m512i, work_q: &mut WorkQueue<u16>) {
    let quad_off = _mm512_set1_epi32( quad_offset);
    let cur_quad = _mm512_add_epi32(quad_i, quad_off);
    let quad = _mm512_i32gather_epi32::<4>(cur_quad, content as *const u8);
    let quad_new = _mm512_mask_and_epi32(quad, 0b111_111_111,quad, rem_mask);
    _mm512_mask_i32scatter_epi32::<4>(content as *mut u8, 0b111_111_111, cur_quad, quad_new);
    detect_1_pos(quad, quad_new, cur_quad, work_q);
}

/// the quadrant given through the first 9 slots in quad_indices has only one possible location
/// given by location & rem_mask = 1, find the index of that location
unsafe fn find_pos_in_one_quad(rem_mask: __m512i, ptr: *mut i32, quad_indices: __m512i) -> i32 {
    //load quad
    let mut quad = _mm512_i32gather_epi32::<4>(quad_indices, ptr as *mut u8);
    //rem mask masks the given value out, we need to invert the mask first
    quad = _mm512_andnot_si512(rem_mask, quad);
    // the slots have any one in them, then they aer larger than zero
    // the number of ones is between 0 and 1 because of the masking
    let el: u16 = _mm512_cmpgt_epi32_mask(quad, _mm512_setzero_epi32());
    let mut i: MaybeUninit<i32> = MaybeUninit::uninit();
    let write_mask = el & 0b1111_1111_1;
    debug_assert_eq!(write_mask.count_ones(), 1);
    // only one location should be possible
    _mm512_mask_i32scatter_epi32::<4>(ptr as *mut u8, write_mask, quad_indices, quad);
    _mm512_mask_compressstoreu_epi32(i.as_mut_ptr() as *mut u8, write_mask, quad_indices);
    i.assume_init()
}

unsafe fn compute_lane_indices(line: u8, col: u8) -> __m512i {
    let col_i = {
        let col =  _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[col as usize].as_ptr() as *const i8);
        let col_off = _mm_set1_epi8((line * 9) as i8);
        let col_i = _mm_add_epi8(col, col_off);
        _mm256_cvtepi8_epi32(col_i)
    };
    let line_i = {
        let line = _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[line as usize].as_ptr() as *const i8);
        //dbg_vec!(line, i8);
        let line = _mm256_cvtepi8_epi32(line);
        let line_mul = _mm256_set1_epi32(9);
        let line = _mm256_mullo_epi32(line, line_mul);
        let line_off = _mm256_set1_epi32(col as i32);
        _mm256_add_epi32(line, line_off)
    };
    let col_i = _mm512_zextsi256_si512(col_i);
    let lane_i = _mm512_inserti64x4::<1>(col_i, line_i);
    lane_i
}

unsafe fn comp_cnt_indices(line: u8, col: u8) -> __m256i {
    static IDX012: [i32; 3] = [0, 1, 2];
    let vec012 = _mm_maskz_loadu_epi32(0b111, IDX012.as_ptr());
    let line_offset = _mm_set1_epi32(((line / 3) * 3) as i32);
    let three_vec = _mm_set1_epi32(3);
    let col_vec = _mm_add_epi32(vec012, line_offset);
    let line_starts = _mm_mullo_epi32(three_vec, vec012);
    let col_offset = _mm_set1_epi32((col / 3) as i32);
    let line_vec = _mm_add_epi32(line_starts, col_offset);
    let col_vec = _mm256_zextsi128_si256(col_vec);
    let both_vec = _mm256_inserti128_si256::<1>(col_vec, line_vec);
    _mm256_maskz_compress_epi32(0b1110111, both_vec)
}

unsafe fn adjust_pos_counts(line: u8, col: u8, changed: u16, counts: *mut i32) ->  ([i32; 6], u32) {
    static MIS_SET: [u16; 9] = [
        0b111_111_110, 0b111_111_101, 0b111_111_011, 0b111_110_111,
        0b111_101_111, 0b111_011_111, 0b110_111_111, 0b101_111_111, 0b011_111_111
    ];
    let cnt_idx = comp_cnt_indices(line, col);
    let mis_i_both = ((MIS_SET[line as usize] as u32) << 9) | (MIS_SET[col as usize] as u32);
    dec_num_count(changed, mis_i_both, counts, cnt_idx)
}

unsafe fn add_one_count_to_q(one_count_i: [i32; 6], mut oc_len: usize, content: *mut i32, rem_mask: __m512i, quad_i: __m512i, work_q: &mut WorkQueue<u16>)  {
    //while instead of for to prevent strange error
    while oc_len > 0 {
        oc_len -= 1;
        let q = one_count_i[oc_len];
        debug_assert!(q >= 0, "index {q} is negative");
        let ind_quad = (q / 3) * 3 * 9 + (q % 3) * 3;
        let quad_off = _mm512_set1_epi32(ind_quad as i32);
        let cur_quad = _mm512_add_epi32(quad_i, quad_off);
        let ind = find_pos_in_one_quad(rem_mask, content, cur_quad);
        debug_assert!(u16::try_from(ind).is_ok(), "{q} is not a i32");
        work_q.push(ind as u16);
    }
}

unsafe fn force_dec_num_count(counts_ptr: *mut i32, remaining: u16, quad_index: i32) {
    static UNIT_IDX: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    let count_base = _mm_maskz_loadu_epi8(0x1FF, UNIT_IDX.as_ptr() as *const i8);
    let count_base = _mm512_cvtepi8_epi32(count_base);
    let nine = _mm512_set1_epi32(9);
    let count_num_base = _mm512_mullo_epi32(count_base, nine);
    let count_off = _mm512_set1_epi32(quad_index);
    let count_idx = _mm512_add_epi32(count_num_base, count_off);
    let counts = _mm512_i32gather_epi32::<4>(count_idx, counts_ptr as *const u8);
    let ones = _mm512_set1_epi32(1);
    let new_counts = _mm512_mask_sub_epi32(counts, remaining, counts, ones);
    _mm512_mask_i32scatter_epi32::<4>(counts_ptr as *mut u8, remaining, count_idx, new_counts);
}

fn comp_line_col(i: u8) -> (u8, u8) {
    (i / 9, i % 9)
}

unsafe fn check_set(inst: &mut LLSudokuSolverInst, i: u8, val: i32, work_q: &mut WorkQueue<u16>) {
    let (line, col) = comp_line_col(i);

    let (quad_ln, quad_col) = ((line / 3) * 3, col / 3);
    inst.num_counts[val as  usize][(quad_ln + quad_col) as usize] = 0;
    let i_quad_start = quad_ln * 9 + quad_col * 3;
    let quad_i = _mm512_maskz_loadu_epi32(0b111_111_111, A9X9.as_ptr());
    let rem_mask = _mm512_set1_epi32(!(1 << val));
    mask_current_quadrant(i_quad_start as i32, rem_mask,inst.content.as_mut_ptr(), quad_i, work_q);
    let lane_i = compute_lane_indices(line, col);
    let changed = reduce_entropy_for_i(inst.content.as_mut_ptr(), lane_i, rem_mask, work_q);
    let (one_count_i, oci) = adjust_pos_counts(line, col, changed, inst.num_counts[val as usize].as_mut_ptr());
    debug_assert!((&one_count_i[0..(oci as usize)]).iter().map(|i| inst.num_counts[val as usize][*i as usize]).all(|c| c==1));
    add_one_count_to_q(one_count_i, oci as usize, inst.content.as_mut_ptr(), rem_mask, quad_i, work_q);
}

pub type Avx512SudokuSolver = LLGeneralSudokuSolver<Avx512SudokuSolverImpl>;
pub struct Avx512SudokuSolverImpl;
impl Default for Avx512SudokuSolverImpl {
    fn default() -> Self {
        Self
    }
}

impl LLSudokuSolverImpl for Avx512SudokuSolverImpl {
    fn tell_value_i(&mut self, inst: &mut LLSudokuSolverInst, i: u8, val: u8, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        let i = i as usize;
        let new_content = 1 << (val - 1);
        unsafe {
            let quad_offset = ((i / 9) / 3) * 3 + ((i % 9) / 3);
            force_dec_num_count(
                inst.num_counts.as_mut_ptr()  as *mut i32,
                (inst.content[i] ^ new_content) as u16,
                quad_offset as i32
            );
        }
        if inst.content[i] & new_content == 0 {
            return Err(());
        }

        inst.content[i as usize] = new_content;
        self.tell_at_ind(inst,i as u8, sudoku, work_q)
    }
    fn tell_at_ind(&mut self, inst: &mut LLSudokuSolverInst, i: u8, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        let val = inst.content[i as usize];
        let val_set = val.trailing_zeros() as i32;
        if val == 0 || val_set > 9 {
            return Err(());
        }
        inst.content[i as usize] = 0;
        let old_len = work_q.len();
        sudoku.set_i(i, val_set as u16 + 1);
        let res = unsafe {
            check_set(inst, i, val_set, work_q);
            if old_len != work_q.len() {
                dbg!(&work_q.as_slice()[old_len..]);
                dbg!(i, work_q.as_slice());
            }
            Ok(())
        };
        res
    }
}