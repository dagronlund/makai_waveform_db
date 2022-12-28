mod ascii;
mod format;
mod integers;
mod iter;
mod tests;

use std::alloc;
use std::convert::TryInto;

use indiscriminant::*;

pub use crate::bitvector::ascii::*;
pub use crate::bitvector::format::*;
pub use crate::bitvector::integers::*;
pub use crate::bitvector::iter::*;

// Concisely stores two or four state bit-vectors using one pointer-sized value
// to indicate the bit-width of the bit-vector, whether it is two or four state
// encoded, and if the payload is a pointer or the actual vector value. This
// approach optimizes most heavily for <64-bit two-state vectors, but provides a
// performant data structure for bigger vectors.

#[indiscriminant()]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BitVectorRadix {
    Binary = "b",
    Octal = "o",
    Decimal = "d",
    Hexadecimal = "h",
}

#[indiscriminant()]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Logic {
    Zero = "0",
    One = "1",
    Unknown = "X",
    HighImpedance = "Z",
}

impl Logic {
    pub fn is_two_state(&self) -> bool {
        match self {
            Self::Zero | Self::One => true,
            _ => false,
        }
    }

    pub fn to_bool_pair(&self) -> (bool, bool) {
        match self {
            Self::Zero => (false, false),
            Self::One => (true, false),
            Self::Unknown => (false, true),
            Self::HighImpedance => (true, true),
        }
    }
}

#[indiscriminant()]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Bit {
    Zero = "0",
    One = "1",
}

impl From<Bit> for Logic {
    fn from(value: Bit) -> Self {
        match value {
            Bit::Zero => Self::Zero,
            Bit::One => Self::One,
        }
    }
}

impl From<Bit> for bool {
    fn from(value: Bit) -> Self {
        match value {
            Bit::Zero => false,
            Bit::One => true,
        }
    }
}

impl From<Logic> for bool {
    fn from(value: Logic) -> Self {
        match value {
            Logic::One => true,
            _ => false,
        }
    }
}

impl From<Logic> for Bit {
    fn from(value: Logic) -> Self {
        match value {
            Logic::One => Self::One,
            _ => Self::Zero,
        }
    }
}

impl From<(bool, bool)> for Logic {
    fn from(value: (bool, bool)) -> Self {
        match value {
            (false, false) => Self::Zero,
            (true, false) => Self::One,
            (false, true) => Self::Unknown,
            (true, true) => Self::HighImpedance,
        }
    }
}

impl From<bool> for Logic {
    fn from(value: bool) -> Self {
        Self::from((value, false))
    }
}

impl From<(usize, usize)> for Logic {
    fn from(value: (usize, usize)) -> Self {
        Self::from((value.0 != 0, value.1 != 0))
    }
}

impl From<bool> for Bit {
    fn from(value: bool) -> Self {
        match value {
            false => Self::Zero,
            true => Self::One,
        }
    }
}

impl From<usize> for Bit {
    fn from(value: usize) -> Self {
        Self::from(value != 0)
    }
}

pub struct BitVector {
    size: usize,
    payload: *mut usize,
}

unsafe impl Send for BitVector {}
unsafe impl Sync for BitVector {}

const USIZE_BYTES: usize = (usize::BITS / 8) as usize;
const WORD_INDEX_WIDTH: usize = (31 - usize::BITS.leading_zeros()) as usize;
const WORD_INDEX_MASK: usize = (1usize << WORD_INDEX_WIDTH) - 1;
const HALF_WORD_BITS: usize = (usize::BITS / 2) as usize;
const HALF_WORD_MASK: usize = (1usize << HALF_WORD_BITS) - 1;
const POINTER_TAG: usize = 1 << (usize::BITS - 1);
const FOUR_STATE_TAG: usize = 1 << (usize::BITS - 2);

