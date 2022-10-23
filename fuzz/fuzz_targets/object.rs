#![no_main]
use libfuzzer_sys::fuzz_target;
use han::Object;

fuzz_target!(|obj: &str| {
    let _ = obj.parse::<Object>();
});
