use crate::solver_base::{CellIndex, CellIndices, FlatIndex, get_quad_offset, quadrant_indices_from_cell_indices};

const fn generate_row_mask(c: CellIndex) -> u128 {

    const ROW_MASK: u128 = compute_row_mask();
    const fn compute_row_mask() -> u128 {
        let mut i = 0;
        let mut mask = 0u128;
        while i < 9 {
            mask |= 1u128 << (i * 9);
            i+= 1;
        }
        mask
    }
    ROW_MASK << (c.get())
}

// Faster Version ??
/*
const fn generate_row_mask2(c: CellIndex) -> u128 {
    const LOW_MASK: u64 = (1u64 << (0 * 9)) | (1 << (1 * 9)) | (1 << (2 * 9)) | (1 << (3 * 9)) | (1 << (4 * 9)) | (1 << (5 * 9)) | (1 << (6 * 9)) | (1 << (7 * 9));
    const HIGH_MASK: u64 = (((LOW_MASK >> 1) as u32) as u64)  | (1 << 63);
    ((HIGH_MASK.rotate_left(c.get() as u32) as u128) << 64) | (LOW_MASK << c.get()) as u128
}*/

const fn generate_quadrant_mask(cell_indices: CellIndices) -> u128 {
    const QUADRANT_MASK: u64 = 0b111 | (0b111 << 9) | (0b111 << 18);
    let quadrant_offset = get_quad_offset(quadrant_indices_from_cell_indices(cell_indices));
    (QUADRANT_MASK as u128) << quadrant_offset.get()
}

const fn generate_column_mask(r: CellIndex) -> u128 {
    const COLUMN_MASK: u64 = 0b111_111_111;
    (COLUMN_MASK as u128) << (r.get() * 9)
}

// Faster Version??
/*
const fn generate_column_mask2(r: CellIndex) -> u128 {
    const COLUMN_MASK: u64 = 0b111_111_111;
    let by = r.get() * 9;
    let rotated = COLUMN_MASK.rotate_left(by as u32);
    let by_min_63 = if by <= 63 { by }else { 63 };
    let low_mask = u64::MAX << (by_min_63);
    let high_mask = !low_mask;

    (((rotated & high_mask) as u128) << 64) | ((rotated & low_mask) as u128)
}*/

/*
#[test]
fn test_column_masks() {
    let mut check_mask: u128 = 0;
    for _ in 0..81 {
        check_mask = (check_mask << 1) | 1;
    }
    for i in  0..81 {
        let off = get_quad_offset(QuadrantIndices::from(CellIndices::from(FlatIndex::new(i).unwrap())));
        // +1 -> 3x halten -> +1
        let my = (off.get() / 3) % 3;
        assert_eq!()
        dbg!();
    }
    panic!();
    for c in 0..9 {
        let c = CellIndex::new(c).unwrap();
        let m1 = generate_column_mask1(c) & check_mask;
        let m2 = generate_column_mask2(c) & check_mask;
        assert_eq!(m1, m2, "c: {c:?}, m1:\n{m1:081b}, m2:\n{m2:081b}");
    }
}*/

const fn generate_mask_const(cell_indices: CellIndices) -> u128 {
    generate_column_mask(cell_indices.row) | generate_row_mask(cell_indices.column) | generate_quadrant_mask(cell_indices)
}

const fn compute_mask_lookup() -> [u128; 81] {
    let mut content = [0u128; 81];
    let mut r = 0;
    while r < 9 {
        let r = {
            let r_tmp = r;
            r += 1;
            r_tmp
        };
        let mut c = 0;
        while c < 9 {
            let c = {
                let c_tmp = c;
                c += 1;
                c_tmp
            };
            let mask = generate_mask_const(CellIndices { row: CellIndex::checked_new(r), column: CellIndex::checked_new(c) });
            content[(r* 9 + c) as usize] = mask;
        }
    }
    #[allow(clippy::needless_return)]
    return content;
}

static MASK_LOOKUP: [u128; 81] = compute_mask_lookup();


pub fn generate_mask(i: FlatIndex) -> u128 {
    MASK_LOOKUP[i.as_idx()]
}