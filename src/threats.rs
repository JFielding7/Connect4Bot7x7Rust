use crate::col_shift;
use crate::engine::{is_win, DEFAULT_MOVE_ORDER};
use crate::state::{COLS, COL_MASK};

pub const FOUR_BIT_MASK: u32 = 0b1111;

#[macro_export]
macro_rules! index {
    ($i:expr) => {
        $i << 2
    };
}

macro_rules! get {
    ($arr:expr, $i:expr) => {
        ($arr >> index!($i)) & FOUR_BIT_MASK
    };
}

macro_rules! elements_mask {
    ($length:expr) => {
        (1 << ($length << 2)) - 1
    };
}

macro_rules! slice {
    ($arr:expr, $start:expr, $end:expr) => {
        ($arr >> index!($start)) & elements_mask!($end - $start)
    };
}

macro_rules! slice_clear {
    ($arr:expr, $start:expr, $end:expr) => {
        $arr & !(elements_mask!($end - $start) << index!($start))
    };
}

macro_rules! element_shift {
    ($arr:expr, $old:expr, $new:expr) => {
        get!($arr, $old) << index!($new)
    };
}

macro_rules! slice_shift {
    ($arr:expr, $start:expr, $end:expr, $new:expr) => {
        slice!($arr, $start, $end) << index!($new)
    };
}

pub fn sort_by_threats(col_threats: u32) -> u32 {
    let mut move_order = DEFAULT_MOVE_ORDER;

    for i in 0..COLS {
        let curr_col = get!(move_order, i);
        let curr_threats = get!(col_threats, curr_col);
        let mut j = i;

        while j > 0 && curr_threats > get!(col_threats, get!(move_order, j - 1)) {
            j -= 1;
        }

        move_order = slice_clear!(move_order, j, i + 1)
            | element_shift!(move_order, i, j)
            | slice_shift!(move_order, j, i, j + 1);
    }

    move_order
}

pub fn count_threats(pieces: u64, height_map: u64) -> u32 {
    let mut threat_count = 0;
    
    for col in 0..COLS {
        
        let col_mask = COL_MASK << col_shift!(col);
        let limit = col_mask >> 1;
        
        let mut cell = height_map & col_mask;
        while cell < limit {
            threat_count += is_win(pieces | cell) as u32;
            cell <<= 1;
        }
    }

    threat_count
}
