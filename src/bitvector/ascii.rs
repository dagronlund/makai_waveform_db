use crate::bitvector::{BitVector, HALF_WORD_BITS, WORD_INDEX_MASK, WORD_INDEX_WIDTH};

impl BitVector {
    pub fn from_ascii(bytes: &[u8]) -> Self {
        let mut bv = Self::new(bytes.len(), false);
        if bv.is_pointer() {
            let mut value = 0usize;
            let mut value_bit = 1usize;
            for i in 0..bytes.len() {
                let byte = bytes[bytes.len() - i - 1];
                value |= if byte == b'1' { value_bit } else { 0 };
                value_bit <<= 1;
                if i & WORD_INDEX_MASK == WORD_INDEX_MASK || i == bytes.len() - 1 {
                    unsafe {
                        *bv.payload.offset((i >> WORD_INDEX_WIDTH) as isize) = value;
                    }
                    value_bit = 1;
                    value = 0;
                }
            }
        } else {
            let mut value = 0usize;
            let mut value_bit = 1usize;
            for i in 0..bytes.len() {
                let byte = bytes[bytes.len() - i - 1];
                value |= if byte == b'1' { value_bit } else { 0 };
                value_bit <<= 1;
            }
            bv.payload = value as *mut usize;
        }
        bv
    }

    pub fn from_ascii_four_state(bytes: &[u8]) -> Self {
        let mut bv = Self::new(bytes.len(), true);
        if bv.is_pointer() {
            let vector_offset = bv.get_vector_words_size();
            let mut value = 0usize;
            let mut mask = 0usize;
            let mut bit = 1usize;
            for i in 0..bytes.len() {
                let byte = bytes[bytes.len() - i - 1];
                match byte {
                    b'1' => value |= bit,
                    b'X' | b'x' => mask |= bit,
                    b'Z' | b'z' => {
                        value |= bit;
                        mask |= bit;
                    }
                    _ => {}
                }
                bit <<= 1;
                if i & WORD_INDEX_MASK == WORD_INDEX_MASK || i == bytes.len() - 1 {
                    unsafe {
                        *bv.payload.offset((i >> WORD_INDEX_WIDTH) as isize) = value;
                        *bv.payload
                            .offset(((i >> WORD_INDEX_WIDTH) + vector_offset) as isize) = mask;
                    }
                    bit = 1;
                    value = 0;
                    mask = 0;
                }
            }
        } else {
            let mut value = 0usize;
            let mut value_bit = 1usize;
            let mut mask_bit = 1usize << HALF_WORD_BITS;
            for i in 0..bytes.len() {
                let byte = bytes[bytes.len() - i - 1];
                match byte {
                    b'1' => value |= value_bit,
                    b'X' | b'x' => value |= mask_bit,
                    b'Z' | b'z' => {
                        value |= value_bit;
                        value |= mask_bit;
                    }
                    _ => {}
                }
                value_bit <<= 1;
                mask_bit <<= 1;
            }
            bv.payload = value as *mut usize;
        }
        bv
    }
}
