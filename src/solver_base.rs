use crate::Sudoku;

pub struct LLSudokuSolverInst {
    pub content: Box<[i32; 81]>,
    //outer: number, inner: 9x9 area
    pub num_counts: Box<[[i32; 9]; 9]>,
}

impl LLSudokuSolverInst {
    pub fn new() -> Self {
        Self {
            content: Box::new([0b1_1111_1111; 81]),
            num_counts: Box::new([[9; 9]; 9]),
        }
    }
}

pub trait GeneralSudokuSolver {
    fn new() -> Self;
    fn give_val(&mut self, lcv: (u8, u8, u8));
    fn run(self) -> Sudoku;
}

pub struct LLGeneralSudokuSolver<T> {
    base_impl: T,
    solver_inst: LLSudokuSolverInst,
    sudoku: Sudoku,
    work_queue: Vec<u16>
}

impl <T: LLSudokuSolverImpl + Default> GeneralSudokuSolver for LLGeneralSudokuSolver<T> {
    fn new() -> Self {
        Self {
            base_impl: T::default(),
            solver_inst: LLSudokuSolverInst::new() ,
            sudoku: Sudoku::new(),
            work_queue: Vec::with_capacity(20)
        }
    }

    fn give_val(&mut self, lcv: (u8, u8, u8)) {
        self.base_impl.tell_value(&mut self.solver_inst, lcv.0, lcv.1, lcv.2, &mut self.sudoku, &mut self.work_queue);
    }

    fn run(mut self) -> Sudoku {
        while let Some(rem) = self.work_queue.pop() {
            self.base_impl.tell_at_ind(&mut self.solver_inst, rem as u8, &mut self.sudoku, &mut self.work_queue);
        }
        self.sudoku
    }
}

pub trait LLSudokuSolverImpl {
    fn tell_value(&mut self, inst: &mut LLSudokuSolverInst, l: u8, c: u8, val: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()> {
        let res = self.tell_value_i(inst, l * 9 + c, val, sudoku, work_q);
        res
    }

    fn tell_value_i(&mut self, inst: &mut LLSudokuSolverInst, i: u8, val: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()>;
    fn tell_at_ind(&mut self, inst: &mut LLSudokuSolverInst, i: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()>;
}