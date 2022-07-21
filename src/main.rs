#![feature(stdsimd)]
#![feature(concat_idents)]

#[cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
mod solver_avx512;
mod solver_base;
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl")))]
mod solver_simple;
#[cfg(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
use solver_avx512::Avx512SudokuSolverImpl as DefaultSolver;
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse", target_feature = "avx", target_feature = "avx2", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl")))]
use solver_simple::SimpleSolverImpl as DefaultSolver;
use std::str::FromStr;

use solver_base::LLSudokuSolverInst;
use crate::solver_base::LLSudokuSolverImpl;


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
    let mut sudoku = Sudoku::new();
    let mut sudoku_solv_inst = LLSudokuSolverInst::new();
    let mut sudoku_solv = DefaultSolver;
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
    let mut q = Vec::with_capacity(81);
    for ((l, c), v) in given1 {
        sudoku_solv.tell_value(&mut sudoku_solv_inst, l, c, v, &mut sudoku, &mut q);
    }
    while let Some(x) = q.pop() {
        sudoku_solv.tell_at_ind(&mut sudoku_solv_inst, x as u8, &mut sudoku, &mut q);
    }

    for i in 0..9 {
        for j in 0..9 {
            print!("{:09b},", sudoku_solv_inst.content.as_ref()[i * 9 + j]);
        }
        println!();
    }

    for i in 0..9 {
        for j in 0..9 {
            print!("{},", sudoku.content.as_ref()[i * 9 + j]);
        }
        println!();
    }
    dbg!(sudoku.content.iter().filter(|x| **x != 0).count());
    //dbg!(sudoku_solv.num_counts.as_ref());
    dbg!(q);
    dbg!(sudoku_solv_inst.num_counts[3][6]);
    println!("Hello, world!");
}
