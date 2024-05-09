use core::arch::x86_64::*;

use crate::solver_base::{CellIndices, FlatIndex, QuadrantIndices};

pub type I32x4 = __m128i;
pub type I32x9 = __m512i;
pub type I32x16 = __m512i;
pub type IndexI32x16 = I32x16;
pub type IndexI32x9 = I32x16;
pub type RowColumnIndex = IndexI32x16;

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
pub static LN_CHECK_PERM: [[u8; 8]; 9] = comp_col();
pub static QUAD_INDEX: [i32; 9] = [0, 1, 2, 9, 10, 11, 18, 19, 20];
static MIS_SET: [u16; 9] = [
    0b111_111_110, 0b111_111_101, 0b111_111_011, 0b111_110_111,
    0b111_101_111, 0b111_011_111, 0b110_111_111, 0b101_111_111, 0b011_111_111
];

pub fn find_missing_index_mask(indices: CellIndices) -> u32 {
    let mis_row = MIS_SET[indices.row.as_idx()] as u32;
    let mis_col = MIS_SET[indices.column.as_idx()] as u32;
    mis_row << 9 | mis_col
}

pub unsafe fn load_quad_indices() -> IndexI32x9 {
    _mm512_maskz_loadu_epi32(0b111_111_111, QUAD_INDEX.as_ptr())
}

pub fn comp_rem_mask(val: i32) ->  i32 {
    !(1 << val)
}

/// accumulate the first 6 triplets and pull the results into the first 6 slots.
/// The numbers should be 8 bit
pub unsafe fn accumulate_triplets(mut vals: __m256i) -> __m256i {
    vals = _mm256_maskz_expand_epi8(0b111_111_111_111_111_0_111, vals);
    // shift all values one unit towards the start of the vec and add, now we have 1 + 2, 3, *
    let mut vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    // shift another unit, now we have, 1 + 2 + 3, *, *
    vals_s = _mm256_alignr_epi8::<1>(vals, vals);
    vals = _mm256_mask_add_epi8(vals_s, 0b001001001_001001_0_001, vals_s, vals);
    _mm256_maskz_compress_epi8(0b001001001_001001_0_001, vals)
}

/// computes lane indices that need to be checked,  return in format (\[col;8\], (\[line; 8\]))
pub unsafe fn compute_lane_indices(indices: CellIndices) -> RowColumnIndex {
    //col indices computed as line * 9 + col_offset
    let col_i = {
        let col_offset =  _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[indices.column.as_idx()].as_ptr() as *const i8);
        let line_base = _mm_set1_epi8((indices.row.get() * 9) as i8);
        let col_i = _mm_add_epi8(col_offset, line_base);
        _mm256_cvtepi8_epi32(col_i)
    };
    //line indices computed as (line_offset * 9) + col
    let line_i = {
        let line_offset = _mm_maskz_loadu_epi8(0x00FF, LN_CHECK_PERM[indices.row.as_idx()].as_ptr() as *const i8);
        let line_offset = _mm256_cvtepi8_epi32(line_offset);
        let nine = _mm256_set1_epi32(9);
        let line_offset9 = _mm256_mullo_epi32(line_offset, nine);
        let col = _mm256_set1_epi32(i32::from(indices.column.get() as i32));
        _mm256_add_epi32(line_offset9, col)
    };
    let col_i = _mm512_zextsi256_si512(col_i);
    let lane_i = _mm512_inserti64x4::<1>(col_i, line_i);
    #[cfg(debug_assertions)]
    {
        validate_cross_indices(lane_i);
        let v: [i32; 16] = super::dbg_dmp::DbgDmp::dmp_arr(lane_i);
        let cell_indices = v.map(|i| FlatIndex::new(u8::try_from(i).unwrap()).unwrap()).map(CellIndices::from);
        for (pos, idx) in cell_indices[0..8].iter().enumerate() {
            let pos = pos as u8;
            assert_eq!(idx.row, indices.row);
            let adj_pos = if pos >= indices.column.get() { pos + 1 } else { pos };
            assert_eq!(idx.column.get(), adj_pos);
        }
        for (pos, idx) in cell_indices[8..16].iter().enumerate() {
            let pos = pos as u8;
            assert_eq!(idx.column, indices.column);
            let adj_pos = if pos >= indices.row.get() { pos + 1 } else { pos };
            assert_eq!(idx.row.get(), adj_pos);
        }
    }
    lane_i
}

pub fn validate_cross_indices(indices: IndexI32x16) {
    let indices: [i32; 16] = super::dbg_dmp::DbgDmp::dmp_arr(indices);
    let cell_indices = indices
        .map(|i| u8::try_from(i).ok().and_then(FlatIndex::new).unwrap_or_else(|| panic!("invalid flat index {i}")))
        .map(CellIndices::from);
    let column_indices = &cell_indices[0..8];
    let row_indices = &cell_indices[8..16];

    for c in column_indices.windows(2) {
        assert_eq!(c[0].row, c[1].row, "indices: {indices:?}/{cell_indices:?}")
    }
    for r in row_indices.windows(2) {
        assert_eq!(r[0].column, r[1].column, "indices: {indices:?}/{cell_indices:?}");
    }

}

/// computes the indices of the count variables that are affected by a change at (line, col)
/// indices are
pub unsafe fn comp_cnt_indices(quad_indices: QuadrantIndices) -> __m256i {
    static IDX012: [i32; 3] = [0, 1, 2];
    let vec012 = _mm_maskz_loadu_epi32(0b111, IDX012.as_ptr());
    let line_base = splat_i32x4(i32::from(quad_indices.row.get() * 3));
    let three_vec = splat_i32x4(3);
    //qline * 3 + (0, 1, 2)
    let col_vec = _mm_add_epi32(line_base, vec012);
    let line_offset = _mm_mullo_epi32(three_vec, vec012);
    let qcol_vec = splat_i32x4(i32::from(quad_indices.column.get()));
    //(0, 1, 2) * 3 + qcol
    let line_vec = _mm_add_epi32(line_offset, qcol_vec);
    let col_vec = _mm256_zextsi128_si256(col_vec);
    let both_vec = _mm256_inserti128_si256::<1>(col_vec, line_vec);
    // the relevant results are stored in the first three slots of each vector, if we simply
    // concatenate them, then the 4th slot still contains garbage. This is why we compress them.
    _mm256_maskz_compress_epi32(0b1110111, both_vec)
}

pub unsafe fn splat_i32x4(i: i32) -> I32x4 {
    _mm_set1_epi32(i)
}

pub unsafe fn splat_i32x16(i: i32) -> I32x16 {
    _mm512_set1_epi32(i)
}

pub unsafe fn gather_i32x16(from: &[i32], idx: I32x16) -> I32x16 {
    _mm512_i32gather_epi32::<4>(idx, from.as_ptr() as *const u8)
}

pub unsafe fn scatter_i32x16(to: &mut [i32], idx: I32x16, values: I32x16) {
    _mm512_i32scatter_epi32::<4>(to.as_mut_ptr() as *mut u8, idx, values)
}

pub unsafe fn scatter_i32x9(to: &mut [i32], idx: IndexI32x9, values: I32x9) {
    _mm512_mask_i32scatter_epi32::<4>(to.as_mut_ptr() as *mut u8, 0b111_111_111, idx, values)
}

pub unsafe fn gather_i32x9(to: &[i32], idx: IndexI32x9) -> I32x9 {
    _mm512_i32gather_epi32::<4>(idx, to.as_ptr() as *const u8)
}