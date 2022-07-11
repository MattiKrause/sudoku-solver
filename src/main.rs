#![feature(stdsimd)]
#![feature(concat_idents)]

use std::str::FromStr;
use core::arch::x86_64::{__m256i, __m128i, __m512i};

struct SudokuSolverInst {
    content: Box<[i32; 81]>,
    //outer: number, inner: 9x9 area
    num_counts: Box<[[i32; 9]; 9]>
}

trait DbgDmp<C: Default + Copy, const N: usize>: Sized {
    unsafe fn store(self, arr: &mut [C; N]);
    unsafe fn dmp_arr(self) -> [C; N] {
        let mut arr = [C::default(); N];
        self.store(&mut arr);
        arr
    }
}
macro_rules! dbg_dmp_impl {
    ($t: ident, $t2: ident,  $c: ident, $n: literal) => {
        impl DbgDmp<$c, $n> for concat_idents!(__, $t, i) {
            unsafe fn store(self, arr: &mut [$c; $n]) {
                use core::arch::x86_64::*;
                concat_idents!(_m, $t2, _storeu_ep, $c)(arr.as_mut_ptr(), self);
            }
        }
    };
    ($t: ident,  $c: ident, $n: literal) => {
        dbg_dmp_impl!($t, $t, $c, $n);
    };
}
dbg_dmp_impl!(m128, m, i8, 16);
dbg_dmp_impl!(m128, m, i16, 8);
dbg_dmp_impl!(m128, m, i32, 4);
dbg_dmp_impl!(m256, i8, 32);
dbg_dmp_impl!(m256, i32, 8);
dbg_dmp_impl!(m512, i32, 16);

macro_rules! str_vec {
                ($vec:expr, $n: ident, $t: ident) => {
                    let $n = $vec.dmp_arr();
                    let _: &[$t] = &$n;
                };
            }
macro_rules! dbg_vec {
                ($vec: ident, $t: ident) => {
                    str_vec!($vec, x, $t);
                    println!("{}:{} = {:?}", line!(), stringify!($vec), x);
                };
                ($vec: ident) => {
                    dbg_vec!($vec, i32)
                }
            }
macro_rules! dbg_vec_bin {
                ($vec: expr) => {
                    str_vec!($vec, x);
                    print!("{} x: [", line!());
                    x.iter().map(|x| format!("{:09b}", x)).for_each(|el| print!("{:9},", &el[0..9]));
                    println!("]");
                };
            }
