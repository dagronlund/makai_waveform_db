use std::fmt;

use crate::bitvector::{BitVector, BitVectorRadix};

fn div_ceil(lhs: usize, rhs: usize) -> usize {
    if lhs % rhs == 0 {
        lhs / rhs
    } else {
        (lhs / rhs) + 1
    }
}

impl BitVector {
    fn fmt_radix(&self, f: &mut fmt::Formatter, radix: BitVectorRadix) -> fmt::Result {
        let table = [
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F",
        ];
        write!(f, "{}", radix.to_str())?;
        if self.is_four_state() {
            match radix {
                BitVectorRadix::Binary => {
                    for i in (0..self.get_bit_width()).rev() {
                        write!(f, "{}", self.get_bit(i).to_str())?;
                    }
                    Ok(())
                }
                BitVectorRadix::Octal => {
                    for i in (0..div_ceil(self.get_bit_width(), 3)).map(|i| i * 3).rev() {
                        let (value0, mask0) = self.get_bit(i).to_bool_pair();
                        let (value1, mask1) = self.get_bit(i + 1).to_bool_pair();
                        let (value2, mask2) = self.get_bit(i + 2).to_bool_pair();
                        let values = [value0, value1, value2];
                        let masks = [mask0, mask1, mask2];
                        // Check if any of the values are X or Z
                        if masks.iter().any(|m| *m) {
                            // Print Z if there are no X values
                            if values.iter().zip(masks.iter()).all(|(v, m)| *v || !*m) {
                                write!(f, "Z")?;
                            } else {
                                write!(f, "X")?;
                            }
                        } else {
                            let digit = values
                                .iter()
                                .enumerate()
                                .map(|(i, v)| if *v { 1 << i } else { 0 })
                                .sum::<usize>();
                            write!(f, "{}", table[digit])?;
                        }
                    }
                    Ok(())
                }
                BitVectorRadix::Hexadecimal => {
                    for i in (0..div_ceil(self.get_bit_width(), 4)).map(|i| i * 4).rev() {
                        let (value0, mask0) = self.get_bit(i).to_bool_pair();
                        let (value1, mask1) = self.get_bit(i + 1).to_bool_pair();
                        let (value2, mask2) = self.get_bit(i + 2).to_bool_pair();
                        let (value3, mask3) = self.get_bit(i + 3).to_bool_pair();
                        let values = [value0, value1, value2, value3];
                        let masks = [mask0, mask1, mask2, mask3];
                        // Check if any of the values are X or Z
                        if masks.iter().any(|m| *m) {
                            // Print Z if there are no X values
                            if values.iter().zip(masks.iter()).all(|(v, m)| *v || !*m) {
                                write!(f, "Z")?;
                            } else {
                                write!(f, "X")?;
                            }
                        } else {
                            let digit = values
                                .iter()
                                .enumerate()
                                .map(|(i, v)| if *v { 1 << i } else { 0 })
                                .sum::<usize>();
                            write!(f, "{}", table[digit])?;
                        }
                    }
                    Ok(())
                }
                BitVectorRadix::Decimal => {
                    if self.is_pointer() {
                        write!(f, "OVERFLOW!")
                    } else {
                        let (value, mask) = self.to_bits_four_state::<u64>();
                        if mask != 0 {
                            if (!value & mask) == 0 {
                                write!(f, "Z")
                            } else {
                                write!(f, "X")
                            }
                        } else {
                            write!(f, "{}", value)
                        }
                    }
                }
            }
        } else {
            match radix {
                BitVectorRadix::Binary => {
                    for i in (0..self.get_bit_width()).rev() {
                        write!(f, "{}", if self.get_bit(i).into() { "1" } else { "0" })?;
                    }
                    Ok(())
                }
                BitVectorRadix::Octal => {
                    for i in (0..div_ceil(self.get_bit_width(), 3)).map(|i| i * 3).rev() {
                        let digit = (if self.get_bit(i).into() { 1 } else { 0 })
                            + (if self.get_bit(i + 1).into() { 2 } else { 0 })
                            + (if self.get_bit(i + 2).into() { 4 } else { 0 });
                        write!(f, "{}", table[digit])?;
                    }
                    Ok(())
                }
                BitVectorRadix::Hexadecimal => {
                    for i in (0..div_ceil(self.get_bit_width(), 4)).map(|i| i * 4).rev() {
                        let digit = (if self.get_bit(i).into() { 1 } else { 0 })
                            + (if self.get_bit(i + 1).into() { 2 } else { 0 })
                            + (if self.get_bit(i + 2).into() { 4 } else { 0 })
                            + (if self.get_bit(i + 3).into() { 8 } else { 0 });
                        write!(f, "{}", table[digit])?;
                    }
                    Ok(())
                }
                BitVectorRadix::Decimal => {
                    if self.is_pointer() {
                        write!(f, "OVERFLOW!")
                    } else {
                        write!(f, "{}", self.to_bits_two_state::<u64>())
                    }
                }
            }
        }
    }

    pub fn to_string_radix(&self, radix: BitVectorRadix) -> String {
        // Slightly confusing way to construct a formatter?
        struct Fmt<F>(pub F)
        where
            F: Fn(&mut fmt::Formatter) -> fmt::Result;

        impl<F> fmt::Display for Fmt<F>
        where
            F: Fn(&mut fmt::Formatter) -> fmt::Result,
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0)(f)
            }
        }

        format!("{}", Fmt(|f| self.fmt_radix(f, radix.clone())))
    }
}

impl fmt::Display for BitVector {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.fmt_radix(fmt, BitVectorRadix::Binary)
    }
}

impl fmt::Debug for BitVector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BitVector")
            .field("width", &self.get_bit_width())
            .field("value", &self.to_string())
            .finish()
    }
}
