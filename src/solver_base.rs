use std::ops::IndexMut;

pub use indices::*;

use crate::Sudoku;
use crate::work_queue::WorkQueue;

pub struct LLSudokuSolverInst<MASK, COUNT>{
    content: Box<[MASK; 96]>,
    //outer: number, inner: 9x9 area
    num_counts: Box<[[COUNT; 9]; 9]>,
}

impl <MASK: From<u16> + Copy, COUNT: From<u8> + Copy> LLSudokuSolverInst<MASK, COUNT> {
    pub fn new() -> Self {
        Self {
            content: Box::new([MASK::from(0b1_1111_1111); 96]),
            num_counts: Box::new([[COUNT::from(9); 9]; 9]),
        }
    }

    pub fn content(&self) -> &[MASK; 81] {
        self.content[0..81].try_into().unwrap()
    }
    pub fn content_aligned(&self) -> &[MASK; 96] {
        &self.content
    }
    pub fn content_mut(&mut self) -> &mut [MASK; 81] {
        self.content.index_mut(..81).try_into().unwrap()
    }
    pub fn content_aligned_mut(&mut self) -> &mut [MASK; 96] {
        &mut self.content
    }
    pub fn num_counts(&self) -> &[[COUNT; 9]; 9] {
        &self.num_counts
    }
    pub fn num_counts_mut(&mut self) -> &mut [[COUNT; 9]; 9] {
        &mut self.num_counts
    }
}

impl <MASK: Copy + std::fmt::Debug, COUNT> LLSudokuSolverInst<MASK, COUNT> where u16: TryFrom<MASK>{
    pub fn debug_print(&self) {
        for i in 0..9 {
            for j in 0..9 {
                let mask = self.content.as_ref()[i * 9 + j];
                let Ok(mask) = u16::try_from(mask) else {
                    panic!("{mask:?} is not a valid mask!")
                };
                for v in 0..9 {
                    if (mask >> v) & 1 > 0 {
                        print!("{}", v + 1);
                    } else {
                        print!("-")
                    }
                }
                print!("  ");
            }
            println!();
        }
    }
}

pub enum GiveValError  {
    PositionAlreadySet, ValueDoesNotFitThere
}

pub trait GeneralSudokuSolver {
    fn new() -> Self;
    fn give_val(&mut self, lc: FlatIndex, v: SudokuValue) -> Result<(), GiveValError>;
    fn into_current_solved_state(self) -> Sudoku;
    fn run(self) -> Result<Sudoku, Sudoku>;
}



pub struct LLGeneralSudokuSolver<T: LLSudokuSolverImpl> {
    base_impl: T,
    pub solver_inst: LLSudokuSolverInst<T::Mask, T::Count>,
    sudoku: Sudoku,
    pub work_queue: T::WorkQueue
}

impl <T: LLSudokuSolverImpl + Default> GeneralSudokuSolver for LLGeneralSudokuSolver<T> where u16:TryFrom<T::Mask>{
    fn new() -> Self {
        Self {
            base_impl: T::default(),
            solver_inst: LLSudokuSolverInst::new() ,
            sudoku: Sudoku::new(),
            work_queue: T::WorkQueue::new()
        }
    }

    fn give_val(&mut self, lc: FlatIndex, v: SudokuValue) -> Result<(), GiveValError> {
        self.base_impl.tell_value(&mut self.solver_inst, lc, v, &mut self.sudoku, &mut self.work_queue)
    }

    fn into_current_solved_state(self) -> Sudoku {
        self.sudoku
    }

    fn run(mut self) -> Result<Sudoku, Sudoku> {
        while let Some(rem) = self.work_queue.pop() {
            let res = self.base_impl.process_index(&mut self.solver_inst, rem, &mut self.sudoku, &mut self.work_queue);

            if let Err(_) = res {
                unreachable!();
            }
        }
        for i in 0..81 {
            if self.sudoku[FlatIndex::checked_new(i)].is_none() {
                return Err(self.sudoku);
            }
        }
        Ok(self.sudoku)
    }
}

pub trait LLSudokuSolverImpl {
    type Mask: From<u16> + Copy + std::fmt::Debug;
    type Count: From<u8> + Copy + std::fmt::Debug;
    type WorkQueue: WorkQueue;
    fn tell_value(&mut self, inst: &mut LLSudokuSolverInst<Self::Mask, Self::Count>, indices: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut Self::WorkQueue) -> Result<(), GiveValError> {
        self.force_set_index(inst, indices, val, sudoku, work_q)
    }

    fn force_set_index(&mut self, inst: &mut LLSudokuSolverInst<Self::Mask, Self::Count>, i: FlatIndex, val: SudokuValue, sudoku: &mut Sudoku, work_q: &mut Self::WorkQueue) -> Result<(), GiveValError>;
    fn process_index(&mut self, inst: &mut LLSudokuSolverInst<Self::Mask, Self::Count>, i: FlatIndex, sudoku: &mut Sudoku, work_q: &mut Self::WorkQueue) -> Result<(), ()>;
}

mod indices {
    use std::num::NonZeroU8;

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
                pub const fn checked_new(i: u8) -> Self {
                    let me = Self::new(i);
                    if let Some(me) = me {
                        me
                    } else {
                        panic!("invalid index")
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
            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Display::fmt(&self.0, f)
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

    impl std::fmt::Display for SudokuValue {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Display::fmt(&self.get_1based().get(), f)
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