unsafe fn clone_be_bytes_to_usizes(bytes: &[u8], ptr: *mut usize) {
    let mut ptr_index = 0usize;
    let mut byte_index = bytes.len();
    loop {
        if byte_index <= USIZE_BYTES {
            let mut temp = [0u8; USIZE_BYTES];
            temp[(USIZE_BYTES - byte_index)..USIZE_BYTES].clone_from_slice(&bytes[0..byte_index]);
            let w = usize::from_be_bytes(temp) as usize;
            *ptr.offset(ptr_index as isize) = w;
            break;
        } else {
            let w = usize::from_be_bytes(
                (&bytes[byte_index - USIZE_BYTES..byte_index])
                    .try_into()
                    .unwrap(),
            ) as usize;
            *ptr.offset(ptr_index as isize) = w;
        }
        ptr_index += 1;
        byte_index -= USIZE_BYTES;
    }
}

unsafe fn clone_usizes_to_be_bytes(ptr: *const usize, bytes: &mut [u8]) {
    let mut ptr_index = 0usize;
    let mut byte_index = bytes.len();
    loop {
        let w = *ptr.offset(ptr_index as isize);
        if byte_index <= USIZE_BYTES {
            bytes[0..byte_index]
                .clone_from_slice(&w.to_be_bytes()[(USIZE_BYTES - byte_index)..USIZE_BYTES]);
            break;
        } else {
            bytes[byte_index - USIZE_BYTES..byte_index].clone_from_slice(&w.to_be_bytes());
        }
        ptr_index += 1;
        byte_index -= USIZE_BYTES;
    }
}

impl BitVector {
    pub fn new(bit_width: usize, four_state: bool) -> Self {
        assert!(
            bit_width < (1 << (usize::BITS - 2)),
            "Bit width too large: {} bits",
            bit_width
        );
        let (words, size) = if four_state {
            if bit_width * 2 > (usize::BITS as usize) {
                // Size storage for two bit_width allocation in chunks of usize
                let words = (((bit_width - 1) / (usize::BITS as usize)) + 1) * 2;
                (words, bit_width | POINTER_TAG | FOUR_STATE_TAG)
            } else {
                return Self {
                    size: bit_width | FOUR_STATE_TAG,
                    payload: 0 as *mut usize,
                };
            }
        } else {
            if bit_width > (usize::BITS as usize) {
                // Size storage for one bit_width allocation in chunks of usize
                let words = ((bit_width - 1) / (usize::BITS as usize)) + 1;
                (words, bit_width | POINTER_TAG)
            } else {
                return Self {
                    size: bit_width,
                    payload: 0 as *mut usize,
                };
            }
        };

        // Allocate memory for bitvector
        let layout = alloc::Layout::array::<usize>(words).unwrap();
        assert!(
            layout.size() <= isize::MAX as usize,
            "Allocation too large: {} bytes",
            layout.size()
        );
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        Self {
            size: size,
            payload: ptr as *mut usize,
        }
    }

    pub fn new_zero_bit() -> Self {
        Self {
            size: 1,
            payload: 0 as *mut usize,
        }
    }

    pub fn new_one_bit() -> Self {
        Self {
            size: 1,
            payload: 1 as *mut usize,
        }
    }

    pub fn new_unknown_bit() -> Self {
        Self {
            size: 1 | FOUR_STATE_TAG,
            payload: ((1usize << (usize::BITS / 2)) | 0) as *mut usize,
        }
    }

    pub fn new_high_impedance_bit() -> Self {
        Self {
            size: 1 | FOUR_STATE_TAG,
            payload: ((1usize << (usize::BITS / 2)) | 1) as *mut usize,
        }
    }

    // From/To Integral Bit Types

    pub fn from_bits_two_state<T: UnsignedInteger>(bit_width: usize, value: T) -> Self {
        assert!(
            bit_width <= usize::BITS as usize,
            "Bit width too large to fit in usize: {} bits",
            bit_width
        );
        Self {
            size: bit_width,
            payload: value.to_usize() as *mut usize,
        }
    }

    pub fn from_bits_four_state<T: UnsignedInteger>(bit_width: usize, value: T, mask: T) -> Self {
        assert!(
            bit_width <= (usize::BITS / 2) as usize,
            "Bit width too large to fit in usize: {} bits",
            bit_width
        );
        Self {
            size: bit_width | FOUR_STATE_TAG,
            payload: (value.to_usize() & HALF_WORD_MASK | mask.to_usize() << (usize::BITS / 2))
                as *mut usize,
        }
    }

