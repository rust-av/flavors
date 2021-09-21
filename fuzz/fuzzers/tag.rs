#![no_main]
use libfuzzer_sys::fuzz_target;

use flavors::parser::complete_tag;

fuzz_target!(|data: &[u8]| {
    let _tag_result = complete_tag(data);
});
