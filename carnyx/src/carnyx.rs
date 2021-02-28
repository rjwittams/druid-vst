use raw_window_handle::RawWindowHandle;
use crate::buffer::AudioBuffer;
use std::sync::{Mutex, Arc};

pub trait CarnyxHost: Sync + Send{
    fn update_host_display(&self);
    fn resize_editor_window(&self, width: usize, height: usize);
}

pub trait CarnyxEditor{
    fn initial_size(&self)->(usize, usize);
    fn initial_position(&self)->(isize, isize);
    fn open(&mut self, handle: Option<RawWindowHandle>)->bool;
    fn is_open(&self)->bool;
}

pub trait CarnyxProcessor {
    type Model: CarnyxModel;
    type Editor: CarnyxEditor;

    fn model(&self)->Arc<Self::Model>;
    fn listener(&self)->SettableListener<Self::Model>;
    fn set_sample_rate(&mut self, rate: f32);
    fn parameters(&self)->Vec<Box<dyn CarnyxParam<Self::Model>>>;
    fn editor(&self)->Self::Editor;
    fn process(&mut self, buffer: &mut AudioBuffer<f32>);
}

pub trait CarnyxParam<Model: CarnyxModel>: Sync{
    fn name(&self, model: &Model) ->String;
    fn label(&self, model: &Model) ->String;
    fn get_value(&self, model: &Model) ->f32;
    fn set_value(&self, model: &Model, val: f32);
    fn formatted(&self, model: &Model) ->String;
}

pub trait CarnyxModelListener<Model> : Send{
    fn notify_change(&self, model: &Model);
}

pub struct SettableListener<Model>{
    listener: Arc<Mutex<Option<Box<dyn CarnyxModelListener<Model>>>>>,
}

impl <Model> Clone for SettableListener<Model>{
    fn clone(&self) -> Self {
        Self{
            listener: Arc::clone(&self.listener)
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.listener = Arc::clone(&source.listener)
    }

}


impl <Model> SettableListener<Model> {
    pub fn new() -> Self {
        Self {
            listener: Arc::new(Mutex::new(None))
        }
    }

    pub fn set_listener(&self, listener: Box<dyn CarnyxModelListener<Model>>) {
        if let Ok(mut listener_opt) = self.listener.lock() {
            *listener_opt = Some(listener);
        }
    }
}

impl <Model> CarnyxModelListener<Model> for SettableListener<Model>{
    fn notify_change(&self, model: &Model) {
        if let Ok(r) = self.listener.lock(){
            if let Some(l) = &*r {
                l.notify_change(model)
            }
        }
    }
}

pub trait CarnyxModel: 'static + Sync + Send {
    type Snap;
    fn snap(&self) -> Self::Snap;
    fn set_snap(&self, snap: &Self::Snap);
}

pub struct BasicParam<Params> {
    name: &'static str,
    label: &'static str,
    get: Box<dyn Fn(&Params)->f32 + Sync>,
    set: Box<dyn Fn(&Params, f32) + Sync>,
    format: Box<dyn Fn(&Params)->String + Sync>
}

impl <Params> BasicParam<Params> {
    pub fn new(name: &'static str, label: &'static str,
               get: impl Fn(&Params) -> f32 + 'static + Sync,
               set: impl Fn(&Params, f32) + 'static + Sync,
               format: impl Fn(&Params) -> String + 'static + Sync) -> Self {
        BasicParam { name, label,
            get: Box::new(get),
            set: Box::new(set),
            format: Box::new(format) }
    }
}

impl <Params: CarnyxModel> CarnyxParam<Params> for BasicParam<Params> {
    fn name(&self, _params: &Params) -> String {
        self.name.to_owned()
    }

    fn label(&self, _params: &Params) -> String {
        self.label.to_owned()
    }

    fn get_value(&self, params: &Params) -> f32 {
        (self.get)(params)
    }

    fn set_value(&self, params: &Params, val: f32) {
        (self.set)(params, val)
    }

    fn formatted(&self, params: &Params) -> String {
        (self.format)(params)
    }
}