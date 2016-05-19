use std::cmp::Ordering;

use libc::{size_t, c_int, c_uint};

// Bounded package merge algorithm, based on the paper
// "A Fast and Space-Economical Algorithm for Length-Limited Coding
// Jyrki Katajainen, Alistair Moffat, Andrew Turpin".

/// Nodes forming chains.
#[repr(C)]
pub struct Node {
  weight: size_t,     // Total weight (symbol count) of this chain.
  tail: *const Node,  // Previous node(s) of this chain, or 0 if none.
  count: c_int,       // Leaf symbol index, or number of leaves before this chain.
}

#[repr(C)]
pub struct Leaf {
  weight: size_t,     // Total weight (symbol count) of this chain.
  count: c_int,       // Leaf symbol index, or number of leaves before this chain.
}

/// Memory pool for nodes.
#[repr(C)]
pub struct NodePool {
  next: *const Node,   // Pointer to a free node in the pool
}

/// Initializes a chain node with the given values and marks it as in use.
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn InitNode(weight: size_t, count: c_int, tail: *const Node, node_ptr: *mut Node) {
    let node = unsafe {
        assert!(!node_ptr.is_null());
        &mut *node_ptr
    };

    node.weight = weight;
    node.count = count;
    node.tail = tail;
}

/// Converts result of boundary package-merge to the bitlengths. The result in the
/// last chain of the last list contains the amount of active leaves in each list.
/// chain: Chain to extract the bit length from (last chain from last list).
#[no_mangle]
#[allow(non_snake_case)]
pub extern fn ExtractBitLengths(chain: *const Node, leaves: *const Leaf, bitlengths: *mut c_uint) {
    let mut counts = [0; 16];
    let mut end = 16;
    let mut ptr = 15;
    let mut value = 1;

    let mut node_ptr = chain;
    while !node_ptr.is_null() {
        let node = unsafe {
           &*node_ptr
        };

        end -= 1;
        counts[end] = node.count;

        node_ptr = node.tail;
    }

    let mut val = counts[15];
    while ptr >= end {
        while val > counts[ptr - 1] {
            unsafe {
                let leaf = &*leaves.offset((val - 1) as isize);
                *bitlengths.offset(leaf.count as isize) = value;
            }
            val -= 1;
        }
        ptr -= 1;
        value += 1;
    }
}

struct N {
    weight: size_t,
    leaf_count: c_int,
}

struct L {
    pub weight: size_t,
    pub index: size_t,
}
impl PartialEq for L {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}
impl Eq for L { }
impl Ord for L {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
impl PartialOrd for L {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


struct List {
    lookahead1: N,
    lookahead2: N,
    last_active: N,
    next_leaf_index: size_t,
}

pub fn length_limited_code_lengths(frequencies: &[size_t], maxbits: c_int) -> Vec<size_t> {
    let mut leaves = vec![];

    // Count used symbols and place them in the leaves.
    for (i, &freq) in frequencies.iter().enumerate() {
        if freq != 0 {
            leaves.push(L { weight: freq, index: i });
        }
    }

    // Sort the leaves from least frequent to most frequent.
    // Add index into the same variable for stable sorting.
    for leaf in leaves.iter_mut() {
        leaf.weight = (leaf.weight << 9) | leaf.index;
    }
    leaves.sort();
    for leaf in leaves.iter_mut() {
        leaf.weight >>= 9;
    }

    let mut lists = Vec::with_capacity(maxbits as usize);
    for _ in 0..maxbits {
        lists.push(List {
            lookahead1: N { weight: leaves[0].weight, leaf_count: 1 },
            lookahead2: N { weight: leaves[1].weight, leaf_count: 2 },
            last_active: N { weight: leaves[1].weight, leaf_count: 2 },
            next_leaf_index: 2,
        });
    }




    let n = frequencies.len();

    let mut result = vec![0; n];
    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_from_paper_3() {
        let input = [1, 1, 5, 7, 10, 14];
        let output = length_limited_code_lengths(&input, 3);
        let answer = vec![3, 3, 3, 3, 2, 2];
        assert_eq!(output, answer);
    }

    #[test]
    fn test_from_paper_4() {
        let input = [1, 1, 5, 7, 10, 14];
        let output = length_limited_code_lengths(&input, 4);
        let answer = vec![4, 4, 3, 2, 2, 2];
        assert_eq!(output, answer);
    }

    // #[test]
    // fn one_test() {
    //     let input = [252, 0, 1, 6, 9, 10, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    //     let output = length_limited_code_lengths(&input, 7);
    //     let answer = vec![1, 0, 6, 4, 3, 3, 3, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    //     assert_eq!(output, answer);
    // }
}

// maxbits: 7
// frequencies: [252, 0, 1, 6, 9, 10, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ]
// bitlengths: [1, 0, 6, 4, 3, 3, 3, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ]
// maxbits: 7
// frequencies: [10, 0, 1, 6, 9, 4, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, ]
// bitlengths: [3, 0, 6, 4, 3, 4, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, ]
// maxbits: 7
// frequencies: [4, 0, 1, 6, 9, 10, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 26, 0, ]
// bitlengths: [4, 0, 6, 4, 3, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, ]
// maxbits: 7
// frequencies: [4, 0, 1, 6, 9, 4, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 1, 26, 0, ]
// bitlengths: [4, 0, 6, 4, 3, 4, 4, 4, 5, 0, 0, 0, 0, 0, 0, 0, 6, 1, 0, ]
// maxbits: 7
// frequencies: [19, 0, 1, 6, 9, 10, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, ]
// bitlengths: [2, 0, 6, 3, 3, 2, 3, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, ]
// maxbits: 7
// frequencies: [8, 0, 1, 6, 9, 4, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 3, ]
// bitlengths: [3, 0, 5, 3, 2, 3, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0, 4, 0, 4, ]
// maxbits: 7
// frequencies: [4, 0, 1, 6, 9, 10, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, ]
// bitlengths: [4, 0, 6, 3, 2, 2, 3, 4, 6, 0, 0, 0, 0, 0, 0, 0, 0, 5, 4, ]
// maxbits: 7
// frequencies: [4, 0, 1, 6, 9, 4, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, ]
// bitlengths: [3, 0, 6, 3, 2, 3, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0, 6, 4, 4, ]
// maxbits: 15
// frequencies: [0, 0, 0, 0, 0, 0, 18, 0, 6, 0, 12, 2, 14, 9, 27, 15, 23, 15, 17, 8, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ]
// bitlengths: [0, 0, 0, 0, 0, 0, 3, 0, 5, 0, 4, 6, 4, 4, 3, 4, 3, 3, 3, 4, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ]