    pub fn to_bits_two_state<T: UnsignedInteger>(&self) -> T {
        // Usize is an intermediate, make sure T isn't wider (basically prevents using u128)
        assert!(usize::BITS >= T::from_usize(0).get_width());
        assert!(
            self.get_bit_width() <= T::from_usize(0).get_width() as usize,
            "Bit width too large to fit in T: {} bits > {} bits",
            self.get_bit_width(),
            T::from_usize(0).get_width() as usize
        );
        assert!(!self.is_four_state());
        assert!(!self.is_pointer());
        T::from_usize(self.payload as usize)
    }

    pub fn to_bits_four_state<T: UnsignedInteger>(&self) -> (T, T) {
        // Usize is an intermediate, make sure T isn't wider (basically prevents using u128)
        assert!(usize::BITS >= T::from_usize(0).get_width());
        assert!(
            self.get_bit_width() <= T::from_usize(0).get_width() as usize,
            "Bit width too large to fit in T: {} bits > {} bits",
            self.get_bit_width(),
            T::from_usize(0).get_width() as usize
        );
        if !self.is_four_state() {
            (self.to_bits_two_state::<T>(), T::from_usize(0))
        } else {
            (
                T::from_usize((self.payload as usize) & HALF_WORD_MASK),
                T::from_usize((self.payload as usize) >> (usize::BITS / 2)),
            )
        }
    }

    // From/To Integral Big-Endian Byte Arrays

    pub fn from_be_bytes_two_state(bit_width: usize, value: &[u8]) -> Self {
        let byte_width = ((bit_width - 1) / 8) + 1;
        assert!(
            value.len() == byte_width,
            "Value bytes length ({}) does not match expected byte length ({})!",
            value.len(),
            byte_width
        );
        let mut bv = Self::new(bit_width, false);
        if bv.is_pointer() {
            unsafe {
                clone_be_bytes_to_usizes(value, bv.payload);
            }
        } else {
            let mut payload = 0usize;
            for b in value {
                payload <<= 8;
                payload |= *b as usize;
            }
            bv.payload = payload as *mut usize;
        }
        bv
    }

    pub fn from_be_bytes_four_state(bit_width: usize, value: &[u8], mask: &[u8]) -> Self {
        let byte_width = ((bit_width - 1) / 8) + 1;
        assert!(
            value.len() == byte_width,
            "Value bytes length ({}) does not match expected byte length ({})!",
            value.len(),
            byte_width
        );
        assert!(
            value.len() == mask.len(),
            "Value and mask bytes length mismatch: {} bytes != {} bytes",
            value.len(),
            mask.len(),
        );
        let mut bv = Self::new(bit_width, true);
        if bv.is_pointer() {
            unsafe {
                clone_be_bytes_to_usizes(value, bv.payload);
                clone_be_bytes_to_usizes(
                    mask,
                    bv.payload.offset(bv.get_vector_words_size() as isize),
                );
            }
        } else {
            let mut payload = 0usize;
            for i in 0..value.len() {
                payload <<= 8;
                payload |= value[i] as usize;
                payload |= (mask[i] as usize) << (usize::BITS as usize / 2);
            }
            bv.payload = payload as *mut usize;
        }
        bv
    }

    pub fn to_be_bytes_two_state(&self, value: &mut [u8]) {
        let byte_width = ((self.get_bit_width() - 1) / 8) + 1;
        assert!(
            value.len() == byte_width,
            "Value bytes length ({}) does not match expected byte length ({})!",
            value.len(),
            byte_width
        );
        assert!(!self.is_four_state());
        if self.is_pointer() {
            unsafe {
                clone_usizes_to_be_bytes(self.payload as *const usize, value);
            }
        } else {
            let mut payload = self.payload as usize;
            for i in (0..value.len()).rev() {
                value[i] = (payload & 0xff) as u8;
                payload >>= 8;
            }
        }
    }

