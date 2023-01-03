use std::cmp::Ordering;

use crate::history::index::WaveformHistoryIndex;
use crate::history::BLOCK_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveformHistoryBlock<'a> {
    block: &'a [u8],
}

impl<'a> WaveformHistoryBlock<'a> {
    pub fn new(block: &'a [u8]) -> Self {
        debug_assert_eq!(block.len(), BLOCK_SIZE);
        Self { block }
    }

    pub fn get_timestamp_index(&self) -> usize {
        u64::from_be_bytes((&self.block[0..8]).try_into().unwrap()) as usize
    }

    pub fn get_value_index(&self) -> usize {
        u64::from_be_bytes((&self.block[8..16]).try_into().unwrap()) as usize
    }

    pub fn get_index(&self) -> WaveformHistoryIndex {
        WaveformHistoryIndex {
            timestamp_index: self.get_timestamp_index(),
            value_index: self.get_value_index(),
        }
    }

    pub(crate) fn get_skips(&self, offset: usize) -> (usize, usize) {
        let (mut skip_bytes, mut skips, mut skips_partial) = (0usize, 0usize, 0usize);
        debug_assert!(offset >= 16);
        for i in offset..BLOCK_SIZE {
            if self.block[i] & 0x80 == 0x80 {
                break;
            }
            skips_partial <<= 7;
            skips_partial |= self.block[i] as usize;
            skip_bytes += 1;
            // Add partial sum to total sum after 8 bytes
            if skip_bytes & 0b111 == 0 {
                skips += skips_partial;
                skips_partial = 0;
            }
        }
        skips += skips_partial;
        debug_assert!(skips > 0 || offset + skip_bytes == BLOCK_SIZE || skip_bytes == 0);
        (skip_bytes, skips)
    }
}

fn next_index<'a>(
    block: &'a WaveformHistoryBlock<'a>,
    index: &mut WaveformHistoryIndex,
    offset: &mut usize,
    consumed_changes: &mut u8,
) -> Option<WaveformHistoryIndex> {
    loop {
        // Consume any skip bytes that are waiting
        let (skip_bytes, skips) = block.get_skips(*offset);
        *offset += skip_bytes;
        index.timestamp_index += skips;
        if *offset >= BLOCK_SIZE {
            return None;
        }
        debug_assert!(block.block[*offset] & 0x80 == 0x80);
        // Consume any available changes
        let total_changes = (block.block[*offset] & 0x7F) + 1;
        if *consumed_changes < total_changes {
            let current_index = index.clone();
            index.value_index += 1;
            index.timestamp_index += 1;
            *consumed_changes += 1;
            return Some(current_index);
        }
        // Clear the consumed changes and start over at next byte
        *consumed_changes = 0;
        *offset += 1;
    }
}

fn seek_index<'a>(
    block: &'a WaveformHistoryBlock<'a>,
    index: &mut WaveformHistoryIndex,
    offset: &mut usize,
    consumed_changes: &mut u8,
    timestamp_index: usize,
) -> Option<WaveformHistoryIndex> {
    let mut last_index = None;
    loop {
        let saved_index = index.clone();
        let saved_offset = *offset;
        let saved_consumed_changes = *consumed_changes;
        // Get next
        let Some(next_index) = next_index(block, index, offset, consumed_changes) else {
            *index = saved_index;
            *offset = saved_offset;
            *consumed_changes = saved_consumed_changes;
            return last_index;
        };
        match next_index.get_timestamp_index().cmp(&timestamp_index) {
            Ordering::Greater => {
                *index = saved_index;
                *offset = saved_offset;
                *consumed_changes = saved_consumed_changes;
                return last_index;
            }
            Ordering::Equal => {
                return Some(next_index);
            }
            Ordering::Less => {
                last_index = Some(next_index);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveformHistoryBlockRefIter<'a> {
    block: &'a WaveformHistoryBlock<'a>,
    index: WaveformHistoryIndex,
    offset: usize,
    consumed_changes: u8,
}

impl<'a> IntoIterator for &'a WaveformHistoryBlock<'a> {
    type Item = WaveformHistoryIndex;

    type IntoIter = WaveformHistoryBlockRefIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        WaveformHistoryBlockRefIter {
            block: self,
            index: self.get_index(),
            offset: 16,
            consumed_changes: 0,
        }
    }
}

impl<'a> Iterator for WaveformHistoryBlockRefIter<'a> {
    type Item = WaveformHistoryIndex;

    fn next(&mut self) -> Option<Self::Item> {
        next_index(
            self.block,
            &mut self.index,
            &mut self.offset,
            &mut self.consumed_changes,
        )
    }
}

impl<'a> WaveformHistoryBlockRefIter<'a> {
    /// Returns the history index (timestamp/value) either at or right before
    /// the requested timestamp index, returning None if nothing exists before
    pub fn seek(&mut self, timestamp_index: usize) -> Option<WaveformHistoryIndex> {
        seek_index(
            self.block,
            &mut self.index,
            &mut self.offset,
            &mut self.consumed_changes,
            timestamp_index,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveformHistoryBlockIter<'a> {
    block: WaveformHistoryBlock<'a>,
    index: WaveformHistoryIndex,
    offset: usize,
    consumed_changes: u8,
}

impl<'a> IntoIterator for WaveformHistoryBlock<'a> {
    type Item = WaveformHistoryIndex;

    type IntoIter = WaveformHistoryBlockIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let index = self.get_index();
        WaveformHistoryBlockIter {
            block: self,
            index,
            offset: 16,
            consumed_changes: 0,
        }
    }
}

impl<'a> Iterator for WaveformHistoryBlockIter<'a> {
    type Item = WaveformHistoryIndex;

    fn next(&mut self) -> Option<Self::Item> {
        next_index(
            &self.block,
            &mut self.index,
            &mut self.offset,
            &mut self.consumed_changes,
        )
    }
}

impl<'a> WaveformHistoryBlockIter<'a> {
    /// Returns the history index (timestamp/value) either at or right before
    /// the requested timestamp index, returning None if nothing exists before
    pub fn seek(&mut self, timestamp_index: usize) -> Option<WaveformHistoryIndex> {
        seek_index(
            &self.block,
            &mut self.index,
            &mut self.offset,
            &mut self.consumed_changes,
            timestamp_index,
        )
    }
}
