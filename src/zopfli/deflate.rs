use std::slice;

use libc::{c_uint, c_int, size_t};

use lz77::{ZopfliLZ77Store, lz77_store_from_c, get_histogram};
use symbols::{ZopfliGetLengthSymbol, ZopfliGetDistSymbol, ZopfliGetLengthSymbolExtraBits, ZopfliGetDistSymbolExtraBits};
use util::{ZOPFLI_NUM_LL};

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn GetFixedTree(ll_lengths: *mut c_uint, d_lengths: *mut c_uint) {
    unsafe {
        for i in 0..144 {
            *ll_lengths.offset(i) = 8;
        }
        for i in 144..256 {
            *ll_lengths.offset(i) = 9;
        }
        for i in 256..280 {
            *ll_lengths.offset(i) = 7;
        }
        for i in 280..288 {
            *ll_lengths.offset(i) = 8;
        }
        for i in 0..32 {
            *d_lengths.offset(i) = 5;
        }
    }
}

/// Changes the population counts in a way that the consequent Huffman tree
/// compression, especially its rle-part, will be more likely to compress this data
/// more efficiently. length contains the size of the histogram.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn OptimizeHuffmanForRle(length: c_int, counts: *mut size_t) {
    let mut length = length as usize;
    let c = unsafe { slice::from_raw_parts(counts, length) };

    // 1) We don't want to touch the trailing zeros. We may break the
    // rules of the format by adding more data in the distance codes.
    loop {
        if length == 0 {
            return;
        }
        if c[length - 1] != 0 {
            // Now counts[0..length - 1] does not have trailing zeros.
            break;
        }
        length -= 1;
    }

    // 2) Let's mark all population counts that already can be encoded
    // with an rle code.
    let mut good_for_rle = vec![0; length as size_t];

    // Let's not spoil any of the existing good rle codes.
    // Mark any seq of 0's that is longer than 5 as a good_for_rle.
    // Mark any seq of non-0's that is longer than 7 as a good_for_rle.
    let mut symbol = c[0];
    let mut stride = 0;
    for i in 0..(length + 1) {
        if i == length || c[i] != symbol {
            if (symbol == 0 && stride >= 5) || (symbol != 0 && stride >= 7) {
                for k in 0..stride {
                    good_for_rle[(i - k - 1) as usize] = 1;
                }
            }
            stride = 1;
            if i != length {
                symbol = c[i];
            }
        } else {
            stride += 1;
        }
    }

    // 3) Let's replace those population counts that lead to more rle codes.
    stride = 0;
    let mut limit = c[0];
    let mut sum = 0;
    for i in 0..(length + 1) {
        // Heuristic for selecting the stride ranges to collapse.
        if i == length || good_for_rle[i as usize] != 0 || (c[i] as i32 - limit as i32).abs() >= 4 {
            if stride >= 4 || (stride >= 3 && sum == 0) {
                // The stride must end, collapse what we have, if we have enough (4).
                let mut count = (sum + stride / 2) / stride;
                if count < 1 {
                    count = 1;
                }
                if sum == 0 {
                    // Don't make an all zeros stride to be upgraded to ones.
                    count = 0;
                }
                for k in 0..stride {
                    // We don't want to change value at counts[i],
                    // that is already belonging to the next stride. Thus - 1.
                    unsafe { *counts.offset((i - k - 1) as isize) = count as size_t };
                }
            }

            stride = 0;
            sum = 0;
            // length > 2 added so it won't underflow; i can't be negative anyway
            if length > 2 && i < length - 3 {
                // All interesting strides have a count of at least 4,
                // at least when non-zeros.
                limit = (c[i] + c[i + 1] + c[i + 2] + c[i + 3] + 2) / 4;
            } else if i < length {
                limit = c[i];
            } else {
                limit = 0;
            }
        }
        stride += 1;
        if i != length {
            sum += c[i];
        }
    }
}

