//! Libass track handle methods
//!
use std::marker::PhantomData;

use crate::library::Library;

/// Handle to a Libass track object.
#[derive(PartialEq, Debug)]
#[allow(clippy::missing_docs_in_private_items)]
pub struct Track<'lib> {
    pub(crate) track: *mut libass_sys::ASS_Track,
    pub(crate) phantom: PhantomData<libass_sys::ASS_Track>,
    pub(crate) lib: &'lib Library,
}

impl Track<'_> {
    /// Explicilty processes styles that have been overridden.
    pub fn force_process_styles(&self) {
        unsafe { libass_sys::ass_process_force_style(self.track) }
    }

    /// Enable or disable features for the track.
    /// Will return as Some if successful. Will return None if the status
    /// of the feature is unknown.
    pub fn set_feature(&self, feat: Feature, enable: bool) -> Option<()> {
        let code =
            unsafe { libass_sys::ass_track_set_feature(self.track, feat as _, enable.into()) };

        match code {
            0 => Some(()),
            _ => None,
        }
    }

    /// Allocate new style for track.
    pub fn alloc_style(&self) -> Result<Style, AllocError> {
        let code = unsafe { libass_sys::ass_alloc_style(self.track) };
        if code >= 0 {
            Ok(Style(code, self))
        } else {
            Err(AllocError())
        }
    }

    /// Allocate new event handle
    pub fn alloc_event(&self) -> Result<Event, AllocError> {
        let code = unsafe { libass_sys::ass_alloc_event(self.track) };

        if code >= 0 {
            Ok(Event(code, self))
        } else {
            Err(AllocError())
        }
    }

    /// Parse a chunkc of subtitle stream data.
    ///
    /// TODO: Look at if there are any invariants needed for this data.
    ///
    /// It looks like it's a slice of string data, but does it need to end/begin on any boundaries
    /// within the stream?
    ///
    /// Currently the only way this function can fail is if a slice that is too large to be indexed
    /// by an i32 is passed.
    pub fn process_slice(&self, data: &str) -> Result<(), SliceTooLong> {
        // Safety:
        // Inspecting the C function, it soundly copies the data over and does not leak the
        // reference.

        match data.len().try_into() {
            Ok(length) => unsafe {
                libass_sys::ass_process_data(self.track, data.as_ptr().cast_mut() as _, length);
                Ok(())
            },
            Err(_) => Err(SliceTooLong()),
        }
    }

    /// Parse and process the Codec Private section of the subtitle stream in the Matroska format.
    ///
    /// Currently can only fail if provided a slice that cannot be indexed by an i32.
    #[allow(dead_code)]
    fn process_codec_private(&self, data: &str) -> Result<(), SliceTooLong> {
        // Safety:
        // Inspecting the C function, it soundly copies the data in to the library internals and
        // does not leak the reference.

        match data.len().try_into() {
            Ok(length) => unsafe {
                libass_sys::ass_process_codec_private(
                    self.track,
                    data.as_ptr().cast_mut() as _,
                    length,
                );
                Ok(())
            },
            Err(_) => Err(SliceTooLong()),
        }
    }

    /// Parse a chuck of subtitle data that corresponds to exactly one Matroska event.
    ///
    /// TODO: Find a library the has some MKV types to feed this thing.
    /// TODO: Time? What's the time library to use now days.
    #[allow(dead_code)]
    fn process_chunk(&self, data: &str, timestamp: i64, duration: i64) {
        unsafe {
            libass_sys::ass_process_chunk(
                self.track,
                data.as_ptr().cast_mut() as _,
                data.len().try_into().unwrap(),
                timestamp,
                duration,
            )
        }
    }
}

impl Drop for Track<'_> {
    fn drop(&mut self) {
        // Safety:
        // :ferrisclueless:
        //
        // This specific function doesn't need to be called before
        // the library is, but most of the methods on it can't be called
        // after the library is dropped.
        unsafe { libass_sys::ass_free_track(self.track) }
    }
}

/// Allocation failure in Libass
#[derive(Debug)]
pub struct AllocError();

/// Slice that is too large for Libass to be able to index (i32)
#[derive(Debug)]
pub struct SliceTooLong();

#[derive(Debug)]
#[repr(i32)]
#[non_exhaustive]
/// Features that can be enabled or disabled for a track
/// with `Track::set_feature`.
pub enum Feature {
    /// Enable libass extensions that would display ASS subtitles incorrectly.
    /// These may be useful for applications, which use libass as renderer for
    /// subtitles converted from another format, or which use libass for other
    /// purposes that do not involve actual ASS subtitles authored for
    /// distribution.
    IncompatibleExtensions = 0,
    /// Match bracket pairs in bidirectional text according to the revised
    /// Unicode Bidirectional Algorithm introduced in Unicode 6.3.
    /// This is incompatible with VSFilter and disabled by default.
    ///
    /// (Directional isolates, also introduced in Unicode 6.3,
    /// are unconditionally processed when FriBidi is new enough.)
    ///
    /// This feature may be unavailable at runtime (ass_track_set_feature
    /// may return -1) if libass was compiled against old FriBidi.
    BidirectionalBrackets = 1,
    /// When this feature is disabled, text is split into VSFilter-compatible
    /// segments and text in each segment is processed in isolation.
    /// Notably, this includes running the Unicode Bidirectional
    /// Algorithm and shaping the text within each run separately.
    /// The individual runs are then laid out left-to-right,
    /// even if they contain right-to-left text.
    ///
    /// When this feature is enabled, each event's text is processed as a whole
    /// (as far as possible). In particular, the Unicode Bidirectional
    /// Algorithm is run on the whole text, and text is shaped across
    /// override tags.
    ///
    /// This is incompatible with VSFilter and disabled by default.
    ///
    /// libass extensions to ASS such as Encoding -1 can cause individual
    /// events to be always processed as if this feature is enabled.:
    WholeTextLayout = 2,
    /// Break lines according to the Unicode Line Breaking Algorithm.
    /// If the track language is set, some additional language-specific tweaks
    /// may be applied. Setting this enables more breaking opportunities
    /// compared to classic ASS. However, it is still possible for long words
    /// without breaking opportunities to cause overfull lines.
    /// This is incompatible with VSFilter and disabled by default.
    ///
    /// This feature may be unavailable at runtime if
    /// libass was compiled without libunibreak support.
    WrapUnicode = 3,
}

/// Style for a track
///
/// TODO: Also must think about deallocation.
/// I currently have a reference to the track included.
/// But I'm not sure if neeeded.
///
/// The Libass docs say that deallocating a Style without subsequently
/// setting the ID field number is UB. But there is no way to do so
/// just from Ass.h. So will probably just have to rely upon
/// dropping the Track
#[derive(Debug, PartialEq)]
pub struct Style<'track, 'lib>(i32, &'track Track<'lib>);

/// Event handle for a track
///
/// TODO: Think about how to deallocate without trivial UB.  
/// Easy way: Hold reference to Track.  
/// Hard way: IDK some way to brand it? :ferrisclueless:
#[derive(Debug, PartialEq)]
pub struct Event<'track, 'lib>(i32, &'track Track<'lib>);
