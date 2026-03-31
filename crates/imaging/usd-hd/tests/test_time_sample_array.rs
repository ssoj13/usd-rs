// Port of pxr/imaging/hd/testenv/testHdTimeSampleArray.cpp

use usd_hd::time_sample_array::{hd_resample_neighbors, hd_resample_raw_time_samples};

#[test]
fn test_resample_neighbors() {
    // Exact values at endpoints
    assert_eq!(hd_resample_neighbors(0.0, &0.0_f32, &256.0_f32), 0.0);
    assert_eq!(hd_resample_neighbors(1.0, &0.0_f32, &256.0_f32), 256.0);

    // Interpolation - approximate intervals
    let v025 = hd_resample_neighbors(0.25, &0.0_f32, &256.0_f32);
    assert!(v025 > 63.0 && v025 < 65.0, "0.25 interpolation: {}", v025);

    let v050 = hd_resample_neighbors(0.50, &0.0_f32, &256.0_f32);
    assert!(v050 > 127.0 && v050 < 129.0, "0.50 interpolation: {}", v050);

    let v075 = hd_resample_neighbors(0.75, &0.0_f32, &256.0_f32);
    assert!(v075 > 191.0 && v075 < 193.0, "0.75 interpolation: {}", v075);

    // Extrapolation
    let vm1 = hd_resample_neighbors(-1.0, &0.0_f32, &256.0_f32);
    assert!(vm1 > -257.0 && vm1 < -255.0, "-1.0 extrapolation: {}", vm1);

    let vp2 = hd_resample_neighbors(2.0, &0.0_f32, &256.0_f32);
    assert!(vp2 > 511.0 && vp2 < 513.0, "+2.0 extrapolation: {}", vp2);
}

#[test]
fn test_resample_raw_time_samples() {
    let times = [0.0_f32, 1.0];
    let values = [0.0_f32, 256.0];

    // Exact values at endpoints
    assert_eq!(hd_resample_raw_time_samples(0.0, 2, &times, &values), 0.0);
    assert_eq!(hd_resample_raw_time_samples(1.0, 2, &times, &values), 256.0);

    // Interpolation
    let v025 = hd_resample_raw_time_samples(0.25, 2, &times, &values);
    assert!(v025 > 63.0 && v025 < 65.0, "0.25 interpolation: {}", v025);

    let v050 = hd_resample_raw_time_samples(0.50, 2, &times, &values);
    assert!(v050 > 127.0 && v050 < 129.0, "0.50 interpolation: {}", v050);

    let v075 = hd_resample_raw_time_samples(0.75, 2, &times, &values);
    assert!(v075 > 191.0 && v075 < 193.0, "0.75 interpolation: {}", v075);

    // Extrapolation - constant outside sample range
    assert_eq!(hd_resample_raw_time_samples(-1.0, 2, &times, &values), 0.0);
    assert_eq!(hd_resample_raw_time_samples(2.0, 2, &times, &values), 256.0);
}
