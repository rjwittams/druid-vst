#[macro_use]
extern crate vst;

use ladder_filter::LadderProcessor;
use vst::plugin::{Plugin, Info, Category, HostCallback, PluginParameters};
use std::sync::Arc;
use carnyx_vst::{VstCarnyxHost, VstParams, VstCarnyxEditor};
use carnyx::buffer::AudioBuffer;
use carnyx::carnyx::CarnyxProcessor;
use vst::editor::Editor;

impl Default for LadderFilterVST {
    fn default() -> LadderFilterVST {
        unimplemented!()
    }
}

pub struct LadderFilterVST {
    processor: LadderProcessor,
}

impl Plugin for LadderFilterVST {
    fn get_info(&self) -> Info {
        Info {
            name: "LadderFilter".to_string(),
            unique_id: 9263,
            inputs: 1,
            outputs: 1,
            category: Category::Effect,
            parameters: 4,
            ..Default::default()
        }
    }

    fn new(host: HostCallback) -> Self
        where
            Self: Sized + Default,
    {
        LadderFilterVST {
            processor: LadderProcessor::new(Arc::new(VstCarnyxHost::new(host)))
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.processor.set_sample_rate(rate)
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        self.processor.process(buffer)
    }

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::new(VstParams::new(
            self.processor.parameters(),
            self.processor.model(),
            self.processor.listener())
        ) as Arc<dyn PluginParameters>
    }

    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        let ce = self.processor.editor();
        Some(Box::new(VstCarnyxEditor::new(ce)) as Box<dyn Editor>)
    }
}

plugin_main!(LadderFilterVST);