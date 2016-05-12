use libc::{c_void, c_uint, c_double, c_int};

use util::{ZopfliGetLengthSymbol, ZopfliGetLengthExtraBits, ZopfliGetDistExtraBits};

#[no_mangle]
#[allow(non_snake_case)]
/// Cost model which should exactly match fixed tree.
/// type: CostModelFun
pub extern fn GetCostFixed(litlen: c_uint, dist: c_uint, _unused: c_void) -> c_double {
    let result = if dist == 0 {
        if litlen <= 143 {
            8
        } else {
            9
        }
    } else {
        let dbits = ZopfliGetDistExtraBits(dist as c_int);
        let lbits = ZopfliGetLengthExtraBits(litlen as c_int);
        let lsym = ZopfliGetLengthSymbol(litlen as c_int);
        let mut cost = 0;
        if lsym <= 279 {
            cost += 7;
        } else {
            cost += 8;
        }
        cost += 5;  // Every dist symbol has length 5.
        cost + dbits + lbits
    };
    result as c_double
}

#[repr(C)]
pub struct RanState {
    m_w: c_uint,
    m_z: c_uint,
}

#[no_mangle]
#[allow(non_snake_case)]
/// Get random number: "Multiply-With-Carry" generator of G. Marsaglia
pub extern fn Ran(state_ptr: *mut RanState) -> c_uint {
    let state = unsafe {
        assert!(!state_ptr.is_null());
        &mut *state_ptr
    };

    state.m_z = 36969 * (state.m_z & 65535) + (state.m_z >> 16);
    state.m_w = 18000 * (state.m_w & 65535) + (state.m_w >> 16);
    (state.m_z << 16) + state.m_w // 32-bit result.
}
