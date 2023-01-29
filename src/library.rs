//! Libass library
//!

use core::{ptr::NonNull, slice};

use libass_sys::ASS_Library;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    marker::PhantomData,
    mem::ManuallyDrop,
};

use crate::track::Track;

/// Libass Library instance
#[derive(Debug, PartialEq)]
#[allow(clippy::missing_docs_in_private_items)]
pub struct Library {
    lib: *mut ASS_Library,
    phan: PhantomData<ASS_Library>,
}

impl Library {
    /// Construct a new Libass library instance
    ///
    /// Returns None if allocation in library fails.
    pub fn new() -> Option<Self> {
        // Safety: There is no global state this function acesses.
        let new = unsafe { libass_sys::ass_library_init() };

        if new.is_null() {
            None
        } else {
            Some(Self {
                lib: new,
                phan: PhantomData,
            })
        }
    }

    /// Set callback for logging.
    ///
    /// May reference count this closure later, but making it static is easiest for now.
    /// Since it shouldn't change often.
    pub fn set_message_cb<T>(&self, callback: T)
    where
        T: Fn(LogLevel, &str) + Send + Sync,
        T: 'static,
    {
        let mut leaked_cb = ManuallyDrop::new(callback);
        let mut cb: &mut dyn Fn(LogLevel, &str) = &mut leaked_cb as &mut T as _;
        let cb = &mut cb;

        // Safety: It is leaked and also static so it should last as long
        // as needed.
        unsafe {
            libass_sys::ass_set_message_cb(
                self.lib,
                Some(message_handler),
                cb as *mut _ as *mut c_void,
            )
        }
    }

    /// Get the avaliable font providers
    /// TODO: if allocator API is real then use it here because Libass allocates the array with the
    /// system allocator, which we are not guarenteed to be using.
    pub fn get_avaliable_font_providers(&self) -> Vec<FontProvider> {
        // Out pointers to pass to Libass.
        // This pointer is to an array. It is allocated by the system allocator.
        let mut out_ptr: Option<NonNull<i32>> = None;
        let mut count = 0;
        let mut buf = Vec::new();

        // Safety:
        // Since we created the pointer we know that Libass has exclusive access to the
        // references provided. Also checked that the pointers aren't leaked anywhere.
        unsafe {
            libass_sys::ass_get_available_font_providers(
                self.lib,
                (&mut out_ptr as *mut Option<NonNull<i32>>).cast(),
                &mut count as _,
            )
        }

        // So once returned we will see if null or not.
        // Safety: inspeting the source shows that it will either be null
        // or occupied.
        match out_ptr {
            Some(out_buf) => {
                let slice = unsafe { slice::from_raw_parts(out_buf.as_ptr().cast(), count) };
                buf.extend_from_slice(slice);
                unsafe {
                    libc::free(out_buf.as_ptr().cast());
                }
                buf
            }
            None => buf,
        }
    }

    /// Whether fonts should be extracted from the track data.
    pub fn extract_fonts(&self, extract: bool) {
        // Safety: This is basically just a setter on the library handle.
        unsafe { libass_sys::ass_set_extract_fonts(self.lib, extract.into()) }
    }

    /// Set additional font directory for lookup.
    /// TODO: Check the path invariants that are described
    /// in the Libass documentation.
    ///
    /// Libass copies the name so lifetime is managed for us.
    pub fn set_font_dir(&self, dir: &CStr) {
        // Safety:
        // Libass copies the string provided and doesn't leak the pointer at all.
        unsafe { libass_sys::ass_set_fonts_dir(self.lib, dir.as_ptr()) }
    }

