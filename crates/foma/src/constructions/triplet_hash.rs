//! Wave-4 split of constructions.c (see mod.rs). The triplet-hash pool is
//! self-contained: an open-addressing table keyed by consecutive ints,
//! used by the product/derived constructions for state numbering.

/* An open addressing scheme (with linear probing) to store triplets of ints */
/* and hashing them with an automatically increasing key at every insert     */
/* The table is rehashed whenever occupancy reaches 0.5                      */

// [spec:foma:def:constructions.triplethash-triplets]
#[derive(Debug, Clone)]
pub struct TriplethashTriplets {
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub key: i32,
}

// [spec:foma:def:constructions.triplethash]
#[derive(Debug)]
pub struct Triplethash {
    pub triplets: Vec<TriplethashTriplets>,
    pub tablesize: u32,
    pub occupancy: i32,
}

// [spec:foma:def:constructions.triplet-hash-init-fn]
// [spec:foma:sem:constructions.triplet-hash-init-fn]
pub fn triplet_hash_init() -> Box<Triplethash> {
    let mut th = Box::new(Triplethash {
        tablesize: 128,
        occupancy: 0,
        /* C: malloc'd — the a/b/c fields of empty slots are uninitialized
        (zeroed here); only key is initialized below */
        triplets: Vec::new(),
    });
    th.triplets = vec![
        TriplethashTriplets {
            a: 0,
            b: 0,
            c: 0,
            key: 0,
        };
        th.tablesize as usize
    ];
    let mut i = 0usize;
    while i < th.tablesize as usize {
        th.triplets[i].key = -1;
        i += 1;
    }
    th
}

// [spec:foma:def:constructions.triplethash-hashf-fn]
// [spec:foma:sem:constructions.triplethash-hashf-fn]
pub fn triplethash_hashf(a: i32, b: i32, c: i32) -> u32 {
    /* C: a * 7907 + b * 86028157 + c * 7919 in signed int arithmetic
    (overflow is UB that wraps in practice) — explicit wrapping i32 ops
    reproduce the same slot sequence */
    a.wrapping_mul(7907)
        .wrapping_add(b.wrapping_mul(86028157))
        .wrapping_add(c.wrapping_mul(7919)) as u32
}

// [spec:foma:def:constructions.triplet-hash-free-fn]
// [spec:foma:sem:constructions.triplet-hash-free-fn]
pub fn triplet_hash_free(th: Option<Box<Triplethash>>) {
    if let Some(th) = th {
        /* free(th->triplets); free(th) */
        drop(th);
    }
}

// [spec:foma:def:constructions.triplet-hash-insert-with-key-fn]
// [spec:foma:sem:constructions.triplet-hash-insert-with-key-fn]
pub fn triplet_hash_insert_with_key(th: &mut Triplethash, a: i32, b: i32, c: i32, key: i32) {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    loop {
        if th.triplets[hash as usize].key == -1 {
            th.triplets[hash as usize].key = key;
            th.triplets[hash as usize].a = a;
            th.triplets[hash as usize].b = b;
            th.triplets[hash as usize].c = c;
            break;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
    }
}

// [spec:foma:def:constructions.triplet-hash-insert-fn]
// [spec:foma:sem:constructions.triplet-hash-insert-fn]
pub fn triplet_hash_insert(th: &mut Triplethash, a: i32, b: i32, c: i32) -> i32 {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    loop {
        if th.triplets[hash as usize].key == -1 {
            th.triplets[hash as usize].key = th.occupancy;
            th.triplets[hash as usize].a = a;
            th.triplets[hash as usize].b = b;
            th.triplets[hash as usize].c = c;
            th.occupancy = th.occupancy + 1;
            /* C: int occupancy > unsigned tablesize/2 — the int converts
            to unsigned in the comparison (occupancy is never negative) */
            if th.occupancy as u32 > th.tablesize / 2 {
                triplet_hash_rehash(th);
            }
            return th.occupancy - 1;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
    }
}

// [spec:foma:def:constructions.triplet-hash-rehash-fn]
// [spec:foma:sem:constructions.triplet-hash-rehash-fn]
pub fn triplet_hash_rehash(th: &mut Triplethash) {
    let newtablesize = th.tablesize * 2;
    let oldtablesize = th.tablesize;
    /* C: malloc'd new table (a/b/c uninitialized; zeroed here) */
    let oldtriplets = std::mem::replace(
        &mut th.triplets,
        vec![
            TriplethashTriplets {
                a: 0,
                b: 0,
                c: 0,
                key: 0,
            };
            newtablesize as usize
        ],
    );
    /* tablesize updated BEFORE reinserting so probes use the new size */
    th.tablesize = newtablesize;
    for i in 0..newtablesize as usize {
        th.triplets[i].key = -1;
    }
    for i in 0..oldtablesize as usize {
        if oldtriplets[i].key != -1 {
            triplet_hash_insert_with_key(
                th,
                oldtriplets[i].a,
                oldtriplets[i].b,
                oldtriplets[i].c,
                oldtriplets[i].key,
            );
        }
    }
    /* free(oldtriplets) — dropped here */
}

// [spec:foma:def:constructions.triplet-hash-find-fn]
// [spec:foma:sem:constructions.triplet-hash-find-fn]
pub fn triplet_hash_find(th: &Triplethash, a: i32, b: i32, c: i32) -> i32 {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    let mut j: u32 = 0;
    while j < th.tablesize {
        if th.triplets[hash as usize].key == -1 {
            return -1;
        }
        if th.triplets[hash as usize].a == a
            && th.triplets[hash as usize].b == b
            && th.triplets[hash as usize].c == c
        {
            return th.triplets[hash as usize].key;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
        j += 1;
    }
    -1
}
