#![feature(stdsimd)]
#![feature(concat_idents)]
#![feature(slice_flatten)]
#![feature(portable_simd)]

extern crate core;

use std::str::FromStr;

type DefaultSolver = solver_full_loop::SolverFullLoop;

use crate::solver_base::{CellIndex, CellIndices, FlatIndex, GeneralSudokuSolver, Indices, LLSudokuSolverImpl, SudokuValue};

mod solver_base;
mod work_queue;
mod solver_full_loop;

pub struct Sudoku {
    content: Box<[Option<SudokuValue>; 81]>
}

impl Sudoku {
    fn new() -> Self {
        Self {
            content: Box::new([None; 81])
        }
    }

    fn set_i(&mut self, index: FlatIndex, val: SudokuValue) {
        self.content[index.as_idx()] = Some(val);
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
            writeln!(f)?;
        }
        Ok(())
    }
}

fn main() {
    let as_text = include_str!("../sudokus/s4.txt");

    /*let given: Vec<_> = as_text
        .lines()
        .enumerate()
        .flat_map(|(r, l)| l.chars().enumerate().map(move |(c, v)| ((r, c), v)))
        .filter(|(_, v)| *v != '-')
        .map(|(pos, v)| {
            let mut str_buf = [0u8; 4];
            let str_buf = v.encode_utf8(&mut str_buf);
            (pos, u8::from_str(str_buf).ok().and_then(SudokuValue::new_1based).unwrap_or_else(|| panic!("invalid sudoku index {v}")))
        })
        .map(|((r, c), v)| {
            let row = u8::try_from(r).ok().and_then(CellIndex::new).expect("invalid row index");
            let column= u8::try_from(c).ok().and_then(CellIndex::new).expect("invalid column index");
            (CellIndices { row, column }, v)
        })
        .collect::<Vec<_>>();*/

    let given1: Vec<_> = as_text
        .lines()
        .map(|l| l.split('\t').map(|l| l.trim()))
        .enumerate()
        .map(|(l, ln)| ln.enumerate().map( move |(c, x)| ((l, c), x)))
        .flatten()
        .filter(|(_, v)| !v.is_empty())
        .map(|((l, c), v)| ((CellIndex::new(l as u8).unwrap(), CellIndex::new(c as u8).unwrap()), v))
        .map(|(c, v)| (c, SudokuValue::new_1based(u8::from_str(v).expect("error")).unwrap()))
        .map(|((row, column), v)| (CellIndices { row, column }, v))
        .collect();
    let start = std::time::Instant::now();
    let mut solv = DefaultSolver::new();
    for (idx, v) in given1 {
        let res = solv.give_val(idx, v);
        if let Err(_) = res {
            eprintln!("sudoku unsolvable:!");
            return;
        }
    }
    let sudoku = solv.run();
    dbg!(start.elapsed());
    println!("{sudoku}");
    check_sudoku(&sudoku);
    println!("Hello, world!");
}

fn check_sudoku(sudoku: &Sudoku) {
    for i in 0..81 {
        sudoku.content[i].unwrap();
    }
    for axis1 in 0..9 {
        for axis2 in 0..9 {
            for p in 0..axis2 {
                let axis1 = CellIndex::new(axis1).unwrap();
                let axis2 = CellIndex::new(axis2).unwrap();
                let p = CellIndex::new(p).unwrap();
                let idx = CellIndices { row: axis1, column: axis2 };
                let idx2 = CellIndices { row: axis1, column: p };
                assert_ne!(sudoku[FlatIndex::from(idx)], sudoku[FlatIndex::from(idx2)]);
                let idx = CellIndices { row: axis2, column: axis1 };
                let idx2 = CellIndices { row: p, column: axis1 };
                assert_ne!(sudoku[FlatIndex::from(idx)], sudoku[FlatIndex::from(idx2)]);

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

/*
6,9,5,7,8,3,2,1,4,
3,1,7,9,4,2,8,5,6,
2,4,8,1,5,6,9,7,3,
1,5,2,4,3,8,6,9,7,
8,7,3,6,2,9,1,4,5,
9,6,4,5,7,1,3,2,8,
7,8,1,3,9,4,5,6,2,
4,2,6,8,1,5,7,3,9,
5,3,9,2,6,7,4,8,1,
*/

/*
6,9,5,7,8,3,2,1,4,
3,1,7,9,4,2,8,5,6,
2,4,8,1,5,6,9,7,3,
1,5,2,4,3,8,6,9,7,
8,7,3,6,2,9,1,4,5,
9,6,4,5,7,1,3,2,8,
7,8,1,3,9,4,5,6,2,
4,2,6,8,1,5,7,3,9,
5,3,9,2,6,7,4,8,1,
*/