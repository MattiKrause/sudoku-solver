#![feature(stdsimd)]
#![feature(concat_idents)]

#[cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
mod solver_avx512;
mod solver_base;
mod solver_simple;
mod work_queue;

#[cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
use solver_avx512::Avx512SudokuSolver as DefaultSolver;
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl")))]
use solver_simple::SimpleSolver as DefaultSolver;
use std::str::FromStr;

use solver_base::LLSudokuSolverInst;
use crate::solver_base::{GeneralSudokuSolver, LLSudokuSolverImpl};


pub struct Sudoku {
    content: Box<[u16; 81]>
}

impl Sudoku {
    fn new() -> Self {
        Self {
            content: Box::new([0; 81])
        }
    }

    fn set_i(&mut self, index: u8, val: u16) {
        self.content[index as usize] = val;
    }
}

fn main() {
    let as_text = include_str!("../sudokus/s4.txt");
    let given1: Vec<_> = as_text
        .lines()
        .map(|l| l.split('\t').map(|l| l.trim()))
        .enumerate()
        .map(|(l, ln)| ln.enumerate().map( move |(c, x)| ((l, c), x)))
        .flatten()
        .filter(|(_, v)| !v.is_empty())
        .map(|((l, c), v)| ((l as u8, c as u8), v))
        .map(|(c, v)| (c, u8::from_str(v).expect("error")))
        .collect();
    dbg!(given1.len());
    let mut solv = DefaultSolver::new();
    for ((l, c), v) in given1 {
        let res = solv.give_val((l, c , v));
        if let Err(_) = res {
            eprintln!("sudoku unsolvable:!");
            return;
        }
    }
    let sudoku = solv.run();
    for i in 0..9 {
        for j in 0..9 {
            print!("{},", sudoku.content.as_ref()[i * 9 + j]);
        }
        println!();
    }
    println!("Hello, world!");
}
