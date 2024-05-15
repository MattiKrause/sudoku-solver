#![warn(clippy::pedantic)]

#![feature(stdsimd)]
#![feature(concat_idents)]
#![feature(slice_flatten)]
#![feature(portable_simd)]

#![no_std]

extern crate core;


pub use solver_base::{CellIndex, CellIndices, FlatIndex, SudokuValue};
pub use sudoku::Sudoku;

use crate::solver_base::{GeneralSudokuSolver, GiveValError};

type DefaultSolver = solver_full_loop::SolverFullLoop;

mod solver_base;
mod work_queue;
mod solver_full_loop;
mod sudoku;


pub enum InvalidSudokuError {
    PositionGivenTwice(FlatIndex),
    ValueCannotFit(FlatIndex, SudokuValue)
}

pub enum SudokuSolveError {
    SudokuHasNotSolution(Sudoku),
    InvalidSudoku(InvalidSudokuError, Sudoku)
}

pub fn solve(given: &[(FlatIndex, SudokuValue)]) -> Result<Sudoku, SudokuSolveError> {
    let mut solver = DefaultSolver::new();
    for (idx, val) in given {
        let result = solver.give_val(*idx,  *val);
        if let Err(err) = result {
           let invalid_err = match err {
               GiveValError::PositionAlreadySet => InvalidSudokuError::PositionGivenTwice(*idx),
               GiveValError::ValueDoesNotFitThere => InvalidSudokuError::ValueCannotFit(*idx, *val)
           };
            return Err(SudokuSolveError::InvalidSudoku(invalid_err, solver.into_current_solved_state()))
        }
    }
    let solver_result = solver.run();
    #[cfg(debug_assertions)]
    if let Ok(s) = &solver_result {
        check_sudoku(s);
    }
    solver_result.map_err(SudokuSolveError::SudokuHasNotSolution)
}

#[cfg(debug_assertions)]
fn check_sudoku(sudoku: &Sudoku) {
    for i in 0..81 {
        sudoku[FlatIndex::checked_new(i)].unwrap();
    }
    for axis1 in 0..9 {
        for axis2 in 0..9 {
            for p in 0..axis2 {
                let axis1 = CellIndex::new(axis1).unwrap();
                let axis2 = CellIndex::new(axis2).unwrap();
                let p = CellIndex::new(p).unwrap();
                let idx = CellIndices { row: axis1, column: axis2 };
                let idx2 = CellIndices { row: axis1, column: p };
                assert_ne!(sudoku[idx], sudoku[FlatIndex::from(idx2)]);
                let idx = CellIndices { row: axis2, column: axis1 };
                let idx2 = CellIndices { row: p, column: axis1 };
                assert_ne!(sudoku[idx], sudoku[FlatIndex::from(idx2)]);

            }
        }
    }
    for q1 in (0..3).map(|i| i * 3) {
        for q2 in (0..3).map(|i| i * 3) {
            let qis = (q1..(q1 + 3))
                .flat_map(|a1| (q2..(q2 + 3)).map(move |a2| (a1, a2)))
                .map(|(a1, a2)| CellIndices { row: CellIndex::new(a1).unwrap(), column: CellIndex::new(a2).unwrap() })
                .map(FlatIndex::from)
                .collect::<Vec<_>>();
            for i in 0..9 {
                for j in 0..i {
                    assert_ne!(sudoku[qis[i]], sudoku[qis[j]]);
                }
            }
        }
    }
}