    pub fn to_be_bytes_four_state(&self, value: &mut [u8], mask: &mut [u8]) {
        let byte_width = ((self.get_bit_width() - 1) / 8) + 1;
        assert!(
            value.len() == byte_width,
            "Value bytes length ({}) does not match expected byte length ({})!",
            value.len(),
            byte_width
        );
        assert!(
            value.len() == mask.len(),
            "Value and mask bytes length mismatch: {} bytes != {} bytes",
            value.len(),
            mask.len(),
        );
        if !self.is_four_state() {
            self.to_be_bytes_two_state(value);
            for i in 0..mask.len() {
                mask[i] = 0;
            }
            return;
        }
        if self.is_pointer() {
            unsafe {
                clone_usizes_to_be_bytes(self.payload as *const usize, value);
                clone_usizes_to_be_bytes(
                    self.payload.offset(self.get_vector_words_size() as isize) as *const usize,
                    mask,
                );
            }
        } else {
            let mut payload = self.payload as usize;
            for i in (0..value.len()).rev() {
                value[i] = (payload & 0xff) as u8;
                mask[i] = ((payload >> (usize::BITS as usize / 2)) & 0xff) as u8;
                payload >>= 8;
            }
        }
    }

    // Bit Manipulation Functions

    fn set_bit_four_state_internal(&mut self, index: usize, bit: Logic) {
        if self.is_pointer() {
            let word_index = index / usize::BITS as usize;
            let word_index_mask = word_index + self.get_vector_words_size();
            let bit_index = index % usize::BITS as usize;
            let bit_mask = 1usize << bit_index;
            unsafe {
                let value_bits = *self.payload.offset(word_index as isize);
                let mask_bits = *self.payload.offset(word_index_mask as isize);
                let value_bits =
                    (value_bits & !bit_mask) | if bit.to_bool_pair().0 { bit_mask } else { 0 };
                let mask_bits =
                    (mask_bits & !bit_mask) | if bit.to_bool_pair().1 { bit_mask } else { 0 };
                *self.payload.offset(word_index as isize) = value_bits;
                *self.payload.offset(word_index_mask as isize) = mask_bits;
            }
        } else {
            let value_mask = 1usize << index;
            let mask_mask = 1usize << (index + HALF_WORD_BITS);
            let bits = (self.payload as usize & !(value_mask | mask_mask))
                | if bit.to_bool_pair().0 { value_mask } else { 0 }
                | if bit.to_bool_pair().1 { mask_mask } else { 0 };
            self.payload = bits as *mut usize;
        }
    }

    fn get_bit_four_state_internal(&self, index: usize) -> Logic {
        if self.is_pointer() {
            let word_index = index / usize::BITS as usize;
            let word_index_mask = word_index + self.get_vector_words_size();
            let bit_index = index % usize::BITS as usize;
            let value_bits = unsafe { *self.payload.offset(word_index as isize) };
            let mask_bits = unsafe { *self.payload.offset(word_index_mask as isize) };
            Logic::from((
                ((value_bits >> bit_index) & 1),
                ((mask_bits >> bit_index) & 1),
            ))
        } else {
            let bits = self.payload as usize;
            Logic::from((
                ((bits >> index) & 1),
                ((bits >> (index + HALF_WORD_BITS)) & 1),
            ))
        }
    }

    fn set_bit_two_state_internal(&mut self, index: usize, bit: Bit) {
        if self.is_pointer() {
            let word_index = index / usize::BITS as usize;
            let bit_index = index % usize::BITS as usize;
            let bit_mask = 1usize << bit_index;
            unsafe {
                let value_bits = *self.payload.offset(word_index as isize);
                let value_bits =
                    (value_bits & !bit_mask) | if bool::from(bit) { bit_mask } else { 0 };
                *self.payload.offset(word_index as isize) = value_bits;
            }
        } else {
            let bit_mask = 1usize << index;
            let bits = self.payload as usize;
            let bits = (bits & !bit_mask) | if bool::from(bit) { bit_mask } else { 0 };
            self.payload = bits as *mut usize;
        }
    }

    fn get_bit_two_state_internal(&self, index: usize) -> Bit {
        if self.is_pointer() {
            let word_index = index / usize::BITS as usize;
            let bit_index = index % usize::BITS as usize;
            let value_bits = unsafe { *self.payload.offset(word_index as isize) };
            Bit::from((value_bits >> bit_index) & 1)
        } else {
            Bit::from((self.payload as usize >> index) & 1)
        }
    }

