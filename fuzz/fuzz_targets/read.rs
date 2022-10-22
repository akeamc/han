#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for readout in han::Reader::new(data.iter().cloned()) {
        let _ = readout.to_telegram();
    }
});
