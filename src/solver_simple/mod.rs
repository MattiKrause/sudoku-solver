mod solver_impl;

use crate::solver_base::{LLGeneralSudokuSolver, LLSudokuSolverImpl};
use crate::{Sudoku, LLSudokuSolverInst};
use crate::work_queue::WorkQueue;

pub type SimpleSolver = LLGeneralSudokuSolver<SimpleSolverImpl>;
pub struct SimpleSolverImpl;
impl Default for SimpleSolverImpl {
    fn default() -> Self {
        Self
    }
}
