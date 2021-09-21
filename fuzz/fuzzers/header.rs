#![no_main]
use libfuzzer_sys::fuzz_target;

use flavors::parser::header;

fuzz_target!(|data: &[u8]| {
    let _header_result = header(data);
});