    pub fn set_bit(&mut self, index: usize, bit: Logic) {
        if index >= self.get_bit_width() {
        } else if self.is_four_state() {
            self.set_bit_four_state_internal(index, bit);
        } else {
            self.set_bit_two_state_internal(index, Bit::from(bit));
        }
    }

    pub fn get_bit(&self, index: usize) -> Logic {
        if index >= self.get_bit_width() {
            Logic::Zero
        } else if self.is_four_state() {
            self.get_bit_four_state_internal(index)
        } else {
            Logic::from(self.get_bit_two_state_internal(index))
        }
    }

    // Various status functions

    pub fn is_pointer(&self) -> bool {
        self.size & POINTER_TAG != 0
    }

    pub fn get_memory_words_size(&self) -> usize {
        if self.is_pointer() {
            if self.is_four_state() {
                (((self.get_bit_width() - 1) / (usize::BITS as usize)) + 1) * 2
            } else {
                ((self.get_bit_width() - 1) / (usize::BITS as usize)) + 1
            }
        } else {
            0
        }
    }

    pub fn get_vector_words_size(&self) -> usize {
        if self.is_pointer() {
            ((self.get_bit_width() - 1) / (usize::BITS as usize)) + 1
        } else {
            1
        }
    }

    pub fn is_four_state(&self) -> bool {
        self.size & FOUR_STATE_TAG != 0
    }

    pub fn get_bit_width(&self) -> usize {
        self.size & !(POINTER_TAG | FOUR_STATE_TAG)
    }

    pub fn is_unknown(&self) -> bool {
        self.iter().any(|b| b == Logic::Unknown)
    }

    pub fn is_high_impedance(&self) -> bool {
        !self.is_unknown() && self.iter().any(|b| b == Logic::HighImpedance)
    }
}

impl<T> From<T> for BitVector
where
    T: UnsignedInteger,
{
    fn from(value: T) -> Self {
        Self::from_bits_two_state(value.get_width() as usize, value)
    }
}

impl From<Bit> for BitVector {
    fn from(value: Bit) -> Self {
        match value {
            Bit::Zero => Self::new_zero_bit(),
            Bit::One => Self::new_one_bit(),
        }
    }
}

impl From<Logic> for BitVector {
    fn from(value: Logic) -> Self {
        match value {
            Logic::Zero => Self::new_zero_bit(),
            Logic::One => Self::new_one_bit(),
            Logic::Unknown => Self::new_unknown_bit(),
            Logic::HighImpedance => Self::new_high_impedance_bit(),
        }
    }
}

impl Clone for BitVector {
    fn clone(&self) -> Self {
        if self.is_pointer() {
            // Allocate memory for new bitvector
            let len = self.get_memory_words_size();
            let layout = alloc::Layout::array::<usize>(len).unwrap();
            assert!(
                layout.size() <= isize::MAX as usize,
                "Allocation too large: {} bytes",
                layout.size()
            );
            let ptr = unsafe { alloc::alloc(layout) as *mut usize };
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }
            unsafe {
                std::ptr::copy_nonoverlapping(self.payload, ptr, len);
            }
            Self {
                size: self.size,
                payload: ptr,
            }
        } else {
            Self {
                size: self.size,
                payload: self.payload,
            }
        }
    }
}

impl PartialEq for BitVector {
    fn eq(&self, other: &Self) -> bool {
        let (myself, other) = if self.get_bit_width() > other.get_bit_width() {
            (self, other)
        } else {
            (other, self)
        };
        for i in 0..other.get_bit_width() {
            if myself.get_bit(i) != other.get_bit(i) {
                return false;
            }
        }
        for i in other.get_bit_width()..myself.get_bit_width() {
            if myself.get_bit(i) != Logic::Zero {
                return false;
            }
        }
        true
    }
}

impl Drop for BitVector {
    fn drop(&mut self) {
        if self.is_pointer() {
            let layout = alloc::Layout::array::<usize>(self.get_memory_words_size()).unwrap();
            unsafe {
                alloc::dealloc(self.payload as *mut u8, layout);
            }
        }
    }
}
