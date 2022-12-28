use crate::bitvector::{BitVector, Logic};

pub struct BitVectorIter<'a> {
    bv: &'a BitVector,
    bits: usize,
    index: usize,
    is_four_state: bool,
}

impl<'a> Iterator for BitVectorIter<'a> {
    type Item = Logic;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.bits {
            let result = if self.is_four_state {
                self.bv.get_bit_four_state_internal(self.index)
            } else {
                Logic::from(self.bv.get_bit_two_state_internal(self.index))
            };
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl BitVector {
    pub fn iter(&self) -> BitVectorIter {
        BitVectorIter {
            bv: &self,
            bits: self.get_bit_width(),
            index: 0,
            is_four_state: self.is_four_state(),
        }
    }
}

impl<'a> IntoIterator for &'a BitVector {
    type Item = Logic;
    type IntoIter = BitVectorIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
