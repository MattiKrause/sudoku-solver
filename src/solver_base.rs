use crate::Sudoku;
use crate::work_queue::{WorkQueue, WorkQueue81};
pub use indices::*;

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

    pub fn debug_print(&self) {
        for i in 0..9 {
            for j in 0..9 {
                print!("{:09b},", self.content.as_ref()[i * 9 + j]);
            }
            println!();
        }
    }
}

pub trait GeneralSudokuSolver {
    fn new() -> Self;
    fn give_val(&mut self, lc: CellIndices, v: SudokuValue) -> Result<(), ()>;
    fn run(self) -> Sudoku;
}

pub struct LLGeneralSudokuSolver<T> {
    base_impl: T,
    solver_inst: LLSudokuSolverInst,
    sudoku: Sudoku,
    work_queue: WorkQueue81<u16>
}

impl <T: LLSudokuSolverImpl + Default> GeneralSudokuSolver for LLGeneralSudokuSolver<T> {
    fn new() -> Self {
        Self {
            base_impl: T::default(),
            solver_inst: LLSudokuSolverInst::new() ,
            sudoku: Sudoku::new(),
            work_queue: WorkQueue81::new()
        }
    }

    fn give_val(&mut self, lc: CellIndices, v: SudokuValue) -> Result<(), ()> {
        self.solver_inst.debug_print();
        println!("{}", self.sudoku);
        self.base_impl.tell_value(&mut self.solver_inst, lc, v, &mut self.sudoku, &mut self.work_queue.wq)
    }

    fn run(mut self) -> Sudoku {
        println!("{}", self.sudoku);
        while let Some(rem) = self.work_queue.wq.pop() {

            let res = self.base_impl.process_index(&mut self.solver_inst, FlatIndex::new(rem as u8).unwrap(), &mut self.sudoku, &mut self.work_queue.wq);

            if let Err(_) = res {
                return self.sudoku;
            }
        }
        self.sudoku
    }
}

pub trait LLSudokuSolverImpl {
    fn tell_value(&mut self, inst: &mut LLSudokuSolverInst, indices: CellIndices, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()> {
        self.force_set_index(inst, FlatIndex::from(indices), val, sudoku, work_q)
    }

    fn force_set_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()>;
    fn process_index(&mut self, inst: &mut LLSudokuSolverInst, i: FlatIndex, sudoku: &mut Sudoku, work_q: &mut WorkQueue<u16>) -> Result<(), ()>;
}

mod indices {
    use std::num::{NonZeroI32, NonZeroU8};
    macro_rules! decl_index {
        ($name: ident, 0..$range: literal) => {
            #[derive(Copy, Clone, Eq, PartialEq, Debug)]
            #[repr(transparent)]
            pub struct $name(u8);
            impl $name {
                pub const fn new(i: u8) -> Option<Self> {
                    if i < $range {
                        Some(Self(i))
                    } else {
                        None
                    }
                }
                pub const fn get(&self) -> u8 {
                    if self.0 < $range {
                        self.0
                    } else {
                        unsafe { std::hint::unreachable_unchecked() };
                    }
                }

                pub const fn as_idx(self) -> usize {
                    self.get()  as usize
                }
            }
        };
    }
    decl_index!(FlatIndex, 0..81);
    decl_index!(CellIndex, 0..9);
    decl_index!(QuadrantIndex, 0..3);
    decl_index!(FlatQuadrantIndex, 0..9);

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    #[repr(transparent)]
    pub struct SudokuValue(NonZeroU8);

    impl SudokuValue {
        pub const fn new_1based(v: u8) -> Option<Self> {
            if 1 <= v && v <= 9 {
                let Some(v) = NonZeroU8::new(v) else { return None };
                Some(Self(v))
            } else {
                None
            }
        }
        pub const fn new_0based(v: u8) -> Option<Self> {
            // only issue is v= 255, in that case v + 1 = 0 which is still not accepted
            let v = v.wrapping_add(1);
            Self::new_1based(v)
        }
        pub const fn get_1based(self) -> NonZeroU8 {
            if 1 <= self.0.get() && self.0.get() <= 9 {
                return self.0
            } else {
                unsafe { std::hint::unreachable_unchecked() }
            }
        }
        pub const fn get_0based(self) -> u8 {
            return self.get_1based().get() - 1;
        }
        pub const fn as_0based_idx(self) -> usize {
            self.get_0based() as usize
        }
        pub const fn as_mask_0based(self) -> u16 {
            1 << (self.get_0based() as u16)
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct Indices<T> {
        pub row: T,
        pub column: T
    }

    pub type CellIndices = Indices<CellIndex>;
    pub type QuadrantIndices = Indices<QuadrantIndex>;

    impl From<FlatIndex> for CellIndices {
        fn from(value: FlatIndex) -> Self {
            Indices {
                row: CellIndex::new(value.get() / 9).unwrap(),
                column: CellIndex::new(value.get() % 9).unwrap()
            }
        }
    }

    impl From<CellIndices> for FlatIndex {
        fn from(value: Indices<CellIndex>) -> Self {
            FlatIndex::new(value.row.get() * 9 + value.column.get()).unwrap()
        }
    }

    impl From<CellIndices> for QuadrantIndices {
        fn from(value: Indices<CellIndex>) -> Self {
            Indices {
                row: QuadrantIndex::new(value.row.get() / 3).unwrap(),
                column: QuadrantIndex::new(value.column.get() / 3).unwrap()
            }
        }
    }

    impl From<FlatQuadrantIndex> for QuadrantIndices {
        fn from(value: FlatQuadrantIndex) -> Self {
            Indices {
                row: QuadrantIndex::new(value.get() / 3).unwrap(),
                column: QuadrantIndex::new(value.get() % 3).unwrap(),
            }
        }
    }

    impl From<QuadrantIndices> for FlatQuadrantIndex {
        fn from(value: Indices<QuadrantIndex>) -> Self {
            FlatQuadrantIndex::new(value.row.get() * 3 + value.column.get()).unwrap()
        }
    }

    pub fn get_quad_offset(indices: QuadrantIndices) -> FlatIndex {
        let quadrant_start = Indices {
            row: CellIndex::new(indices.row.get() * 3).unwrap(),
            column: CellIndex::new(indices.column.get() * 3).unwrap(),
        };
        FlatIndex::from(quadrant_start)
    }
}