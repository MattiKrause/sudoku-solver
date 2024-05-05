#![cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
use std::arch::x86_64::*;
use std::mem::MaybeUninit;
use crate::Sudoku;
use crate::solver_avx512::dbg_dmp::DbgDmp;
use crate::solver_base::{LLGeneralSudokuSolver, LLSudokuSolverImpl, LLSudokuSolverInst};
use crate::work_queue::WorkQueue;
use self::util::*;

mod dbg_dmp;
#[cfg(test)]
mod tests;
mod util;

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
    //check if vals changed
    let changed: u16 = _mm512_cmp_epi32_mask::<_MM_CMPINT_NE>(old, new);
    let pos_cnt = _mm512_popcnt_epi32(new);
    let ones = splat_i32x16(1);
    let one_pos: u16 = _mm512_cmp_epi32_mask::<_MM_CMPINT_EQ>(pos_cnt, ones);
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


unsafe fn adjust_pos_counts(line: u8, col: u8, changed: u16, counts: *mut i32) ->  ([i32; 6], u32) {
    let cnt_idx = comp_cnt_indices(line, col);
    let mis_i_both = comp_mis_set(line, col);
    dec_num_count(changed, mis_i_both, counts, cnt_idx)
}

unsafe fn add_one_count_to_q(one_count_i: [i32; 6], mut oc_len: usize, content: *mut i32, rem_mask: __m512i, quad_i: __m512i, work_q: &mut WorkQueue<u16>)  {
    //while instead of for to prevent strange error
    while oc_len > 0 {
        oc_len -= 1;
        let qi = one_count_i[oc_len];
        debug_assert!(qi >= 0, "index {qi} is negative");
        let quad_offset = comp_quad_offset(qline_qcol_from_qi(qi));
        let quad_off = _mm512_set1_epi32(quad_offset);
        let cur_quad = _mm512_add_epi32(quad_i, quad_off);
        let ind = find_pos_in_one_quad(rem_mask, content, cur_quad);
        debug_assert!(u16::try_from(ind).is_ok(), "{qi} is not a i32");
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

fn all_one_count_i_are_one_count(oci: &[i32], counts: &[i32]) -> bool {
    oci.iter().map(|i| counts[*i as usize]).all(|c| c==1)
}

unsafe fn check_set(inst: &mut LLSudokuSolverInst, i: u8, val: i32, work_q: &mut WorkQueue<u16>) {
    let (line, col) = line_col_from_i(i);
    let (qline, qcol) = comp_qline_qcol((line, col));
    inst.num_counts[val as  usize][qi_from_qline_qcol((qline, qcol)) as usize] = 0;
    let i_quad_start = comp_quad_offset((qline, qcol)) as i32;
    let quad_i = load_quad_i();
    let rem_mask = _mm512_set1_epi32(comp_rem_mask(val));
    mask_current_quadrant(i_quad_start, rem_mask,inst.content.as_mut_ptr(), quad_i, work_q);
    let lane_i = compute_lane_indices(line, col);
    let changed = reduce_entropy_for_i(inst.content.as_mut_ptr(), lane_i, rem_mask, work_q);
    let (one_count_i, oci_len) = adjust_pos_counts(line, col, changed, inst.num_counts[val as usize].as_mut_ptr());
    debug_assert!(all_one_count_i_are_one_count(&one_count_i[0..(oci_len as usize)], &inst.num_counts[val as usize]));
    add_one_count_to_q(one_count_i, oci_len as usize, inst.content.as_mut_ptr(), rem_mask, quad_i, work_q);
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
            let quad_i = qi_from_qline_qcol(comp_qline_qcol(line_col_from_i(i)));
            force_dec_num_count(
                inst.num_counts.as_mut_ptr()  as *mut i32,
                (inst.content[i] ^ new_content) as u16,
                quad_i as i32
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