    /// Load font in to library instance
    /// TODO: Get some font types for this.
    ///
    /// Internally Libass copies the string and the
    /// data so it manages the lifetimes.
    #[allow(dead_code, unused_variables, unreachable_code)]
    fn add_font<T>(&self, name: T, data: &[()]) -> ()
    where
        T: AsRef<CStr>,
    {
        /// Cute trick to reduce compile times.
        fn inner_font(lib: &Library, name: &CStr, data: &[()]) {
            // Safety:
            // It copies the name and doesn't leak the pointer anywhere
            // Data is also memcpy'd to the library through the handle.
            unsafe {
                libass_sys::ass_add_font(
                    lib.lib,
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
        unsafe { libass_sys::ass_clear_fonts(self.lib) }
        self
    }

    /// Register style overrides for this library instance.
    /// TODO: Actually implement this.
    /// Need to make some type for overrides.
    #[allow(dead_code, unreachable_code, unused_variables)]
    fn style_overrides(&self, overrides: &()) {
        todo!("Make custom style override type");
        // Safety
        // It copies the overrides so it doesn't outlive the owner.
        unsafe { libass_sys::ass_set_style_overrides(self.lib, overrides as *const () as _) }
    }

    /// Allocate new `Track` for a new subtitle stream.
    pub fn new_track(&self) -> Option<Track> {
        let new = unsafe { libass_sys::ass_new_track(self.lib) };
        if new.is_null() {
            None
        } else {
            Some(Track {
                track: new,
                lib: self,
                phantom: PhantomData,
            })
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // Safety:
        // :ferrisclueless:
        unsafe { libass_sys::ass_library_done(self.lib) }
    }
}

/// The Libass loglevel.
/// Anthing less than 5 is reported to stderr if
/// a callback is not registered with `Library::set_message_cb`
#[derive(Debug, Default, PartialEq, PartialOrd, Copy, Clone)]
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
    #[default]
    Application = 5,
    /// Not reported to stderr
    Verbose = 6,
    /// Not reported to stderr
    Debug = 7,
}

impl From<i32> for LogLevel {
    fn from(log_level: i32) -> Self {
        match log_level {
            0 => LogLevel::Fatal,
            1 => LogLevel::Error,
            2 => LogLevel::Warn,
            4 => LogLevel::Info,
            6 => LogLevel::Verbose,
            7 => LogLevel::Debug,
            _ => LogLevel::Application,
        }
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
    let mess = {
        if fmt.is_null() {
            // Safety:
            // the string has a static lifetime and is valid UTF-8 (empty).
            unsafe {
                CStr::from_bytes_with_nul_unchecked(b"\0")
                    .to_str()
                    .unwrap_unchecked()
            }
        } else {
            // Safety:
            // I know that it will atleast live through 'a
            // But I have no checked every log callsite so
            // let's hope for the best that fmt is always valid.
            unsafe { CStr::from_ptr(fmt).to_str().unwrap_or("") }
        }
    };
    let log_lev = level.into();

    // Safety:
    // I believe this is correct because it has Send+Sync bounds, so it should be safe to call
    // concurrently, though I do not believe Libass does? It may though. Libass keeps a pointer to
    // the closure within itself, but it doesn't modify the data at all as it is a c_void.
    let data = unsafe { data.cast::<&dyn Fn(LogLevel, &str)>().as_ref() };

    if let Some(data_ref) = data {
        let closure = data_ref;
        closure(log_lev, mess)
    }
}

/// Font provider to use for rendering.
#[repr(i32)]
#[derive(Debug, Default, PartialEq, Copy, Clone, PartialOrd)]
#[non_exhaustive]
pub enum FontProvider {
    /// Don't use any default font provider for font lookup.
    None = libass_sys::ASS_DefaultFontProvider::ASS_FONTPROVIDER_NONE,
    /// Use the first avaliable font provider.
    #[default]
    Autodetect = libass_sys::ASS_DefaultFontProvider::ASS_FONTPROVIDER_AUTODETECT,
    /// Force Coretext (OSX Only)
    CoreText = libass_sys::ASS_DefaultFontProvider::ASS_FONTPROVIDER_CORETEXT,
    /// Force a Fontconfig-based font provider
    Fontconfig = libass_sys::ASS_DefaultFontProvider::ASS_FONTPROVIDER_FONTCONFIG,
    /// Force a DirectWrite-based font provider (Windows only)
    DirectWrite = libass_sys::ASS_DefaultFontProvider::ASS_FONTPROVIDER_DIRECTWRITE,
}

impl From<i32> for FontProvider {
    fn from(value: i32) -> Self {
        use libass_sys::ASS_DefaultFontProvider::*;
        use FontProvider::*;
        match value {
            ASS_FONTPROVIDER_AUTODETECT => Autodetect,
            ASS_FONTPROVIDER_CORETEXT => CoreText,
            ASS_FONTPROVIDER_FONTCONFIG => Fontconfig,
            ASS_FONTPROVIDER_DIRECTWRITE => DirectWrite,
            _ => None,
        }
    }
}
