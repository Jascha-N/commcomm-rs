use super::cvt;

use ole32;
use winapi as w;

use std::io;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;

pub unsafe trait Interface {
    fn iid() -> &'static w::IID;
}

pub struct Pointer<T>(*mut T);

impl<T> Pointer<T> {
    pub unsafe fn from_raw(ptr: *mut T) -> Pointer<T> {
        Pointer(ptr)
    }

    pub fn upcast<U>(self) -> Pointer<U> where T: Deref<Target=U> {
        Pointer(self.into_raw() as *mut U)
    }

    // pub unsafe fn as_mut(&self) -> &mut T {
    //     &mut *self.0
    // }

    fn into_raw(self) -> *mut T {
        let result = self.0;
        mem::forget(self);
        result
    }

    fn as_unknown(&self) -> &mut w::IUnknown {
        unsafe { &mut *(self.0 as *mut w::IUnknown) }
    }
}

impl<T: Interface> Pointer<T> {
    pub fn create(clsid: &w::CLSID) -> io::Result<Pointer<T>> {
        unsafe {
            let mut instance = mem::uninitialized();
            cvt(ole32::CoCreateInstance(clsid, ptr::null_mut(), w::CLSCTX_ALL, T::iid(),
                                        &mut instance as *mut _ as *mut _))?;
            Ok(Pointer(instance))
        }
    }

    pub fn downcast<U: Interface>(&self) -> Option<Pointer<U>> {
        unsafe {
            let mut instance = mem::uninitialized();
            cvt(self.as_unknown().QueryInterface(U::iid(), &mut instance as *mut _ as *mut _)).map(|_| {
                Some(Pointer(instance))
            }).unwrap_or(None)
        }
    }
}

impl<T> Drop for Pointer<T> {
    fn drop(&mut self) {
        unsafe {
            self.as_unknown().Release();
        }
    }
}

impl<T> Deref for Pointer<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for Pointer<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0 }
    }
}

impl<T> Clone for Pointer<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.as_unknown().AddRef();
            Pointer::from_raw(self.0)
        }
    }
}

impl<T> PartialEq<Pointer<T>> for Pointer<T> {
    fn eq(&self, other: &Pointer<T>) -> bool {
        self.0 == other.0
    }
}