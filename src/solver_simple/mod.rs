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
impl LLSudokuSolverImpl for SimpleSolverImpl {
    fn tell_value_i(&mut self, inst: &mut LLSudokuSolverInst, i: u8, val: u8, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        Ok(())
    }

    fn tell_at_ind(&mut self, inst: &mut LLSudokuSolverInst, i: u8, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        Ok(())
    }
}