impl SudokuSolverInst {
    fn new() -> Self {
        Self {
            content: Box::new([0b1_1111_1111; 81]),
            num_counts: Box::new([[9; 9]; 9])
        }
    }
    unsafe fn check_set(&mut self, i: u8, val: i32, work_q: &mut Vec<u16>) -> u32 {
        const fn comp_col() -> [[u8; 8]; 9] {
            let mut res = [[0; 8]; 9];
            let mut i = 0;
            while i < 9 {
                let mut wr_i = 0;
                let mut resi = [0; 8];
                let mut j = 0;
                while j < 9 {
                    if j != i {
                        resi[wr_i] = j as u8;
                        wr_i += 1;
                    }
                    j += 1;
                }
                res[i] = resi;
                i += 1;
            }
            res
        }
        static COL_CHECK: [[u8; 8]; 9] = comp_col();
        static MIS_SET: [u16; 9] = [0b111_111_110, 0b111_111_101, 0b111_111_011, 0b111_110_111, 0b111_101_111, 0b111_011_111, 0b110_111_111, 0b101_111_111, 0b011_111_111];
        static IDX012: [i32; 4] = [0, 1, 2, 0];
        //static LINE_CHECK: [[u8; 8]; 9] = comp_line();
        unsafe fn comp_pos_for_i(src: *mut i32, idx: __m512i, rem_mask: __m512i, work_q: *mut u16) -> (u32, u16) {
            let vals = core::arch::x86_64::_mm512_i32gather_epi32::<4>(idx, src as *const u8);
            let vals_rem = core::arch::x86_64::_mm512_and_si512(vals, rem_mask);
            core::arch::x86_64::_mm512_i32scatter_epi32::<4>(src as *mut u8, idx, vals_rem);
            let changed: u16 = core::arch::x86_64::_mm512_cmp_epi32_mask::<4>(vals, vals_rem);
            let pos_cnt = core::arch::x86_64::_mm512_popcnt_epi32(vals_rem);
            let ones = core::arch::x86_64::_mm512_set1_epi32(1);
            let one_pos: u16 = core::arch::x86_64::_mm512_cmp_epi32_mask::<0>(pos_cnt, ones);
            let changed_one_pos = one_pos & changed;

            let idxu16 = core::arch::x86_64::_mm512_cvtepi32_epi16(idx);
            core::arch::x86_64::_mm256_mask_compressstoreu_epi16(work_q as *mut u8, changed_one_pos, idxu16);
            (changed_one_pos.count_ones(), changed)
        }
        unsafe fn dec_num_count(changed: u16, mis_i_set: u32, cnt_ptr: *mut i32, cnt_idx_o: __m256i) -> ([i32; 4], u32) {
            let cnt_idx = core::arch::x86_64::_mm512_zextsi256_si512(cnt_idx_o);
            let cnts = core::arch::x86_64::_mm512_i32gather_epi32::<4>(cnt_idx, cnt_ptr as *const u8);
            let mis_i_set = mis_i_set as u32;
            let changed = core::arch::x86_64::_mm256_maskz_set1_epi8(changed as u32, 1);
            let changed = core::arch::x86_64::_mm256_maskz_expand_epi8(mis_i_set, changed);
            let sub_i = core::arch::x86_64::_mm256_maskz_set1_epi8(mis_i_set, 1);
            let sub_i = core::arch::x86_64::_mm256_and_si256(sub_i, changed);
            let zeroes = core::arch::x86_64::_mm256_setzero_si256();
            let mut sub_r = core::arch::x86_64::_mm256_alignr_epi8::<1>(sub_i, sub_i);
            let mut sub_a = core::arch::x86_64::_mm256_mask_add_epi8(sub_r, 0b001001001_001001001, sub_i, sub_r);
            sub_r = core::arch::x86_64::_mm256_alignr_epi8::<1>(sub_a, sub_a);
            sub_a = core::arch::x86_64::_mm256_mask_add_epi8(sub_a, 0b001001001_001001001, sub_a, sub_r);
            let sub_a = core::arch::x86_64::_mm256_maskz_compress_epi8(0b001001001_001001001, sub_a);
            let sub_a = core::arch::x86_64::_mm512_cvtepi8_epi32(core::arch::x86_64::_mm256_extracti128_si256::<0>(sub_a));
            let ncnts = core::arch::x86_64::_mm512_sub_epi32(cnts, sub_a);
            core::arch::x86_64::_mm512_mask_i32scatter_epi32::<4>(cnt_ptr as *mut u8, 0b111_111, cnt_idx, ncnts);
            let ncnts_changed = core::arch::x86_64::_mm512_cmp_epi32_mask::<4>(ncnts, cnts);
            let ones = core::arch::x86_64::_mm512_set1_epi32(1);
            let is_one: u16 = core::arch::x86_64::_mm512_cmp_epi32_mask::<0>(ones, ncnts);
            let  mut one_idx = [-1i32; 4];
            core::arch::x86_64::_mm256_mask_compressstoreu_epi32(one_idx.as_mut_ptr() as *mut u8, (is_one & ncnts_changed & 0b111_111) as u8, cnt_idx_o);
            (one_idx, (is_one & ncnts_changed).count_ones())
        }
        let (line, col) = (i / 9, i % 9);
        let rem_mask = core::arch::x86_64::_mm512_set1_epi32(!(1 << (val - 1)));
        let col_i = {
            let col = core::arch::x86_64::_mm_maskz_loadu_epi8(0x00FF, COL_CHECK[col as usize].as_ptr() as *const i8);
            let col_off = core::arch::x86_64::_mm_set1_epi8((line * 9) as i8);
            let col_i = core::arch::x86_64::_mm_add_epi8(col, col_off);
            core::arch::x86_64::_mm256_cvtepi8_epi32(col_i)
        };
        let line_i = {
            let line = core::arch::x86_64::_mm_maskz_loadu_epi8(0x00FF, COL_CHECK[line as usize].as_ptr() as *const i8);
            //dbg_vec!(line, i8);
            let line = core::arch::x86_64::_mm256_cvtepi8_epi32(line);
            let line_mul = core::arch::x86_64::_mm256_set1_epi32(9);
            let line = core::arch::x86_64::_mm256_mullo_epi32(line, line_mul);
            let line_off = core::arch::x86_64::_mm256_set1_epi32(col as i32);
            core::arch::x86_64::_mm256_add_epi32(line, line_off)
        };
        let col_i = core::arch::x86_64::_mm512_zextsi256_si512(col_i);
        let lane_i = core::arch::x86_64::_mm512_inserti64x4::<1>(col_i, line_i);
        let (mut new_q, changed) = comp_pos_for_i(self.content.as_mut_ptr(), lane_i, rem_mask, work_q.as_mut_ptr().add(work_q.len()));
        work_q.set_len(work_q.len() + new_q as usize);
        let cnt_idx  = {
            //for some reason _mm_maskz_load_epi32 seams to seg fault randomly
            let vec012 = core::arch::x86_64::_mm_load_epi32(IDX012.as_ptr());
            let line_offset = core::arch::x86_64::_mm_set1_epi32(((line / 3) * 3) as i32);
            let three_vec = core::arch::x86_64::_mm_set1_epi32(3);
            let col_vec = core::arch::x86_64::_mm_add_epi32(vec012, line_offset);
            let line_starts = core::arch::x86_64::_mm_mullo_epi32(three_vec, vec012);
            let col_offset = core::arch::x86_64::_mm_set1_epi32((col / 3) as i32);
            let line_vec = core::arch::x86_64::_mm_add_epi32(line_starts, col_offset);
            let col_vec = core::arch::x86_64::_mm256_zextsi128_si256(col_vec);
            let both_vec = core::arch::x86_64::_mm256_inserti128_si256::<1>(col_vec, line_vec);
            core::arch::x86_64::_mm256_maskz_compress_epi32(0b1110111, both_vec)
        };
        let (singled_box, singled_cnt) = dec_num_count(changed, ((MIS_SET[line as usize] as u32) << 9) | (MIS_SET[col as usize] as u32), self.num_counts[val as usize - 1].as_mut_ptr(),cnt_idx);
        if singled_cnt > 0 {
            dbg!(&singled_box[0..(singled_cnt as usize)]);
        }
        new_q
    }
    fn tell_value(&mut self, (l, c): (u8, u8), val: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()>{
        self.tell_value_i(l * 9 + c, val, sudoku, work_q)
    }
    fn tell_value_i(&mut self, i: u8, val: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()>{
        let new_content = 1 << (val - 1);
        if self.content[i as usize] & new_content != 0 {
            let i= i as usize;
            let a9x9 = ((i / 9) / 3) * 3 + (i % 9) / 3;
            self.num_counts[val as usize - 1][a9x9] -= 1;
        }
        self.content[i as usize] = new_content;
        self.tell_at_ind(i, sudoku, work_q)
    }
    fn tell_at_ind(&mut self, i: u8, sudoku: &mut Sudoku, work_q: &mut Vec<u16>) -> Result<u32, ()> {
        let val = self.content[i as usize];
        let val_set = val.trailing_zeros() as i32 + 1;
        if val == 0 || val_set > 9 {
            return Err(());
        }
        if work_q.capacity() - work_q.len() < 27{
            return Err(());
        }
        self.content[i as usize] = 0;
        sudoku.set_i(i, val_set as u16);
        unsafe {
            Ok(self.check_set(i, val_set, work_q))
        }
    }
}

struct Sudoku {
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
    macro_rules! given {
        ($($val: literal: $l: literal, $c: literal);*;) => {
            vec![$((($l, $c), $val)),*]
        };
    }
    let mut sudoku = Sudoku::new();
    let mut sudoku_solv = SudokuSolverInst::new();
    let as_text = r#"     	7 					1
			8 	9 	5
						3 	4
		6 					8 	2
		8 	4 	2 				3
		1 					9
8 		3 	7 		9 	4 		5
4 				5 	6
2 			3 					1"#;
    let given1: Vec<_> = as_text
        .lines()
        .map(|l| l.split('\t').map(|l| l.trim()))
        .enumerate()
        .map(|(l, ln)| ln.enumerate().map( move |(c, x)| ((l, c), x)))
        .flatten()
        .filter(|(c, v)| !v.is_empty())
        .map(|((l, c), v)| ((l as u8, c as u8), v))
        .map(|(c, v)| (c, u8::from_str(v).expect("error")))
        .collect();
    let mut q = Vec::with_capacity(81);
    for (c, v) in given1 {
        sudoku_solv.tell_value(c, v, &mut sudoku, &mut q);
    }
    while let Some(x) = q.pop() {
        sudoku_solv.tell_at_ind(x as u8, &mut sudoku, &mut q);
    }

    for i in 0..9 {
        for j in 0..9 {
            print!("{:09b},", sudoku_solv.content.as_ref()[i * 9 + j]);
        }
        println!();
    }

    for i in 0..9 {
        for j in 0..9 {
            print!("{},", sudoku.content.as_ref()[i * 9 + j]);
        }
        println!();
    }
    dbg!(sudoku_solv.num_counts.as_ref());
    dbg!(q);
    println!("Hello, world!");
}
