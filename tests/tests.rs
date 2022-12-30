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
