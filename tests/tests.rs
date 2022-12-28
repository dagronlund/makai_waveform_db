#[test]
fn test_waveform_search() {
    use waveform_db::*;

    let mut waveform = Waveform::new();
    waveform.insert_timestamp(5).unwrap();
    waveform.insert_timestamp(10).unwrap();
    waveform.insert_timestamp(15).unwrap();
    waveform.insert_timestamp(25).unwrap();

    assert_eq!(waveform.search_timestamp(0), None);
    assert_eq!(waveform.search_timestamp(4), None);
    assert_eq!(waveform.search_timestamp(5), Some(0));
    assert_eq!(waveform.search_timestamp(7), Some(0));
    assert_eq!(waveform.search_timestamp(9), Some(0));
    assert_eq!(waveform.search_timestamp(10), Some(1));
    assert_eq!(waveform.search_timestamp(25), Some(3));
    assert_eq!(waveform.search_timestamp(30), Some(3));

    assert_eq!(waveform.search_timestamp_after(0), Some(0));
    assert_eq!(waveform.search_timestamp_after(2), Some(0));
    assert_eq!(waveform.search_timestamp_after(4), Some(0));
    assert_eq!(waveform.search_timestamp_after(5), Some(0));
    assert_eq!(waveform.search_timestamp_after(6), Some(1));
    assert_eq!(waveform.search_timestamp_after(7), Some(1));
    assert_eq!(waveform.search_timestamp_after(9), Some(1));
    assert_eq!(waveform.search_timestamp_after(10), Some(1));
    assert_eq!(waveform.search_timestamp_after(25), Some(3));
    assert_eq!(waveform.search_timestamp_after(26), None);
    assert_eq!(waveform.search_timestamp_after(30), None);

    assert_eq!(waveform.search_timestamp_range(0..10, false), Some(0..0));
    assert_eq!(waveform.search_timestamp_range(0..10, true), Some(0..1));

    assert_eq!(waveform.search_timestamp_range(0..12, false), Some(0..1));
    assert_eq!(waveform.search_timestamp_range(0..12, true), Some(0..2));

    assert_eq!(waveform.search_timestamp_range(20..30, false), Some(3..3));
    assert_eq!(waveform.search_timestamp_range(25..30, false), Some(3..3));
    assert_eq!(waveform.search_timestamp_range(26..30, false), None);
}
