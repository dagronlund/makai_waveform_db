use waveform_db::history::{index::WaveformHistoryIndex, WaveformHistory};

fn generate_history(num: usize, start: usize) -> (WaveformHistory, Vec<WaveformHistoryIndex>) {
    let mut history = WaveformHistory::new();
    let mut timestamp_index = start;
    let mut value_index = 0;
    let mut expected = Vec::new();
    for _ in 0..num {
        expected.push(WaveformHistoryIndex {
            timestamp_index,
            value_index,
        });
        history.add_change(timestamp_index, value_index);
        let mut delta = rand::random::<u8>();
        while delta == 0 {
            delta = rand::random::<u8>();
        }
        timestamp_index += delta as usize;
        value_index += 1;
    }
    (history, expected)
}

#[test]
fn test_waveform_history_block() {
    use waveform_db::history::{
        block::WaveformHistoryBlock, index::WaveformHistoryIndex, BLOCK_SIZE,
    };

    let block_raw = vec![0; BLOCK_SIZE];
    let block = WaveformHistoryBlock::new(&block_raw);
    assert_eq!(
        (&block).into_iter().collect::<Vec<WaveformHistoryIndex>>(),
        vec![]
    );

    let mut block_raw = vec![0; BLOCK_SIZE];
    block_raw[16 + 0] = 0x80; // One Change
    block_raw[16 + 1] = 0x1; // One skip
    block_raw[16 + 2] = 0x81; // Two changes
    let block = WaveformHistoryBlock::new(&block_raw);
    assert_eq!(
        (&block).into_iter().collect::<Vec<WaveformHistoryIndex>>(),
        vec![
            WaveformHistoryIndex {
                timestamp_index: 0,
                value_index: 0
            },
            WaveformHistoryIndex {
                timestamp_index: 2,
                value_index: 1
            },
            WaveformHistoryIndex {
                timestamp_index: 3,
                value_index: 2
            }
        ]
    );

    let mut block_raw = vec![0; BLOCK_SIZE];
    block_raw[16 + 0] = 0x80; // One Change
    block_raw[16 + 1] = 0x1; // One skip
    block_raw[16 + 2] = 0x81; // Two changes
    block_raw[16 + 3] = 0x2; // Two skips
    block_raw[16 + 4] = 0x80; // One change
    let block = WaveformHistoryBlock::new(&block_raw);
    assert_eq!(
        (&block).into_iter().collect::<Vec<WaveformHistoryIndex>>(),
        vec![
            WaveformHistoryIndex {
                timestamp_index: 0,
                value_index: 0
            },
            WaveformHistoryIndex {
                timestamp_index: 2,
                value_index: 1
            },
            WaveformHistoryIndex {
                timestamp_index: 3,
                value_index: 2
            },
            WaveformHistoryIndex {
                timestamp_index: 6,
                value_index: 3
            },
        ]
    );

    let mut block_raw = vec![0; BLOCK_SIZE];
    block_raw[16 + 0] = 0x7F; // 127 * 128 + 127 skips
    block_raw[16 + 1] = 0x7F;
    block_raw[16 + 2] = 0xff; // 128 changes
    block_raw[16 + 3] = 0xff; // 128 changes
    let block = WaveformHistoryBlock::new(&block_raw);
    let mut expected = Vec::new();
    for i in 0..256 {
        expected.push(WaveformHistoryIndex {
            timestamp_index: 127 * 128 + 127 + i,
            value_index: i,
        });
    }
    assert_eq!(
        (&block).into_iter().collect::<Vec<WaveformHistoryIndex>>(),
        expected
    );
}

#[test]
fn test_waveform_history() {
    let (history, expected) = generate_history(1 << 16, 0);
    for (i, index) in history.into_iter().enumerate() {
        assert_eq!(expected[i], index);
    }
}

#[test]
fn test_waveform_history_seek() {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let (history, expected) = generate_history(1 << 16, 0);
    // Check for index expected to be found
    let i = rng.gen_range(0..expected.len());
    let mut iter = history.into_iter();
    assert_eq!(
        iter.seek(expected[i].get_timestamp_index()),
        Some(expected[i].clone()),
    );
    assert_eq!(iter.next(), expected.get(i + 1).map(|i| i.clone()));

    // The chose value has one after and a gap in the timestamps
    if i < expected.len() - 1
        && expected[i].get_timestamp_index() + 1 < expected[i + 1].get_timestamp_index()
    {
        let mut iter = history.into_iter();
        let seeked = iter.seek(expected[i].get_timestamp_index() + 1);
        assert_eq!(seeked, Some(expected[i].clone()),);
        assert_eq!(iter.next(), expected.get(i + 1).map(|i| i.clone()));
    }

    // Check for last index by looking for one after
    let mut iter = history.into_iter();
    assert_eq!(
        iter.seek(expected.last().unwrap().get_timestamp_index() + 1),
        Some(expected.last().unwrap().clone()),
    );

    // Check for index not expected to exist
    let (history, _) = generate_history(1 << 16, 1);
    assert_eq!(history.into_iter().seek(0), None);
}

