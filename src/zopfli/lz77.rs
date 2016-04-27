use std::slice;

use libc::{size_t, c_ushort, c_uchar, c_int};

use cache::ZopfliLongestMatchCache;
use symbols::{ZopfliGetLengthSymbol, ZopfliGetDistSymbol, ZOPFLI_NUM_LL, ZOPFLI_NUM_D};
use zopfli::ZopfliOptions;

// Comment from C:
// Stores lit/length and dist pairs for LZ77.
// Parameter litlens: Contains the literal symbols or length values.
// Parameter dists: Contains the distances. A value is 0 to indicate that there is
// no dist and the corresponding litlens value is a literal instead of a length.
// Parameter size: The size of both the litlens and dists arrays.
// The memory can best be managed by using ZopfliInitLZ77Store to initialize it,
// ZopfliCleanLZ77Store to destroy it, and ZopfliStoreLitLenDist to append values.

#[repr(C)]
pub struct ZopfliLZ77Store {
  litlens: *mut c_ushort,  /* Lit or len. */
  dists: *mut c_ushort,  /* If 0: indicates literal in corresponding litlens,
      if > 0: length in corresponding litlens, this is the distance. */
  size: size_t,

  data: *mut c_uchar,  /* original data */
  pos: *mut size_t,  /* position in data where this LZ77 command begins */

  ll_symbol: *mut c_ushort,
  d_symbol: *mut c_ushort,

  /* Cumulative histograms wrapping around per chunk. Each chunk has the amount
  of distinct symbols as length, so using 1 value per LZ77 symbol, we have a
  precise histogram at every N symbols, and the rest can be calculated by
  looping through the actual symbols of this chunk. */
  ll_counts: *mut size_t,
  d_counts: *mut size_t,
}

#[derive(Debug)]
#[repr(C)]
pub struct Lz77Store {
   pub litlens: Vec<c_ushort>,
   pub dists: Vec<c_ushort>,

   pos: Vec<size_t>,

   ll_symbol: Vec<c_ushort>,
   d_symbol: Vec<c_ushort>,

   ll_counts: Vec<size_t>,
   d_counts: Vec<size_t>,
}

impl Lz77Store {
    pub fn new() -> Lz77Store {
        Lz77Store {
          litlens: vec![],
          dists: vec![],

          pos: vec![],

          ll_symbol: vec![],
          d_symbol: vec![],

          ll_counts: vec![],
          d_counts: vec![],
       }
    }

    pub fn lit_len_dist(&mut self, length: c_ushort, dist: c_ushort, pos: size_t) {
        let origsize = self.litlens.len();
        let llstart = ZOPFLI_NUM_LL * (origsize / ZOPFLI_NUM_LL);
        let dstart = ZOPFLI_NUM_D * (origsize / ZOPFLI_NUM_D);

        if origsize % ZOPFLI_NUM_LL == 0 {
            for i in 0..ZOPFLI_NUM_LL {
                if origsize == 0 {
                    self.ll_counts.push(0);
                } else {
                    let last_histogram_value = self.ll_counts[origsize - ZOPFLI_NUM_LL + i];
                    self.ll_counts.push(last_histogram_value);
                }
            }
        }

        if origsize % ZOPFLI_NUM_D == 0 {
            for i in 0..ZOPFLI_NUM_D {
                if origsize == 0 {
                    self.d_counts.push(0);
                } else {
                    let last_histogram_value = self.d_counts[origsize - ZOPFLI_NUM_D + i];
                    self.d_counts.push(last_histogram_value);
                }
            }
        }

        self.litlens.push(length);
        self.dists.push(dist);
        self.pos.push(pos);

        // Why isn't this at the beginning of this function?
        // assert(length < 259);

        if dist == 0 {
            self.ll_symbol.push(length);
            self.d_symbol.push(0);
            self.ll_counts[llstart + length as usize] += 1;
        } else {
            self.ll_symbol.push(ZopfliGetLengthSymbol(length as c_int) as c_ushort);
            self.d_symbol.push(ZopfliGetDistSymbol(dist as c_int) as c_ushort);
            self.ll_counts[llstart + ZopfliGetLengthSymbol(length as c_int) as usize] += 1;
            self.d_counts[dstart + ZopfliGetDistSymbol(dist as c_int) as usize] += 1;
        }
    }
}

#[no_mangle]
pub extern fn lz77_store_new() -> *mut Lz77Store {
    Box::into_raw(Box::new(Lz77Store::new()))
}

#[no_mangle]
pub extern fn lz77_store_free(ptr: *mut Lz77Store) {
    if ptr.is_null() { return }
    unsafe { Box::from_raw(ptr); }
}

#[no_mangle]
pub extern fn lz77_store_lit_len_dist(ptr: *mut Lz77Store, length: c_ushort, dist: c_ushort, pos: size_t) {
    let store = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };
    store.lit_len_dist(length, dist, pos);
}

#[no_mangle]
pub extern fn lz77_store_from_c(store: *const ZopfliLZ77Store) -> *mut Lz77Store {
    Box::into_raw(Box::new(store.into()))
}

impl From<*const ZopfliLZ77Store> for Lz77Store {
    fn from(ptr: *const ZopfliLZ77Store) -> Lz77Store {
        let store = unsafe {
            assert!(!ptr.is_null());
            &*ptr
        };

        let len = store.size as usize;
        let ll_len = (ZOPFLI_NUM_LL * (store.size / ZOPFLI_NUM_LL) + ZOPFLI_NUM_LL) as usize;
        let d_len = (ZOPFLI_NUM_D * (store.size / ZOPFLI_NUM_D) + ZOPFLI_NUM_D) as usize;

        Lz77Store {
            litlens: ptr_to_vec(store.litlens, len),
            dists: ptr_to_vec(store.dists, len),

            pos: ptr_to_vec(store.pos, len),

            ll_symbol: ptr_to_vec(store.ll_symbol, len),
            d_symbol: ptr_to_vec(store.d_symbol, len),

            ll_counts: ptr_to_vec(store.ll_counts, ll_len),
            d_counts: ptr_to_vec(store.d_counts, d_len),
        }
    }
}

fn ptr_to_vec<T: Clone>(ptr: *mut T, length: usize) -> Vec<T> {
    if ptr.is_null() {
        vec![]
    } else {
        unsafe { slice::from_raw_parts(ptr, length).to_vec() }
    }
}

#[no_mangle]
pub extern fn lz77_store_result(ptr: *mut Lz77Store, store: &mut ZopfliLZ77Store) {
    let lz77 = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    let len = lz77.litlens.len();

    store.litlens = lz77.litlens.as_mut_ptr();
    store.dists = lz77.dists.as_mut_ptr();
    store.size = len;
    store.pos = lz77.pos.as_mut_ptr();
    store.ll_symbol = lz77.ll_symbol.as_mut_ptr();
    store.d_symbol = lz77.d_symbol.as_mut_ptr();
    store.ll_counts = lz77.ll_counts.as_mut_ptr();
    store.d_counts = lz77.d_counts.as_mut_ptr();
}

/// Some state information for compressing a block.
/// This is currently a bit under-used (with mainly only the longest match cache),
/// but is kept for easy future expansion.
#[repr(C)]
pub struct ZopfliBlockState {
    options: *const ZopfliOptions,
    /* Cache for length/distance pairs found so far. */
    lmc: *mut ZopfliLongestMatchCache,
    /* The start (inclusive) and end (not inclusive) of the current block. */
    blockstart: size_t,
    blockend: size_t,
}
