#![no_main]
extern crate libfuzzer_sys;
extern crate flavors;

use flavors::header;

#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let header_result = header(data);
}
