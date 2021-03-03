use carnyx::{CarnyxModel, CarnyxParam, CarnyxHost, CarnyxEditor, CarnyxModelListener, CarnyxWindowResizer};
use vst::plugin::{PluginParameters, HostCallback};
use std::sync::Arc;
use vst::host::Host;
use std::ffi::{CString, c_void};
use vst::editor::Editor;
use raw_window_handle::RawWindowHandle;


pub struct VstParams<DP: CarnyxModel, L: CarnyxModelListener<DP> + Sync>{
    params: Vec<Box<dyn CarnyxParam<DP>>>,
    inner: Arc<DP>,
    listener: L
}

impl<DP: CarnyxModel, L: CarnyxModelListener<DP> + Sync> VstParams<DP, L> {
    pub fn new(params: Vec<Box<dyn CarnyxParam<DP>>>, inner: Arc<DP>, listener: L) -> Self {
        VstParams { params, inner, listener }
    }
}

impl <DP: CarnyxModel, L: CarnyxModelListener<DP> + Sync> PluginParameters for VstParams<DP, L> {
    fn get_parameter_label(&self, index: i32) -> String {
        let param = self.params.get(index as usize);
        param.map(|p|p.label(&self.inner)).unwrap_or_else(||"".to_owned())
    }

    fn get_parameter_text(&self, index: i32) -> String {
        let param = self.params.get(index as usize);
        param.map(|p|p.formatted(&self.inner)).unwrap_or_else(||"".to_owned())
    }

    fn get_parameter_name(&self, index: i32) -> String {
        let param = self.params.get(index as usize);
        param.map(|p|p.name(&self.inner)).unwrap_or_else(||"".to_owned())
    }

    // get_parameter has to return the value used in set_parameter
    fn get_parameter(&self, index: i32) -> f32 {
        let param = self.params.get(index as usize);
        param.map(|p|p.get_value(&self.inner)).unwrap_or(0.0)
    }

    fn set_parameter(&self, index: i32, value: f32) {
        let param = self.params.get(index as usize);
        param.map(|p|p.set_value(&self.inner, value));
        self.listener.notify_change(&self.inner)
    }
}

pub struct VstCarnyxHost{
    inner: HostCallback
}

impl VstCarnyxHost {
    pub fn new(host_callback: HostCallback) -> Self {
        VstCarnyxHost { inner: host_callback }
    }

    pub fn resizer(&self)->Box<dyn CarnyxWindowResizer>{
        Box::new(VstCarnyxResizer{
            inner: self.inner.clone()
        })
    }
}

impl CarnyxHost for VstCarnyxHost {
    fn update_host_display(&self) {
        if self.inner.raw_callback().is_some() {
            self.inner.update_display()
        }
    }
}

pub struct VstCarnyxResizer {
    inner: HostCallback
}

impl VstCarnyxResizer {
    pub fn new(inner: HostCallback) -> Self {
        VstCarnyxResizer { inner }
    }
}

impl CarnyxWindowResizer for VstCarnyxResizer{
    fn resize_editor_window(&self, width: usize, height: usize)->bool {
        let (_, vendor, _) = self.inner.get_info();
        let is_ableton = "Ableton".eq(&vendor);


        if let Some(callback) = self.inner.raw_callback() {
            let effect = self.inner.raw_effect();
            let string = CString::new("sizeWindow").unwrap();

            let res = callback(
                effect,
                vst::host::OpCode::CanDo.into(),
                0,
                0,
                string.as_bytes().as_ptr() as *mut c_void,
                0.,
            );
            if res == 1 || is_ableton {
                let res = callback(
                    effect,
                    vst::host::OpCode::SizeWindow.into(),
                    width as i32,
                    height as isize,
                    std::ptr::null_mut(),
                    0.,
                );
                return res == 1
            }
        }
        false
    }
}

pub struct VstCarnyxEditor<C: CarnyxEditor>{
    inner: C,
    host_callback: HostCallback
}

impl<C: CarnyxEditor> VstCarnyxEditor<C> {
    pub fn new(inner: C, host_callback: HostCallback) -> Self {
        VstCarnyxEditor { inner, host_callback }
    }
}

#[cfg(target_os = "macos")]
fn to_raw_window_handle(parent: *mut c_void) -> RawWindowHandle {
    use raw_window_handle::macos::MacOSHandle;
    RawWindowHandle::MacOS(MacOSHandle {
        ns_view: parent as *mut _,
        ..MacOSHandle::empty()
    })
}

#[cfg(target_os = "windows")]
fn to_raw_window_handle(parent: *mut c_void) -> RawWindowHandle {
    use raw_window_handle::windows::WindowsHandle;
    RawWindowHandle::Windows(WindowsHandle {
        hwnd: parent as *mut _,
        ..WindowsHandle::empty()
    })
}

impl <C: CarnyxEditor> Editor for VstCarnyxEditor<C>{
    fn size(&self) -> (i32, i32) {
        let (w, h) = self.inner.initial_size();
        (w as i32, h as i32)
    }

    fn position(&self) -> (i32, i32) {
        let (x, y) = self.inner.initial_position();
        (x as i32, y as i32)
    }

    fn open(&mut self, parent: *mut c_void) -> bool {
        self.inner.open(Some(to_raw_window_handle(parent)), Box::new(VstCarnyxResizer::new( self.host_callback )))
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }
}