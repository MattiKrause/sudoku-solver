pub use indices::*;

use crate::Sudoku;

pub enum GiveValError  {
    PositionAlreadySet, ValueDoesNotFitThere
}

pub trait GeneralSudokuSolver {
    fn new() -> Self;
    fn give_val(&mut self, lc: FlatIndex, v: SudokuValue) -> Result<(), GiveValError>;
    fn into_current_solved_state(self) -> Sudoku;
    fn run(self) -> Result<Sudoku, Sudoku>;
}

mod indices {
    use core::num::NonZeroU8;

    macro_rules! decl_index {
        ($name: ident, 0..$range: literal) => {
            #[derive(Copy, Clone, Eq, PartialEq, Debug)]
            #[repr(transparent)]
            pub struct $name(u8);
            impl $name {
                #[must_use]
                pub const fn new(i: u8) -> Option<Self> {
                    if i < $range {
                        Some(Self(i))
                    } else {
                        None
                    }
                }
                #[must_use]
                pub const fn checked_new(i: u8) -> Self {
                    let me = Self::new(i);
                    if let Some(me) = me {
                        me
                    } else {
                        panic!("invalid index")
                    }
                }
                #[must_use]
                pub const fn get(&self) -> u8 {
                    if self.0 < $range {
                        self.0
                    } else {
                        unsafe { core::hint::unreachable_unchecked() };
                    }
                }

                #[must_use]
                pub const fn as_idx(self) -> usize {
                    self.get()  as usize
                }
            }
            impl core::fmt::Display for $name {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    core::fmt::Display::fmt(&self.0, f)
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
        #[must_use]
        pub const fn new_1based(v: u8) -> Option<Self> {
            if 1 <= v && v <= 9 {
                let Some(v) = NonZeroU8::new(v) else { return None };
                Some(Self(v))
            } else {
                None
            }
        }

        #[must_use]
        pub const fn new_0based(v: u8) -> Option<Self> {
            // only issue is v= 255, in that case v + 1 = 0 which is still not accepted
            let v = v.wrapping_add(1);
            Self::new_1based(v)
        }
        #[must_use]
        pub const fn get_1based(self) -> NonZeroU8 {
            if 1 <= self.0.get() && self.0.get() <= 9 {
                self.0
            } else {
                unsafe { core::hint::unreachable_unchecked() }
            }
        }

        #[must_use]
        pub const fn get_0based(self) -> u8 {
            self.get_1based().get() - 1
        }

        #[must_use]
        pub const fn as_0based_idx(self) -> usize {
            self.get_0based() as usize
        }

        #[must_use]
        pub const fn as_mask_0based(self) -> u16 {
            1 << (self.get_0based() as u16)
        }
    }

    impl core::fmt::Display for SudokuValue {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            core::fmt::Display::fmt(&self.get_1based().get(), f)
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct Indices<T> {
        pub row: T,
        pub column: T
    }

    #[allow(clippy::module_name_repetitions)]
    pub type CellIndices = Indices<CellIndex>;
    #[allow(clippy::module_name_repetitions)]
    pub type QuadrantIndices = Indices<QuadrantIndex>;

    macro_rules! impl_from_and_const_fn {
        ($(fn $fn_name: ident($var_name: ident: $from: ty) -> $to: ty $backing: block)*) => {
            $(
            pub const fn $fn_name($var_name: $from) -> $to $backing
            impl From<$from> for $to {
                fn from(value: $from) -> Self {
                    $fn_name(value)
                }
            }
            )*
        };
    }

    impl_from_and_const_fn! {
        fn cell_indices_from_flat_index(value: FlatIndex) -> CellIndices {
            Indices {
                row: CellIndex::checked_new(value.get() / 9),
                column: CellIndex::checked_new(value.get() % 9)
            }
        }
        fn flat_index_from_cell_indices(value: CellIndices) -> FlatIndex {
            FlatIndex::checked_new(value.row.get() * 9 + value.column.get())
        }
        fn quadrant_indices_from_cell_indices(value: CellIndices) -> QuadrantIndices {
            Indices {
                row: QuadrantIndex::checked_new(value.row.get() / 3),
                column: QuadrantIndex::checked_new(value.column.get() / 3)
            }
        }
        fn quadrant_indices_from_flat_quadrant_index(value: FlatQuadrantIndex) -> QuadrantIndices {
            Indices {
                row: QuadrantIndex::checked_new(value.get() / 3),
                column: QuadrantIndex::checked_new(value.get() % 3),
            }
        }
        fn flat_quadrant_index_from_quadrant_indices(value: QuadrantIndices) -> FlatQuadrantIndex {
            FlatQuadrantIndex::checked_new(value.row.get() * 3 + value.column.get())
        }
    }

    pub const fn get_quad_offset(indices: QuadrantIndices) -> FlatIndex {
        let quadrant_start = Indices {
            row: CellIndex::checked_new(indices.row.get() * 3),
            column: CellIndex::checked_new(indices.column.get() * 3),
        };
        flat_index_from_cell_indices(quadrant_start)
    }
}