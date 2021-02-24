#[macro_use]
extern crate vst;

mod ladder_filter;

plugin_main!(ladder_filter::LadderFilter);

