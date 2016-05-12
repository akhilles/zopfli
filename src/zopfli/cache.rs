use std::mem;

use libc::{c_ushort, c_uchar, size_t, c_uint, malloc, free, c_void};

use util::{ZOPFLI_CACHE_LENGTH};

// Cache used by ZopfliFindLongestMatch to remember previously found length/dist
// values.
// This is needed because the squeeze runs will ask these values multiple times for
// the same position.
// Uses large amounts of memory, since it has to remember the distance belonging
// to every possible shorter-than-the-best length (the so called "sublen" array).
pub struct ZopfliLongestMatchCache {
    length: *mut c_ushort,
    dist: *mut c_ushort,
    sublen: *mut c_uchar,
}

impl ZopfliLongestMatchCache {
    pub fn new(blocksize: size_t) -> ZopfliLongestMatchCache {
        unsafe {
            let lmc = ZopfliLongestMatchCache {
                length: malloc(mem::size_of::<c_ushort>() as size_t * blocksize) as *mut c_ushort,
                dist: malloc(mem::size_of::<c_ushort>() as size_t * blocksize) as *mut c_ushort,
                /* Rather large amount of memory. */
                sublen: malloc(ZOPFLI_CACHE_LENGTH * 3 * blocksize) as *mut c_uchar,
            };
            /* length > 0 and dist 0 is invalid combination, which indicates on purpose
            that this cache value is not filled in yet. */
            for i in 0..blocksize as isize {
                *lmc.length.offset(i) = 1;
                *lmc.dist.offset(i) = 0;
            }

            for i in 0..(ZOPFLI_CACHE_LENGTH * blocksize * 3) as isize {
                *lmc.sublen.offset(i) = 0;
            }
            lmc
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliInitCache(blocksize: size_t) -> *mut ZopfliLongestMatchCache {
    Box::into_raw(Box::new(ZopfliLongestMatchCache::new(blocksize)))
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliCleanCache(lmc_ptr: *mut ZopfliLongestMatchCache) {
    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };
    unsafe {
        free(lmc.length as *mut c_void);
        free(lmc.dist as *mut c_void);
        free(lmc.sublen as *mut c_void);
    }
}

/// Returns the length up to which could be stored in the cache.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliMaxCachedSublen(lmc_ptr: *mut ZopfliLongestMatchCache, pos: size_t, _length: size_t) -> c_uint {

    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };

    unsafe {
        let start = (ZOPFLI_CACHE_LENGTH * pos * 3) as isize;
        if *lmc.sublen.offset(start + 1) == 0 && *lmc.sublen.offset(start + 2) == 0 {
            return 0;  // No sublen cached.
        }
        *lmc.sublen.offset(start + ((ZOPFLI_CACHE_LENGTH - 1) * 3) as isize) as c_uint + 3
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliCacheToSublen(lmc_ptr: *mut ZopfliLongestMatchCache, pos: size_t, length: size_t, sublen: *mut c_ushort) {
    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };

    let maxlength = ZopfliMaxCachedSublen(lmc_ptr, pos, length);
    let mut prevlength = 0;

    if length < 3 {
        return;
    }

    unsafe {
        let start = (ZOPFLI_CACHE_LENGTH * pos * 3) as isize;

        for j in 0..ZOPFLI_CACHE_LENGTH {
            let length = *lmc.sublen.offset(start + (j * 3) as isize) as c_uint + 3;
            let dist = *lmc.sublen.offset(start + (j * 3 + 1) as isize) as c_ushort + 256 * *lmc.sublen.offset(start + (j * 3 + 2) as isize) as c_ushort;

            let mut i = prevlength;
            while i <= length {
                *sublen.offset(i as isize) = dist;
                i += 1;
            }
            if length == maxlength {
                break;
            }
            prevlength = length + 1;
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliSublenToCache(sublen: *mut c_ushort, pos: size_t, length: size_t, lmc_ptr: *mut ZopfliLongestMatchCache) {
    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };

    let mut j: isize = 0;
    let mut bestlength: c_uint = 0;

    if length < 3 {
        return;
    }

    unsafe {
        let start = (ZOPFLI_CACHE_LENGTH * pos * 3) as isize;

        let mut i: isize = 3;
        while i <= length as isize {
            if i == length as isize || *sublen.offset(i) != *sublen.offset(i + 1) {
                *lmc.sublen.offset(start + (j * 3) as isize) = (i - 3) as c_uchar;
                *lmc.sublen.offset(start + (j * 3 + 1) as isize) = (*sublen.offset(i)).wrapping_rem(256) as c_uchar;
                *lmc.sublen.offset(start + (j * 3 + 2) as isize) = ((*sublen.offset(i) >> 8)).wrapping_rem(256) as c_uchar;
                bestlength = i as c_uint;
                j += 1;
                if j >= ZOPFLI_CACHE_LENGTH as isize {
                    break;
                }
            }
            i += 1;
        }

        if j < ZOPFLI_CACHE_LENGTH as isize {
            assert!(bestlength == length as c_uint);
            *lmc.sublen.offset(start + ((ZOPFLI_CACHE_LENGTH - 1) * 3) as isize) = (bestlength - 3) as c_uchar;
        } else {
            assert!(bestlength <= length as c_uint);
        }
        assert!(bestlength == ZopfliMaxCachedSublen(lmc, pos, length));
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliCacheLengthAt(lmc_ptr: *mut ZopfliLongestMatchCache, pos: size_t) -> c_ushort {
    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };
    unsafe {
        *lmc.length.offset(pos as isize)
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ZopfliCacheDistAt(lmc_ptr: *mut ZopfliLongestMatchCache, pos: size_t) -> c_ushort {
    let lmc = unsafe {
        assert!(!lmc_ptr.is_null());
        &mut *lmc_ptr
    };
    unsafe {
        *lmc.dist.offset(pos as isize)
    }
}
