use crate::bitvector::BitVector;
use crate::history::WaveformHistory;

#[derive(Clone, Debug, PartialEq)]
enum WaveformVectorPacking {
    Bits(usize),  // How many bits required for value + mask
    Bytes(usize), // How many bytes required for value + mask
}

impl WaveformVectorPacking {
    fn new(width: usize) -> Self {
        match width {
            // Store multiple values and masks in the same byte
            1 => Self::Bits(2),
            2 => Self::Bits(4),
            3..=4 => Self::Bits(8),
            // Store the value and mask in two byte-aligned chunks
            _ => Self::Bytes(((width - 1) / 8 + 1) * 2),
        }
    }
}

pub struct WaveformSignalVector {
    // How many bits wide is the signal
    width: usize,
    // How many bytes are used to store the four-state vector
    packing: WaveformVectorPacking,
    history: WaveformHistory,
    vectors: Vec<u8>,
    vector_index: usize,
    bits_unused: usize,
}

impl WaveformSignalVector {
    pub fn new(width: usize) -> Self {
        Self {
            width: width,
            packing: WaveformVectorPacking::new(width),
            history: WaveformHistory::new(),
            vectors: Vec::new(),
            vector_index: 0,
            bits_unused: 0,
        }
    }

    pub fn get_history(&self) -> &WaveformHistory {
        &self.history
    }

    pub fn update(&mut self, timestamp_index: usize, bv: BitVector) {
        self.history.add_change(timestamp_index, self.vector_index);
        let offset = self.vectors.len();
        match self.packing {
            WaveformVectorPacking::Bits(bits) => {
                let combined_mask = (1 << (bits / 2)) - 1;
                let (value, mask) = bv.to_bits_four_state::<u8>();
                let combined =
                    ((value & combined_mask) | ((mask & combined_mask) << (bits / 2))) as u8;
                match self.bits_unused {
                    2 => self.vectors[offset - 1] |= combined << 6,
                    4 => self.vectors[offset - 1] |= combined << 4,
                    6 => self.vectors[offset - 1] |= combined << 2,
                    _ => {
                        self.vectors.push(combined);
                        self.bits_unused = 8;
                    }
                }
                self.bits_unused -= bits;
            }
            WaveformVectorPacking::Bytes(bytes) => {
                let byte_width = ((bv.get_bit_width() - 1) / 8) + 1;
                self.vectors.resize(offset + bytes, 0);
                let (value_vector, mask_vector) =
                    (&mut self.vectors[offset..offset + bytes]).split_at_mut(bytes / 2);
                // Compensate for incoming vectors that are shorter than the allocated space
                bv.to_be_bytes_four_state(
                    &mut value_vector[(bytes / 2 - byte_width)..(bytes / 2)],
                    &mut mask_vector[(bytes / 2 - byte_width)..(bytes / 2)],
                );
            }
        }
        self.vector_index += 1;
    }

    pub fn get_bitvector(&self, index: usize) -> BitVector {
        match self.packing {
            WaveformVectorPacking::Bits(bits) => {
                let bit_mask = (1 << (bits / 2)) - 1;
                let vectors_per_byte = 8 / bits;
                let byte_index = index / vectors_per_byte;
                let bit_index = (index % vectors_per_byte) * bits;
                let byte = self.vectors[byte_index];
                let value = (byte >> bit_index) & bit_mask;
                let mask = (byte >> (bit_index + bits / 2)) & bit_mask;
                BitVector::from_bits_four_state(self.get_width(), value, mask)
            }
            WaveformVectorPacking::Bytes(bytes) => {
                let offset = bytes * index;
                BitVector::from_be_bytes_four_state(
                    self.get_width(),
                    &self.vectors[offset..offset + bytes / 2],
                    &self.vectors[offset + bytes / 2..offset + bytes],
                )
            }
        }
    }

    pub fn get_vector_size(&self) -> usize {
        self.vectors.len()
    }

    pub fn get_width(&self) -> usize {
        self.width
    }

    pub fn len(&self) -> usize {
        self.vector_index
    }
}
