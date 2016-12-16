use user32;
use winapi as w;

use std::ffi::OsStr;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

pub mod com;
pub mod sapi;

fn cvt(r: w::HRESULT) -> io::Result<w::HRESULT> {
    if w::SUCCEEDED(r) {
        Ok(r)
    } else {
        Err(io::Error::from_raw_os_error(r))
    }
}

pub trait ToWide {
    fn to_wide(&self) -> Vec<u16>;
}

impl<T> ToWide for T where T: AsRef<OsStr> {
    fn to_wide(&self) -> Vec<u16> {
        self.as_ref().encode_wide().chain(iter::once(0)).collect()
    }
}

pub fn error_message_box(message: &str) {
    let message = message.to_wide();
    let caption = text!("Error").to_wide();

    unsafe {
        user32::MessageBoxW(ptr::null_mut(), message.as_ptr(), caption.as_ptr(),
                            w::MB_SETFOREGROUND | w::MB_SYSTEMMODAL | w::MB_ICONERROR | w::MB_OK);
    }
}