#[test]
fn test_waveform_history_search() {
    use rand::Rng;
    use waveform_db::WaveformSearchMode;
    let mut rng = rand::thread_rng();

    let (history, expected) = generate_history(1 << 16, 0);
    let timestamp_index_range = expected.first().unwrap().get_timestamp_index()
        ..expected.last().unwrap().get_timestamp_index();

    let search = rng.gen_range(timestamp_index_range.clone());

    // "Manually" determine what all the search results should be
    let mut before: Option<WaveformHistoryIndex> = None;
    let mut after: Option<WaveformHistoryIndex> = None;
    let mut closest: Option<WaveformHistoryIndex> = None;
    let mut exact: Option<WaveformHistoryIndex> = None;
    for index in &expected {
        if index.get_timestamp_index() <= search {
            before = match before {
                Some(before) if index.get_timestamp_index() > before.get_timestamp_index() => {
                    Some(index.clone())
                }
                Some(before) => Some(before),
                None => Some(index.clone()),
            };
        }

        if index.get_timestamp_index() >= search {
            after = match after {
                Some(after) if index.get_timestamp_index() < after.get_timestamp_index() => {
                    Some(index.clone())
                }
                Some(after) => Some(after),
                None => Some(index.clone()),
            };
        }

        closest = match closest {
            Some(closest) => {
                let diff_closest = (search as isize - closest.get_timestamp_index() as isize).abs();
                let diff_index = (search as isize - index.get_timestamp_index() as isize).abs();
                if diff_index < diff_closest {
                    Some(index.clone())
                } else {
                    Some(closest)
                }
            }
            None => Some(index.clone()),
        };

        if index.get_timestamp_index() == search {
            exact = Some(index.clone());
        }
    }

    let before_search = history.search_timestamp_index(search, WaveformSearchMode::Before);
    let after_search = history.search_timestamp_index(search, WaveformSearchMode::After);
    let closest_search = history.search_timestamp_index(search, WaveformSearchMode::Closest);
    let exact_search = history.search_timestamp_index(search, WaveformSearchMode::Exact);

    assert_eq!(before, before_search);
    assert_eq!(after, after_search);
    assert_eq!(closest, closest_search,);
    assert_eq!(exact, exact_search);

    println!("Searching for {search}...");
    println!("Before:  {before_search:?}");
    println!("After:   {after_search:?}");
    println!("Closest: {closest_search:?}");
    println!("Exact:   {exact_search:?}");
}

#[test]
fn test_waveform_search_timestamp() {
    use waveform_db::{Waveform, WaveformSearchMode as Mode};

    let mut waveform = Waveform::new();
    waveform.insert_timestamp(5).unwrap();
    waveform.insert_timestamp(10).unwrap();
    waveform.insert_timestamp(15).unwrap();
    waveform.insert_timestamp(25).unwrap();

    assert_eq!(waveform.search_timestamp(0, Mode::Before), None);
    assert_eq!(waveform.search_timestamp(4, Mode::Before), None);
    assert_eq!(waveform.search_timestamp(5, Mode::Before), Some(0));
    assert_eq!(waveform.search_timestamp(7, Mode::Before), Some(0));
    assert_eq!(waveform.search_timestamp(9, Mode::Before), Some(0));
    assert_eq!(waveform.search_timestamp(10, Mode::Before), Some(1));
    assert_eq!(waveform.search_timestamp(25, Mode::Before), Some(3));
    assert_eq!(waveform.search_timestamp(30, Mode::Before), Some(3));

    assert_eq!(waveform.search_timestamp(0, Mode::After), Some(0));
    assert_eq!(waveform.search_timestamp(2, Mode::After), Some(0));
    assert_eq!(waveform.search_timestamp(4, Mode::After), Some(0));
    assert_eq!(waveform.search_timestamp(5, Mode::After), Some(0));
    assert_eq!(waveform.search_timestamp(6, Mode::After), Some(1));
    assert_eq!(waveform.search_timestamp(7, Mode::After), Some(1));
    assert_eq!(waveform.search_timestamp(9, Mode::After), Some(1));
    assert_eq!(waveform.search_timestamp(10, Mode::After), Some(1));
    assert_eq!(waveform.search_timestamp(25, Mode::After), Some(3));
    assert_eq!(waveform.search_timestamp(26, Mode::After), None);
    assert_eq!(waveform.search_timestamp(30, Mode::After), None);

    assert_eq!(waveform.search_timestamp(0, Mode::Exact), None);
    assert_eq!(waveform.search_timestamp(4, Mode::Exact), None);
    assert_eq!(waveform.search_timestamp(5, Mode::Exact), Some(0));
    assert_eq!(waveform.search_timestamp(6, Mode::Exact), None);
    assert_eq!(waveform.search_timestamp(24, Mode::Exact), None);
    assert_eq!(waveform.search_timestamp(25, Mode::Exact), Some(3));
    assert_eq!(waveform.search_timestamp(26, Mode::Exact), None);

    assert_eq!(waveform.search_timestamp(0, Mode::Closest), Some(0));
    assert_eq!(waveform.search_timestamp(4, Mode::Closest), Some(0));
    assert_eq!(waveform.search_timestamp(5, Mode::Closest), Some(0));
    assert_eq!(waveform.search_timestamp(6, Mode::Closest), Some(0));
    assert_eq!(waveform.search_timestamp(7, Mode::Closest), Some(0));
    assert_eq!(waveform.search_timestamp(8, Mode::Closest), Some(1));
    assert_eq!(waveform.search_timestamp(9, Mode::Closest), Some(1));
    assert_eq!(waveform.search_timestamp(10, Mode::Closest), Some(1));
    assert_eq!(waveform.search_timestamp(24, Mode::Closest), Some(3));
    assert_eq!(waveform.search_timestamp(25, Mode::Closest), Some(3));
    assert_eq!(waveform.search_timestamp(26, Mode::Closest), Some(3));
}
