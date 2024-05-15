use std::io::Read;
use std::process::ExitCode;

use arrayvec::ArrayVec;

use sudoku::{CellIndices, FlatIndex, InvalidSudokuError, SudokuSolveError, SudokuValue};

fn main() -> ExitCode {
    let mut buffer = [0u8; 89];
    {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();

        match stdin.read(&mut buffer) {
            Ok(n) if n < buffer.len() => {
                eprintln!("ERROR: not enough bytes provided(need exactly 81, got {n})");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("ERROR: failed to read bytes from stdin: {e}");
                return ExitCode::FAILURE;
            }
            Ok(_) => {}
        }
    }

    let mut found: ArrayVec<(FlatIndex, SudokuValue), 81> = ArrayVec::new();

    for r in 0..9 {
        for c in 0..9 {
            //skip newlines
            let buffer_idx = r * 10 + c;
            let sudoku_idx = FlatIndex::checked_new(r * 9 + c);

            let byte_value = buffer[buffer_idx as usize];
            if byte_value == b'-' {
                continue;
            }
            let value = Some(byte_value)
                .filter(|v| *v > b'0')
                .map(|v| v - b'0')
                .and_then(SudokuValue::new_1based);
            let value = match value {
                Some(v) => v,
                None => {
                    let idx = CellIndices::from(sudoku_idx);
                    eprintln!("ERROR: invalid sudoku cell value {} at row {} column {}. Expected one of {{ - ,1, 2, 3, 4, 5, 6, 7, 8, 9}}", byte_value as char, idx.row, idx.column);
                    return ExitCode::FAILURE;
                }
            };
            found.push((sudoku_idx, value))
        }
    }

    let solved = sudoku::solve(&found.as_slice());
    match solved {
        Ok(sudoku) => {
            println!("{sudoku}");
        }
        Err(e) => {
            match e {
                SudokuSolveError::SudokuHasNotSolution(s) => {
                    eprintln!("ERROR: sudoku unsolvable");
                    println!("{s}");
                }
                SudokuSolveError::InvalidSudoku(InvalidSudokuError::ValueCannotFit(cell, v), _) => {
                    let cell = CellIndices::from(cell);
                    eprintln!("ERROR: sudoku is not valid: {v} cannot be placed at row {} column {} according to the rules", cell.row, cell.column)
                }
                SudokuSolveError::InvalidSudoku(InvalidSudokuError::PositionGivenTwice(_), _) => {
                    unreachable!();
                }
            }
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}