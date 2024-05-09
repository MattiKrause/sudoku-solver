#![cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]

use std::arch::x86_64::*;
use std::mem::MaybeUninit;

use crate::solver_avx512::dbg_dmp::DbgDmp;
use crate::solver_base::{CellIndices, FlatIndex, FlatQuadrantIndex, get_quad_offset, LLGeneralSudokuSolver, LLSudokuSolverImpl, LLSudokuSolverInst, QuadrantIndices, SudokuValue};
use crate::Sudoku;
use crate::work_queue::WorkQueue;

use self::util::*;

mod dbg_dmp;
#[cfg(test)]
mod tests;
mod util;

// Algorithm:
// The Sudoku grid is implemented as a 9x9 grid of (bit)sets of the possible values for that cell
// the 9 3x3 sub-grids are called quadrants
// A second grid contains the sudoku numbers mapped to the amount of cells, that the number can still appear in per quadrant
//
// If a value is set at a specific cell(from now on called value cell) then the following happens
// 1. The value is removed from all affected cell's set, i.e. cells that share a row, column or quadrant with the value cell
// 2. Any affected cell whose set has *reached* a size of 1 is added to the work queue
// 3. The counts for all the affected quadrants are updated
// 4. if the count reaches 1, then the quadrant is investigated for the cell which can still contain the value, that cell is then added to the work queue


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
    ($vec: ident, $t: ident, $num: literal) => {
        str_vec!($vec, x, $t);
        println!("{}:{} = {:?}", line!(), stringify!($vec), &x[..$num]);
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

/// Find all cells whose set size is 1 and whose value has changed and add them to the work queue
/// returns a mask of all the elements whose value changed
unsafe fn detect_set_size_1(old: I32x16, new: I32x16, idx: I32x16, write_to: &mut WorkQueue<u16>) -> u16 {
    //check if vals changed
    let changed: u16 = _mm512_cmp_epi32_mask::<_MM_CMPINT_NE>(old, new);
    let set_size = _mm512_popcnt_epi32(new);
    let ones = splat_i32x16(1);
    let set_size1_mask: u16 = _mm512_cmp_epi32_mask::<_MM_CMPINT_EQ>(set_size, ones);
    let reached_set_size1_mask = set_size1_mask & changed;
    let idxu16 = _mm512_cvtepi32_epi16(idx);
    //store all indices where the content has changed, with one possible value remaining in the set
    write_to.write_simd256u16(idxu16, reached_set_size1_mask);
    changed
}

/// removes the value from the sets with the given indices using the remove mask
/// Then adds the cells whose set size has reached 1 to the work queue
/// Returns a mask of the elements whose value has changed
unsafe fn remove_from_set_x16(src: &mut [i32; 81], idx: IndexI32x16, remove_mask: I32x16, work_q: &mut WorkQueue<u16>) -> u16 {
    #[cfg(debug_assertions)]
    {
        validate_cross_indices(idx);
    }
    //load lane
    let vals = gather_i32x16(src, idx);
    //remove val from bitset using rem_mask
    let vals_new = _mm512_and_si512(vals, remove_mask);

    //store lane
    scatter_i32x16(src, idx, vals_new);
    detect_set_size_1(vals, vals_new, idx, work_q)
}

/// like [remove_from_set_x16] just for 9 items
unsafe fn remove_from_set_x9(src: &mut [i32; 81], idx: IndexI32x9, remove_mask: I32x9, work_queue: &mut WorkQueue<u16>) -> u16 {
    let vals = gather_i32x9(src, idx);
    let vals_new = _mm512_mask_and_epi32(vals, 0b111_111_111, vals, remove_mask);
    scatter_i32x9(src, idx, vals_new);
    detect_set_size_1(vals, vals_new, idx, work_queue)
}

unsafe fn mask_current_quadrant(quad_offset: FlatIndex, remove_mask: I32x16, content: &mut [i32; 81], quad_i: IndexI32x9, work_q: &mut WorkQueue<u16>) {
    let quad_off = splat_i32x16(i32::from(quad_offset.get()));
    let cur_quad: IndexI32x9 = _mm512_add_epi32(quad_i, quad_off);
    let _ = remove_from_set_x9(content, cur_quad, remove_mask, work_q);
}

/// count the number of changes for each quadrant, subtract that from current number and save the result.
/// Find the quadrants where only one cell can contain the value and add these quadrants to the list
unsafe fn decrease_num_count(changed: u16, missing_index_set: u32, num_count: &mut [i32; 9], cnt_idx_o: __m256i) -> ([i32; 6], u32) {
    let cnt_idx = cnt_idx_o;
    //load counts into the first 6 slots, the rest is trash
    let cnts = _mm256_i32gather_epi32::<4>(num_count.as_ptr(), cnt_idx);
    //turn changed into simd reg, so that all slots, where the mask is 1, the slot contains a 1
    let changed = _mm256_maskz_set1_epi8(changed as u32, 1);
    // changed covers 16 values(8 for row and 8 for column), the zero in mis_i_set covers the missing value, thus a zero is added
    // in changed at the appropriate place
    // the first 18 places of vector are now filled with actual data
    let changed = _mm256_maskz_expand_epi8(missing_index_set, changed);
    //accumulate changes in triplets, one triplet represents one quadrant
    let sub_a = accumulate_triplets(changed);
    // sub_a now only contains data for six quadrants
    let sub_a = _mm256_extracti128_si256::<0>(sub_a);
    let sub_a = _mm256_cvtepi8_epi32(sub_a);
    //subtract changes from current count
    let new_cnts = _mm256_sub_epi32(cnts, sub_a);
    //store new counts, only the first six slots matter
    _mm512_mask_i32scatter_epi32::<4>(num_count.as_mut_ptr() as *mut u8, 0b111_111, _mm512_zextsi256_si512(cnt_idx), _mm512_zextsi256_si512(new_cnts));
    let ncnts_changed = _mm256_cmpneq_epi32_mask(new_cnts, cnts);
    let ones = _mm256_set1_epi32(1);
    let is_one = _mm256_cmpeq_epi32_mask(ones, new_cnts);
    let mut one_idx = [-1i32; 6];
    // we want to further process quadrants, when the number of possible locations for the current val
    // changed to one, also only the first six matter
    let write_mask = is_one & ncnts_changed & 0b111_111;
    debug_assert!(write_mask.count_ones() <= 6);
    _mm256_mask_compressstoreu_epi32(one_idx.as_mut_ptr() as *mut u8, write_mask, cnt_idx_o);
    (one_idx, write_mask.count_ones())
}

/// the quadrant given through the first 9 slots in quad_indices has only one possible location
/// given by location & rem_mask = 1, find the index of that location
unsafe fn find_pos_in_one_quad(rem_mask: I32x16, ptr: &mut  [i32; 81], quad_indices: IndexI32x9) -> i32 {
    //load quad
    let mut quad = gather_i32x9(ptr, quad_indices);
    //rem mask masks the given value out, we need to invert the mask first
    quad = _mm512_andnot_si512(rem_mask, quad);
    // the slots have any one in them, then they aer larger than zero
    // the number of ones is between 0 and 1 because of the masking
    let el: u16 = _mm512_cmpgt_epi32_mask(quad, _mm512_setzero_epi32());
    let mut i: MaybeUninit<i32> = MaybeUninit::uninit();
    let write_mask = el & 0b1111_1111_1;
    debug_assert_eq!(write_mask.count_ones(), 1);
    // only one location should be possible
    _mm512_mask_i32scatter_epi32::<4>(ptr.as_mut_ptr() as *mut u8, write_mask, quad_indices, quad);
    _mm512_mask_compressstoreu_epi32(i.as_mut_ptr() as *mut u8, write_mask, quad_indices);
    i.assume_init()
}


unsafe fn adjust_pos_counts(cell_indices: CellIndices, indices_with_change_mask: u16, num_counts: &mut [i32; 9]) ->  ([i32; 6], u32) {
    let cnt_idx = comp_cnt_indices(QuadrantIndices::from(cell_indices));
    let mis_i_both = find_missing_index_mask(cell_indices);
    decrease_num_count(indices_with_change_mask, mis_i_both, num_counts, cnt_idx)
}

unsafe fn add_one_count_to_q(one_count_i: &[i32], content: &mut [i32; 81], rem_mask: I32x16, quad_i: IndexI32x9, work_q: &mut WorkQueue<u16>)  {
    //while instead of for to prevent strange error
    for qi in one_count_i {
        debug_assert!(*qi >= 0, "index {qi} is negative");
        let qi = FlatQuadrantIndex::new(*qi  as u8).unwrap();
        let quad_offset = get_quad_offset(QuadrantIndices::from(qi));
        let quad_off = splat_i32x16(i32::from(quad_offset.get()));
        let cur_quad = _mm512_add_epi32(quad_i, quad_off);
        let ind = find_pos_in_one_quad(rem_mask, content, cur_quad);
        debug_assert!(u16::try_from(ind).is_ok(), "{qi:?} is not a i32");
        work_q.push(ind as u16);
    }
}

/// Decreases the counts of the values given in remaining_mask
unsafe fn force_dec_num_count(all_counts: &mut [[i32; 9]; 9], remaining_mask: u16, quad_index: FlatQuadrantIndex) {
    debug_assert!(remaining_mask & 0b111_111_111 == remaining_mask);

    static COUNT_OFFSETS: [u8; 9] = [0, 1 * 9, 2 * 9, 3 * 9, 4 * 9, 5 * 9, 6 * 9, 7 * 9, 8 * 9];
    let count_base = _mm_maskz_loadu_epi8(0x1FF, COUNT_OFFSETS.as_ptr() as *const i8);
    let count_base: IndexI32x9 = _mm512_cvtepi8_epi32(count_base);
    let count_off: I32x16 = splat_i32x16(quad_index.get() as i32);
    let count_idx: IndexI32x9 = _mm512_add_epi32(count_base, count_off);
    let counts = gather_i32x9(all_counts.flatten(), count_idx);
    let ones = splat_i32x16(1);
    let new_counts = _mm512_mask_sub_epi32(counts, remaining_mask, counts, ones);
    scatter_i32x9(all_counts.flatten_mut(), count_idx, new_counts);
}

fn all_one_count_i_are_one_count(oci: &[i32], counts: &[i32]) -> bool {
    oci.iter().map(|i| counts[*i as usize]).all(|c| c==1)
}

unsafe fn check_set(inst: &mut LLSudokuSolverInst, i: FlatIndex, value: SudokuValue, work_q: &mut WorkQueue<u16>) {
    let cell_indices = CellIndices::from(i);
    let quad_indices = QuadrantIndices::from(cell_indices);
    let quad_index = FlatQuadrantIndex::from(quad_indices);
    inst.num_counts[value.as_0based_idx()][quad_index.as_idx()] = 0;
    let rem_mask = splat_i32x16(!i32::from(value.as_mask_0based()));

    let quadrant_offset = get_quad_offset(quad_indices);
    let quad_i = load_quad_indices();
    mask_current_quadrant(quadrant_offset, rem_mask, &mut inst.content, quad_i, work_q);

    let lane_i = compute_lane_indices(cell_indices);
    // indices_with_change contains a mask of the indices whose value has been changed
    let indices_with_change = remove_from_set_x16(&mut inst.content, lane_i, rem_mask, work_q);

    let (one_count_i, oci_len) = adjust_pos_counts(cell_indices, indices_with_change, &mut inst.num_counts[value.as_0based_idx()]);
    debug_assert!(all_one_count_i_are_one_count(&one_count_i[0..(oci_len as usize)], &inst.num_counts[value.as_0based_idx()]));
    add_one_count_to_q(&one_count_i[..(oci_len as usize)], &mut inst.content, rem_mask, quad_i, work_q);
}

pub type Avx512SudokuSolver = LLGeneralSudokuSolver<Avx512SudokuSolverImpl>;
pub struct Avx512SudokuSolverImpl;
impl Default for Avx512SudokuSolverImpl {
    fn default() -> Self {
        Self
    }
}

impl LLSudokuSolverImpl for Avx512SudokuSolverImpl {
    fn force_set_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        let new_content = i32::from(val.as_mask_0based());
        if inst.content[i.as_idx()] & new_content == 0 {
            return Err(());
        }
        unsafe {
            let quad_index = FlatQuadrantIndex::from(QuadrantIndices::from(CellIndices::from(i)));
            force_dec_num_count(
                &mut inst.num_counts,
                (inst.content[i.as_idx()] ^ new_content) as u16,
                quad_index
            );
        }


        inst.content[i.as_idx()] = new_content;
        self.process_index(inst, i, sudoku, work_q)
    }
    fn process_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        let val_set = inst.content[i.as_idx()];
        let value = val_set.trailing_zeros();
        let value = SudokuValue::new_0based(u8::try_from(value).map_err(|_|())?).ok_or(())?;
        inst.content[i.as_idx()] = 0;
        let old_len = work_q.len();
        sudoku[i] = Some(value);
        let res = unsafe {
            check_set(inst, i, value, work_q);
            if old_len != work_q.len() {
                dbg!(&work_q.as_slice()[old_len..]);
                dbg!(i, work_q.as_slice());
            }
            Ok(())
        };
        res
    }
}