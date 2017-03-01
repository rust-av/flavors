#![no_main]
extern crate libfuzzer_sys;
extern crate flavors;

use flavors::parser::complete_tag;

#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let tag_result = complete_tag(data);
}
