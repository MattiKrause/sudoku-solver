use std::simd::{Mask, SimdConstPtr, SimdInt, SimdMutPtr, SimdPartialEq, SimdPartialOrd, SimdUint, ToBitMask};
use crate::solver_base::{CellIndices, FlatIndex, FlatQuadrantIndex, get_quad_offset, LLSudokuSolverImpl, QuadrantIndices, SudokuValue};
use crate::Sudoku;
use crate::work_queue::BitMaskWorkQueue;

type LLSudokuSolverInst = crate::solver_base::LLSudokuSolverInst<u16, u8>;

#[inline(never)]
fn force_dec_num_count(count: &mut [[u8; 9]; 9], remaining_values: u16, quad_index: FlatQuadrantIndex) {
    for i in 0..9 {
        count[i as usize][quad_index.as_idx()] -= ((remaining_values >> i) & 1) as u8;
    }
}

#[inline(never)]
fn generate_mask(i: FlatIndex) -> u128{
    static COLUMN_MASK: u128 = 0b111_111_111;
    static ROW_MASK: u128 = (1u128 << (0 * 9)) | (1 << (1 * 9)) | (1 << (2 * 9)) | (1 << (3 * 9)) | (1 << (4 * 9)) | (1 << (5 * 9)) | (1 << (6 * 9)) | (1 << (7 * 9)) | (1 << (8 * 9));
    static QUADRANT_MASK: u128 = 0b111u128 | (0b111 << 9) | (0b111 << 2 * 9);
    let cell_indices = CellIndices::from(i);
    let quadrant_offset = get_quad_offset(QuadrantIndices::from(cell_indices));
    (COLUMN_MASK << (cell_indices.row.get() * 9)) | (ROW_MASK << (cell_indices.column.get())) | (QUADRANT_MASK << quadrant_offset.get())
}

#[inline(never)]
fn check_set(inst: &mut LLSudokuSolverInst, i: FlatIndex, value: SudokuValue, work_q: &mut BitMaskWorkQueue) {
    let active_mask = generate_mask(i);
    let remove_mask = !value.as_mask_0based();

    let mut changed_mask = 0u128;

    for i in 0..81 {
        let mut mask = inst.content_mut()[i];
        let new_mask = mask & remove_mask;
        let changed = new_mask != mask;
        //let is_one = new_mask.is_power_of_two();

        let is_one = {
            let eax = new_mask - 1;
            let edi = (new_mask ^ eax);
            edi > eax
        };

        let is_set = ((active_mask >> i) as u32 & 1) == 1;
        work_q.0 |= ((changed & is_one & is_set) as u128) << i;
        changed_mask |= ((changed & is_set) as u128) << i;
        inst.content_mut()[i] = if is_set { new_mask } else { mask };
    }
}

#[inline(never)]
fn check_set1(inst: &mut LLSudokuSolverInst, i: FlatIndex, value: SudokuValue, work_q: &mut BitMaskWorkQueue) -> u128 {
    let active_mask = generate_mask(i);
    let remove_mask = !value.as_mask_0based();
    let mut changed_mask: u128;

    {
        let data_lo = active_mask as u64;
        let mut data = std::simd::u16x64::from_slice(&inst.content()[0..64]);
        let mask_out = std::simd::Simd::splat(remove_mask);
        let mask = std::simd::Mask::from_bitmask(data_lo);
        let new_data = mask.select(data & mask_out, data);
        new_data.copy_to_slice(&mut inst.content_mut()[0..64]);

        let changed = data.simd_ne(new_data).to_bitmask();
        let is_one = {
            let eax = new_data - std::simd::Simd::splat(1);
            let edi = (new_data ^ eax);
            edi.simd_gt(eax).to_bitmask() & changed
        };
        work_q.0 |= is_one as u128;
        changed_mask = changed as u128;
    }

    {
        let data_high = (active_mask >> 64) as u32;
        let mut data = std::simd::u16x32::from_slice(&inst.content_aligned()[64..96]);
        let mask_out = std::simd::Simd::splat(remove_mask);
        let mask = std::simd::Mask::from_bitmask(data_high);
        let new_data = mask.select(data & mask_out, data);
        new_data.copy_to_slice(&mut inst.content_aligned_mut()[64..96]);

        let changed = data.simd_ne(new_data).to_bitmask();
        let is_one = {
            let eax = new_data - std::simd::Simd::splat(1);
            let edi = (new_data ^ eax);
            edi.simd_gt(eax).to_bitmask() & changed
        };
        work_q.0 |= (is_one as u128) << 64;
        changed_mask = (changed as u128) <<64;
    }
    changed_mask
}

#[inline(never)]
fn adjust_count(num_count: &mut [u8; 9], change_mask: u128) {
    static MASK_LOW: u64 = 0b000_000_111_000_000_111_000_000_111;
    static INDICES: [u8; 9] = [0, 3, 6, 27, 30, 33, 0, 3, 6];

    let mut low = change_mask as u64;
    for i in 0..6 {
        num_count[i] -= (low & (MASK_LOW << INDICES[i])).count_ones() as u8;
    }

    let high = (low >> 54) as u32 | (((change_mask >> 64) as u32) << 10);
    for i in 6..9 {
        num_count[i] -= (high & ((MASK_LOW as u32) << INDICES[i])).count_ones() as u8;
    }
}


#[derive(Default)]
pub struct SolverFullLoop;

impl LLSudokuSolverImpl for SolverFullLoop {
    type Mask = u16;
    type Count = u8;
    type WorkQueue = BitMaskWorkQueue;

    fn force_set_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut BitMaskWorkQueue) -> Result<(), ()> {
        let new_content = val.as_mask_0based();
        if inst.content()[i.as_idx()] & new_content == 0 {
            return Err(());
        }
        let quad_index = FlatQuadrantIndex::from(QuadrantIndices::from(CellIndices::from(i)));
        let remaining_values = inst.content()[i.as_idx()];
        force_dec_num_count(inst.num_counts_mut(), remaining_values, quad_index);
        inst.content_mut()[i.as_idx()] = new_content;
        self.process_index(inst, i, sudoku, work_q)
    }

    fn process_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, sudoku: &mut Sudoku, work_q: &mut BitMaskWorkQueue) -> Result<(), ()> {
        let val_set = inst.content()[i.as_idx()];
        let value = val_set.trailing_zeros();
        let value = SudokuValue::new_0based(u8::try_from(value).map_err(|_|())?).ok_or(())?;
        debug_assert!(val_set == value.as_mask_0based(), "v: {value:?}, mask: {val_set:0b}");
        inst.content_mut()[i.as_idx()] = 0;
        sudoku[i] = Some(value);
        let changed = check_set1(inst, i, value, work_q);
        adjust_count(&mut inst.num_counts_mut()[value.as_0based_idx()], changed);
        Ok(())
    }
}