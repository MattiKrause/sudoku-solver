use crate::solver_base::{CellIndices, FlatIndex, SudokuValue};

pub struct Sudoku {
    content: Box<[Option<SudokuValue>; 81]>
}

impl Default for Sudoku {
    fn default() -> Self {
        Self::new()
    }
}

impl Sudoku {
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: Box::new([None; 81])
        }
    }

    #[must_use]
    pub fn get_value(&self, indices: CellIndices) -> Option<SudokuValue> {
        self.content[FlatIndex::from(indices).as_idx()]
    }
}

impl std::ops::Index<CellIndices> for Sudoku {
    type Output = Option<SudokuValue>;

    fn index(&self, index: CellIndices) -> &Self::Output {
        &self[FlatIndex::from(index)]
    }
}

impl std::ops::Index<FlatIndex> for Sudoku {
    type Output = Option<SudokuValue>;

    fn index(&self, index: FlatIndex) -> &Self::Output {
        &self.content[index.as_idx()]
    }
}
impl std::ops::IndexMut<FlatIndex> for Sudoku {
    fn index_mut(&mut self, index: FlatIndex) -> &mut Self::Output {
        &mut self.content[index.as_idx()]
    }
}
impl std::fmt::Display for Sudoku {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in 0..9 {
            for column in 0..9 {
                let index: u8 = row  * 9 + column;
                let index = usize::from(index);
                let value = self.content[index];
                match value {
                    None => write!(f, "-")?,
                    Some(v) => std::fmt::Display::fmt(&v.get_1based(), f)?
                }
            }
            if row != 8 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}