// Ensures there are at least 2 distance codes to support buggy decoders.
// Zlib 1.2.1 and below have a bug where it fails if there isn't at least 1
// distance code (with length > 0), even though it's valid according to the
// deflate spec to have 0 distance codes. On top of that, some mobile phones
// require at least two distance codes. To support these decoders too (but
// potentially at the cost of a few bytes), add dummy code lengths of 1.
// References to this bug can be found in the changelog of
// Zlib 1.2.2 and here: http://www.jonof.id.au/forum/index.php?topic=515.0.
//
// d_lengths: the 32 lengths of the distance codes.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn PatchDistanceCodesForBuggyDecoders(d_lengths: *mut c_uint) {
    let mut num_dist_codes = 0; // Amount of non-zero distance codes
    // Ignore the two unused codes from the spec
    for i in 0..30 {
        if unsafe { *d_lengths.offset(i as isize) } != 0 {
            num_dist_codes += 1;
        }
        // Two or more codes is fine.
        if num_dist_codes >= 2 {
            return;
        }
    }

    if num_dist_codes == 0 {
        unsafe {
            *d_lengths.offset(0) = 1;
            *d_lengths.offset(1) = 1;
        }
    } else if num_dist_codes == 1 {
        unsafe {
            let index = if *d_lengths.offset(0) == 0 {
                0
            } else {
                1
            };
            *d_lengths.offset(index) = 1;
        }
    }
}

/// Same as CalculateBlockSymbolSize, but for block size smaller than histogram
/// size.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn CalculateBlockSymbolSizeSmall(ll_lengths: *const c_uint, d_lengths: *const c_uint, lz77_ptr: *const ZopfliLZ77Store, lstart: size_t, lend: size_t) -> size_t {
    let rust_store = lz77_store_from_c(lz77_ptr);
    let rs = unsafe { &*rust_store };
    let mut result = 0;

    for i in lstart..lend {
        assert!(i < rs.size());
        let litlens_i = rs.litlens[i];
        let dists_i = rs.dists[i];
        assert!(litlens_i < 259);
        if dists_i == 0 {
            result += unsafe { *ll_lengths.offset(litlens_i as isize) };
        } else {
            let ll_symbol = ZopfliGetLengthSymbol(litlens_i as c_int);
            let d_symbol = ZopfliGetDistSymbol(dists_i as c_int);
            result += unsafe { *ll_lengths.offset(ll_symbol as isize) };
            result += unsafe { *d_lengths.offset(d_symbol as isize) };
            result += ZopfliGetLengthSymbolExtraBits(ll_symbol) as c_uint;
            result += ZopfliGetDistSymbolExtraBits(d_symbol) as c_uint;
        }
    }
    result += unsafe { *ll_lengths.offset(256) }; // end symbol
    result as size_t
}

/// Same as CalculateBlockSymbolSize, but with the histogram provided by the caller.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn CalculateBlockSymbolSizeGivenCounts(ll_counts: *const size_t, d_counts: *const size_t, ll_lengths: *const c_uint, d_lengths: *const c_uint, lz77: *const ZopfliLZ77Store, lstart: size_t, lend: size_t) -> size_t {
    let mut result = 0;

    if lstart + ZOPFLI_NUM_LL * 3 > lend {
        return CalculateBlockSymbolSizeSmall(ll_lengths, d_lengths, lz77, lstart, lend);
    } else {
        for i in 0..256 {
            result += unsafe { *ll_lengths.offset(i as isize) * *ll_counts.offset(i as isize) as c_uint };
        }
        for i in 257..286 {
            result += unsafe { *ll_lengths.offset(i as isize) * *ll_counts.offset(i as isize) as c_uint };
            result += (ZopfliGetLengthSymbolExtraBits(i) * unsafe { *ll_counts.offset(i as isize) as c_int }) as c_uint;
        }
        for i in 0..30 {
            result += unsafe { *d_lengths.offset(i as isize) * *d_counts.offset(i as isize) as c_uint };
            result += (ZopfliGetDistSymbolExtraBits(i) * unsafe { *d_counts.offset(i as isize) as c_int }) as c_uint;
        }
        result += unsafe { *ll_lengths.offset(256) }; // end symbol
        result as size_t
    }
}

/*
Calculates size of the part after the header and tree of an LZ77 block, in bits.
*/
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn CalculateBlockSymbolSize(ll_lengths: *const c_uint, d_lengths: *const c_uint, lz77: *const ZopfliLZ77Store, lstart: size_t, lend: size_t) -> size_t {
    if lstart + ZOPFLI_NUM_LL * 3 > lend {
        CalculateBlockSymbolSizeSmall(ll_lengths, d_lengths, lz77, lstart, lend)
    } else {
        let (ll_counts, d_counts) = get_histogram(unsafe { &*lz77 }, lstart, lend);
        CalculateBlockSymbolSizeGivenCounts(ll_counts.as_ptr(), d_counts.as_ptr(), ll_lengths, d_lengths, lz77, lstart, lend)
    }
}
