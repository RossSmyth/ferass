//! Libass library
//!

use core::{ptr::NonNull, slice};

use libass_sys::ASS_Library;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    marker::PhantomData,
    mem::ManuallyDrop,
};

/// Libass Library instance
#[derive(Debug)]
pub struct Library {
    lib: *const ASS_Library,
    phan: PhantomData<ASS_Library>,
}

impl Library {
    /// Construct a new Libass library instance
    pub fn new() -> Self {
        // Safety: There is no global state this function acesses.
        Self {
            lib: unsafe { libass_sys::ass_library_init() },
            phan: PhantomData,
        }
    }

    /// Set callback for logging.
    /// The callback lifetime should be tied to the library object
    /// but I need to map out the lifetimes of everything first.
    pub fn set_message_cb<T>(&self, callback: T)
    where
        T: Fn(LogLevel, &CStr) + Send + Sync,
        T: 'static,
    {
        let mut leaked_cb = ManuallyDrop::new(callback);
        let mut cb: &mut dyn Fn(LogLevel, &CStr) = &mut leaked_cb as &mut T as _;
        let cb = &mut cb;
        
        // Safety: It is leaked and also static so it should last as long
        // as needed.
        unsafe {
            libass_sys::ass_set_message_cb(
                self.lib.cast_mut(),
                Some(message_handler),
                cb as *mut _ as *mut c_void,
            )
        }
    }

    /// Get the avaliable font providers
    pub fn get_avaliable_font_providers(&self) -> &'static [i32] {
        let mut buf: Option<NonNull<i32>> = None;
        let mut count = 0;

        // Safety: 
        // Since we created the pointer we know that Libass has exclusive access to the
        // references provided. Also checked that the pointers aren't leaked anywhere.
        unsafe {
            libass_sys::ass_get_available_font_providers(
                self.lib.cast_mut(),
                std::mem::transmute(&mut buf),
                &mut count as _,
            )
        }
        
        // So once returned we will see if null or not.
        // Safety: inspeting the source shows that it will either be null
        // or occupied.
        match buf {
            Some(buf) => unsafe { slice::from_raw_parts(buf.as_ptr().cast(), count) },
            None => &[],
        }
    }

    /// Whether fonts should be extracted from the track data.
    pub fn extract_fonts(&mut self, extract: bool) {
        // Safety: This is basically just a setter on the library handle.
        unsafe { libass_sys::ass_set_extract_fonts(self.lib.cast_mut(), extract.into()) }
    }

    /// Set additional font directory for lookup.
    /// TODO: Check the path invariants that are described
    /// in the Libass documentation.
    ///
    /// Libass copies the name so lifetime is managed for us.
    pub fn set_font_dir(&mut self, dir: &CStr) {
        // Safety: 
        // Libass copies the string provided and doesn't leak the pointer at all.
        unsafe { libass_sys::ass_set_fonts_dir(self.lib.cast_mut(), dir.as_ptr()) }
    }

    /// Load font in to library instance
    /// TODO: Get some font types for this.
    ///
    /// Internally Libass copies the string and the
    /// data so it manages the lifetimes.
    #[allow(dead_code, unused_variables, unreachable_code)]
    fn add_font<T>(&mut self, name: T, data: &[()]) -> ()
    where
        T: AsRef<CStr>,
    {
        fn inner_font(lib: &mut Library, name: &CStr, data: &[()]) {
            // Safety:
            // It copies the name and doesn't leak the pointer anywhere
            // Data is also memcpy'd to the library through the handle.
            unsafe {
                libass_sys::ass_add_font(
                    lib.lib.cast_mut(),
                    name.as_ptr(),
                    data.as_ptr() as *const i8,
                    data.len().try_into().unwrap(),
                )
            }
        }
        todo!("Need some font types. Check out font-kit?");
        inner_font(self, name.as_ref(), data)
    }

    /// Clear all fonts associated with the Library instance
    ///
    /// Must take ownership of the Library because all Track
    /// and Render instance must be released before this
    /// method can be called.
    pub fn clear_fonts(self) -> Self {
        // Safety:
        // It frees memory in the library that was allocated within the library.
        unsafe { libass_sys::ass_clear_fonts(self.lib.cast_mut()) }
        self
    }

    /// Register style overrides for this library instance.
    #[allow(dead_code, unreachable_code, unused_variables)]
    fn style_overrides(&mut self, overrides: &()) {
        todo!("Make custom style override type");
        // Safety
        // It copies the overrides so it doesn't outlive the owner.
        unsafe {
            libass_sys::ass_set_style_overrides(self.lib.cast_mut(), overrides as *const () as _)
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // Safety:
        // :ferrisclueless:
        unsafe { libass_sys::ass_library_done(self.lib.cast_mut()) }
    }
}

/// The Libass loglevel.
/// Anthing less than 5 is reported to stderr if
/// a callback is not registered with `Library::set_message_cb`
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
#[repr(i32)]
pub enum LogLevel {
    /// Fatal errors are reported as this. Reported to stderr
    Fatal = 0,
    /// Reported to stderr
    Error = 1,
    /// Reported to stderr
    Warn = 2,
    /// Reported to stderr
    Info = 4,
    /// The recommended level for applications to use. Not reported to stderr.
    Application = 5,
    /// Not reported to stderr
    Verbose = 6,
    /// Not reported to stderr
    Debug = 7,
}

impl From<i32> for LogLevel {
    fn from(log_level: i32) -> Self {
        // Safety:
        // repr i32 & non_exhaustive should be enough I think.
        unsafe { std::mem::transmute(log_level) }
    }
}

/// Handler for the libass logging
/// TODO: Figure out something to do with the variadic argument
extern "C" fn message_handler(
    level: c_int,
    fmt: *const c_char,
    _: libass_sys::va_list,
    data: *mut c_void,
) {
    let mess = unsafe { CStr::from_ptr(fmt) };
    let log_lev = level.into();

    let closure: &mut &mut dyn Fn(LogLevel, &CStr) = unsafe { std::mem::transmute(data) };
    closure(log_lev, mess)
}
