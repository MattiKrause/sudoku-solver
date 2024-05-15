mod gen_mask;

use std::simd::{SimdPartialOrd, ToBitMask};

use crate::solver_base::{CellIndex, CellIndices, FlatIndex, FlatQuadrantIndex, GeneralSudokuSolver, get_quad_offset, Indices, QuadrantIndex, QuadrantIndices, SudokuValue};
use crate::solver_full_loop::gen_mask::generate_mask;
use crate::Sudoku;
use crate::work_queue::{BitMaskWorkQueue, WorkQueue};

type LLSudokuSolverInst = crate::solver_base::LLSudokuSolverInst<u16, u8>;
type Board = [u16; 96];


#[inline(never)]
fn check_set1(content: &mut Board, i: FlatIndex, value: SudokuValue, work_q: &mut BitMaskWorkQueue) {
    let active_mask = generate_mask(i);
    let remove_mask = !value.as_mask_0based();

    {
        let data_lo = active_mask as u64;
        let data = std::simd::u16x64::from_slice(&content[0..64]);
        let mask_out = std::simd::Simd::splat(remove_mask);
        let mask = std::simd::Mask::from_bitmask(data_lo);
        let new_data = mask.select(data & mask_out, data);
        new_data.copy_to_slice(&mut content[0..64]);

        let is_one = {
            let eax = new_data - std::simd::Simd::splat(1);
            let edi = new_data ^ eax;
            edi.simd_gt(eax).to_bitmask()
        };
        work_q.0 |= is_one as u128;
    }

    {
        let data_high = (active_mask >> 64) as u32;
        let data = std::simd::u16x32::from_slice(&content[64..96]);
        let mask_out = std::simd::Simd::splat(remove_mask);
        let mask = std::simd::Mask::from_bitmask(data_high);
        let new_data = mask.select(data & mask_out, data);
        new_data.copy_to_slice(&mut content[64..96]);

        let is_one = {
            let eax = new_data - std::simd::Simd::splat(1);
            let edi = new_data ^ eax;
            edi.simd_gt(eax).to_bitmask()
        };
        work_q.0 |= (is_one as u128) << 64;
    }
}

#[inline(never)]
pub fn seek_quad_single_value(board: &mut Board, wq: &mut u128) {
    let indices = [0, 3, 6, 27, 30, 33, 54, 57, 60];
    let offsets = [0, 1, 2, 9, 9 + 1, 9 + 2, 18, 18 + 1, 18 + 2];
    for idx in indices {
        let mut l1 = 0;
        let mut l2 = 0;
        for o in offsets {
            if (idx + o) >= 81 {
                unsafe { std::hint::unreachable_unchecked() }
            }
            let m = board[idx + o];
            l2 |= l1 & m;
            l1 |= m;
        }
        let e1 = l1 & !l2;
        for o in offsets {
            if (idx + o) >= 81 {
                unsafe { std::hint::unreachable_unchecked() }
            }
            let m = board[idx + o];
            let m2 = m & e1;
            board[idx + o] = if m2 > 0 { m2 } else { m };
            *wq |= ((m2 > 0) as u128) << (idx + o);
        }
    }
}


#[derive(Default)]
pub struct SolverFullLoopImpl;

pub struct SolverFullLoop {
    sudoku: Sudoku,
    content: Box<[u16; 96]>,
    work_queue: BitMaskWorkQueue,
    filled_pos: u8,
}

#[inline(never)]
fn set_value(content: &mut Board, value: SudokuValue, idx: FlatIndex, sudoku: &mut Sudoku, work_q: &mut BitMaskWorkQueue) {
    content[idx.as_idx()] = 0;
    sudoku[idx] = Some(value);
    check_set1(content, idx, value, work_q);
}

fn force_set_index(content: &mut Board, i: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut BitMaskWorkQueue) -> Result<(), ()> {
    let new_content = val.as_mask_0based();
    if content[i.as_idx()] & new_content == 0 {
        return Err(());
    }
    set_value(content, val, i, sudoku, work_q);
    Ok(())
}

#[inline(never)]
fn process_index(content: &mut Board, i: FlatIndex, sudoku: &mut Sudoku, work_q: &mut BitMaskWorkQueue) -> Result<(), ()> {
    let val_set = content[i.as_idx()];
    let value = val_set.trailing_zeros();
    let value = SudokuValue::new_0based(u8::try_from(value).map_err(|_| ())?).ok_or(())?;
    debug_assert!(val_set == value.as_mask_0based(), "v: {value:?}, mask: {val_set:0b}");
    set_value(content, value, i, sudoku, work_q);
    Ok(())
}

impl GeneralSudokuSolver for SolverFullLoop {
    fn new() -> Self {
        Self {
            sudoku: Sudoku::new(),
            content: Box::new([0b111_111_111; 96]),
            work_queue: BitMaskWorkQueue(0),
            filled_pos: 0,
        }
    }

    fn give_val(&mut self, lc: CellIndices, v: SudokuValue) -> Result<(), ()> {
        force_set_index(&mut self.content, FlatIndex::from(lc), v, &mut self.sudoku, &mut self.work_queue)?;
        self.filled_pos += 1;
        Ok(())
    }

    fn run(mut self) -> Sudoku {
        while self.filled_pos < 81 {
            while let Some(idx) = self.work_queue.pop() {
                let res = process_index(&mut self.content, idx, &mut self.sudoku, &mut self.work_queue);
                if let Err(_) = res {
                    // row 8/cell 5
                    return self.sudoku;
                }
                self.filled_pos += 1;
            }
            if self.filled_pos >= 81 {
                break;
            }
            seek_quad_single_value(&mut self.content, &mut self.work_queue.0);
            if self.work_queue.0 == 0 {
                break;
            }
        }
        self.sudoku
    }
}