//! Renderer module
//!
use std::{
    ffi::{CString, OsString},
    marker::PhantomData,
    mem::ManuallyDrop,
    path::PathBuf,
    ptr::NonNull,
};
use time::Duration;

use libass_sys;
use thiserror::Error;

use crate::{library::FontProvider, Library, Track};

/// Handle to a Libass rendering instance.
///
/// Constructed from a `Library` handle. See `Library::new_renderer`.
#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::missing_docs_in_private_items)]
pub struct Renderer<'lib> {
    pub(crate) renderer: NonNull<libass_sys::ASS_Renderer>,
    pub(crate) data: PhantomData<libass_sys::ASS_Renderer>,
    pub(crate) parent: &'lib Library,
}

impl<'lib> Renderer<'lib> {
    /// Set the frame size in pixels, including margins.
    ///
    /// he renderer will never return images that are outside of the frame area. The value set with
    /// this function can influence the pixel aspect ratio used for rendering. If after
    /// compensating for configured margins the frame size is not an isotropically scaled version
    /// of the video display size, you may have to use set_pixel_aspect().
    pub fn set_frame_size(&self, width: u32, height: u32) {
        // Safety:
        // This is basically a setter.
        // It just sets some values on the handle struct.
        // Along with some other stuff that I think is safe
        unsafe {
            libass_sys::ass_set_frame_size(
                self.renderer.as_ptr(),
                width.try_into().unwrap_or(0),
                height.try_into().unwrap_or(0),
            )
        }
    }

    /// Set the source image size in pixels.
    ///
    /// This affects some ASS tags like e.g. 3D transforms and is used to calculate the source
    /// aspect ratio and blur scale. If subtitles specify valid LayoutRes* headers, those will take
    /// precedence. The source image size can be reset to default by setting w and h to 0. The
    /// value set with this function can influence the pixel aspect ratio used for rendering.
    ///
    /// The values must be the actual storage size of the video stream, without any anamorphic
    /// de-squeeze applied.
    pub fn set_storage_size(&self, width: u32, height: u32) {
        // Safety:
        // This is basically just a field setter.
        unsafe {
            libass_sys::ass_set_storage_size(
                self.renderer.as_ptr(),
                width.try_into().unwrap_or(0),
                height.try_into().unwrap_or(0),
            )
        }
    }

    /// Set shaping level. This is merely a hint, the renderer will use whatever is available if
    /// the request cannot be fulfilled.
    pub fn set_shaper(&self, level: ShapingLevel) {
        // Safety:
        // Just a setter
        unsafe { libass_sys::ass_set_shaper(self.renderer.as_ptr(), level as _) }
    }

    /// Set frame margins.
    ///
    /// These values may be negative if pan-and-scan is used. The margins are in pixels. Each value
    /// specifies the distance from the video rectangle to the renderer frame. If a given margin
    /// value is positive, there will be free space between renderer frame and video area. If a
    /// given margin value is negative, the frame is inside the video, i.e. the video has been
    /// cropped.
    ///
    /// The renderer will try to keep subtitles inside the frame area. If possible, text is layout
    /// so that it is inside the cropped area. Subtitle events that can't be moved are cropped
    /// against the frame area.
    ///
    /// `Renderer::use_margins()` can be used to allow libass to render subtitles into  the empty
    /// areas if margins are positive, i.e. the video area is smaller than the frame.
    /// (Traditionally, this has been used to show subtitles in the bottom "black bar" between
    /// video bottom screen border when playing 16:9 video on a 4:3 screen.).
    pub fn set_margins(
        &self,
        top_margin: i32,
        bottom_margin: i32,
        left_margin: i32,
        right_margin: i32,
    ) {
        // Safety:
        // Just a setter
        unsafe {
            libass_sys::ass_set_margins(
                self.renderer.as_ptr(),
                top_margin,
                bottom_margin,
                left_margin,
                right_margin,
            )
        }
    }

    /// Whether margins should be used for placing regular events.
    pub fn use_margins(&self, r#use: bool) {
        // Safety: setter
        unsafe { libass_sys::ass_set_use_margins(self.renderer.as_ptr(), r#use as _) }
    }

    /// Set pixel aspect ratio correction. This is the ratio of pixel width to pixel height.
    ///
    /// Generally, this is (d_w / d_h) / (s_w / s_h), where s_w and s_h is the video storage size,
    /// and d_w and d_h is the video display size. (Display and storage size can be different for
    /// anamorphic video, such as DVDs.)
    ///
    /// If the pixel aspect ratio is 0, or if the aspect ratio has never been set by calling this
    /// function, libass will calculate a default pixel aspect ratio out of values set with
    /// ass_set_frame_size() and ass_set_storage_size(). Note that this default assumes the frame
    /// size after compensating for margins corresponds to an isotropically scaled version of the
    /// video display size. If the storage size has not been set, a pixel aspect ratio of 1 is
    /// assumed.
    ///
    /// If subtitles specify valid LayoutRes* headers, the API-configured pixel aspect value is
    /// discarded in favour of one calculated out of the headers and values set with
    /// ass_set_frame_size().
    pub fn set_pixel_aspect(&self, aspect_ratio: f64) {
        // Safety: setter
        unsafe { libass_sys::ass_set_pixel_aspect(self.renderer.as_ptr(), aspect_ratio) }
    }

    /// Set a fixed font scaling factor.
    pub fn set_font_scale(&self, scale: f64) {
        // Safety: setter
        unsafe { libass_sys::ass_set_font_scale(self.renderer.as_ptr(), scale) }
    }

    /// Set font hinting method
    ///
    /// Setting hinting to anything but ASS_HINTING_NONE will put libass in a mode that reduces
    /// compatibility with vsfilter and many ASS scripts. The main problem is that hinting
    /// conflicts with smooth scaling, which precludes animations and precise positioning. In other
    /// words, enabling hinting might break some scripts severely.
    ///
    /// FreeType's native hinter is still buggy sometimes and it is recommended to use the light
    /// autohinter, ASS_HINTING_LIGHT, instead. For best compatibility with problematic fonts,
    /// disable hinting.
    pub fn set_font_hinting(&self, method: FontHinting) {
        // Safety: setter
        unsafe { libass_sys::ass_set_hinting(self.renderer.as_ptr(), method as _) }
    }

    /// Set line spacing. Will not be scaled with frame size.
    ///
    /// This spacing is in pixels.
    pub fn set_line_spacing(&self, spacing: f64) {
        unsafe { libass_sys::ass_set_line_spacing(self.renderer.as_ptr(), spacing) }
    }

    /// Set vertical line position in percent.
    ///
    /// The range for this value is 0 to 100. If outside this range it will be clamped to the range. The
    /// default value in Libass is 0.
    ///
    /// 0 = on the bottom
    ///
    /// 100 = on top
    pub fn set_line_position(&self, position: f64) {
        // Safety: setter
        unsafe {
            libass_sys::ass_set_line_position(self.renderer.as_ptr(), position.clamp(0.0, 100.0))
        }
    }

    /// Set font lookup defaults.
    ///
    /// # Arguments
    ///
    /// * `default_font` - Path to default font to use. Must be supplied if all system
    /// fontproviders are disabled or unavailable.
    ///
    /// * `default_family` - Fallback font family
    ///
    /// * `font_provider` - Which font provider to use If the requested fontprovider does not exist
    /// or fails to initialize, the behavior is the same as when `FontProvider::None` is passed.
    ///
    /// * `fontconfig_config` - Path to Fontconfig configuration file. Only relevant if fontconfig
    /// is used. The encoding must match the one accepted by fontconfig.
    ///
    /// * `update` - Whether Fontconfig cache should be built/updated now. Only relevant if
    /// Fontconfig is used.
    ///
    /// Currently unsound to have the PathBuf's allocated with anything except for the system
    /// allocater.
    pub fn set_fonts(
        &self,
        default_font: Option<PathBuf>,
        default_family: Option<PathBuf>,
        font_provider: FontProvider,
        mut fontconfig_config: Option<PathBuf>,
        update: bool,
    ) -> Result<(), PathErr> {
        // Need to leak everything.
        // Also it's unsound for anything to be allocated with anything except the system
        // allocator so fun.
        let font_out = match default_font {
            Some(path) => path_to_ptr(path)?,
            None => core::ptr::null(),
        };
        let family_out = match default_family {
            Some(path) => path_to_ptr(path)?,
            None => core::ptr::null(),
        };
        if font_provider == FontProvider::Fontconfig {
            fontconfig_config = None;
        }

        let config_out = match fontconfig_config {
            Some(path) => path_to_ptr(path)?,
            None => core::ptr::null(),
        };

        // Safety:
        // ferrisclueless
        unsafe {
            libass_sys::ass_set_fonts(
                self.renderer.as_ptr(),
                font_out,
                family_out,
                font_provider as _,
                config_out,
                update as _,
            )
        }
        Ok(())
    }

    /// Set selective style override mode.
    ///
    /// If enabled, the renderer attempts to override the ASS script's styling of normal subtitles,
    /// without affecting explicitly positioned text. If an event looks like a normal subtitle,
    /// parts of the font style are copied from the user style set with with method.
    ///
    /// Warning: the heuristic used for deciding when to override the style is rather rough, and
    /// enabling this option can lead to incorrectly rendered subtitles. Since the ASS format
    /// doesn't have any support for allowing end-users to customize subtitle styling, this feature
    /// can only be implemented on "best effort" basis, and has to rely on heuristics that can
    /// easily break.
    pub fn set_selective_style_override_flags(&self, flags: OverrideBits) {
        unsafe {
            libass_sys::ass_set_selective_style_override_enabled(self.renderer.as_ptr(), flags.bits)
        }
    }

    /// Set style for selective style override.
    ///
    /// See `Renderer::selective_style_override_flags()`.
    ///
    /// Style style settings to use if override is enabled.
    /// TODO: Make this real.
    #[allow(dead_code)]
    fn set_selective_style_override(&self, _: &()) {
        todo!("Make style type(s)")
    }

    /// Set hard cache limits.  Do not set, or set to zero, for reasonable defaults.
    ///
    /// # Arguments
    ///
    /// * `glyph_max` - Maximum number of cached glyphs
    /// * `bitmap_cache` - Maximum bitmap cache size in MB
    pub fn set_cache_limits(&self, glyph_max: u32, bitmap_cache: u32) {
        // Safety:
        // setter
        unsafe {
            libass_sys::ass_set_cache_limits(
                self.renderer.as_ptr(),
                glyph_max.try_into().unwrap_or(i32::MAX),
                bitmap_cache.try_into().unwrap_or(i32::MAX),
            )
        }
    }

    /// Render a frame, producing a list of images.
    /// TODO: wut is detect change
    /// TODO: Linked list things
    /// TODO: How does this need to be called?
    /// It provides a linked list of images, but what is that linked list? Is it for the entire
    /// track after the timestamp provided, or must you call the function again for the next
    /// timestamp?
    #[allow(dead_code, unused_variables)]
    fn render_frame(
        &self,
        track: &Track,
        timestamp: &Duration,
        detect_change: &mut Option<ChangeDetection>,
    ) -> Option<*const ()> {
        let mut out_value = 0;
        let out_ptr = match detect_change {
            Some(val) => {
                out_value = *val as i32;
                &mut out_value as *mut i32
            }
            None => core::ptr::null_mut(),
        };

        let image_out = NonNull::new(unsafe {
            libass_sys::ass_render_frame(
                self.renderer.as_ptr(),
                track.track.as_ptr(),
                // Not really the proper error handling but oh well.
                timestamp.whole_milliseconds().try_into().ok()?,
                out_ptr,
            )
        });

        *detect_change = detect_change.map(|_| out_value.try_into().expect("Libass has changed and can return invalid values from the detect_change out ptr in ass_render_frame"));
        image_out.map(|inner| inner.as_ptr().cast_const().cast())
    }
}

impl Drop for Renderer<'_> {
    fn drop(&mut self) {
        // Safety:
        // :ferrisclueless:
        // I think it's safe to call. It just frees all it's fields.
        unsafe { libass_sys::ass_renderer_done(self.renderer.as_ptr()) }
    }
}

bitflags::bitflags! {
    /// Style override flags.
    #[repr(transparent)]
    pub struct OverrideBits: i32 {
        /// Default mode (with no other bits set). All selective override features as well as the
        /// style set with `set_selective_style_override()` are disabled, but traditional
        /// overrides like `set_font_scale()` are applied unconditionally.
        const DEFAULT = 0;
        /// Apply the style as set with `Renderer::set_selective_style_override()` on events which look
        /// like dialogue. Other style overrides are also applied this way, except
        /// `Renderer::set_font_scale()`. How `Renderer::set_font_scale()` is applied epends on the
        /// `OverrideBits::SELECTIVE_FONT_SCALE` flag.
        ///
        /// This is equivalent to setting all of the following bits:
        /// * `FONT_NAME`
        /// * `FONT_SIZE_FIELDS`
        /// * `BIT_COLORS`
        /// * `BIT_BORDER`
        /// * `ATTRIBUTES`
        const STYLE = 1 << 0;
        /// Apply `Renderer::set_font_scale()` only on events which look like dialogue. If not set,
        /// the font scale is applied to all events. (The behavior and name of this flag are
        /// unintuitive, but exist for compatibility)
        const SELECTIVE_FONT_SCALE = 1 << 1;
        /// On dialogue events override: FontSize, Spacing, Blur, ScaleX, ScaleY
        const FONT_SIZE_FIELDS = 1 << 2;
        /// On dialogue events override: FontName, treat_fontname_as_pattern
        const FONT_NAME = 1 << 3;
        /// On dialogue events override: PrimaryColour, SecondaryColour, OutlineColour, BackColour
        const COLORS = 1 << 4;
        /// On dialogue events override: Bold, Italic, Underline, StrikeOut
        const ATTRIBUTES = 1 << 5;
        /// On dialogue events override: BorderStyle, Outline, Shadow
        const BORDER = 1 << 6;
        /// On dialogue events override: Alignment
        const ALIGNMENT = 1 << 7;
        /// On dialogue events override: MarginL, MarginR, MarginV
        const MARGINS = 1 << 8;
        /// Unconditionally replace all fields of all styles with the one provided with
        /// `Rederer::set_selective_style_override()`.
        ///
        /// Does not apply `SELECTIVE_FONT_SCALE`.
        ///
        /// Add ASS_OVERRIDE_BIT_FONT_SIZE_FIELDS and ASS_OVERRIDE_BIT_BORDER if you want FontSize,
        /// Spacing, Outline, Shadow to be scaled to the script resolution given by the ASS_Track.
        const FULL_STYLE = 1 << 9;
        /// On dialogue events override: Justify
        const JUSTIFY = 1 << 10;
    }
}

/// Errors for leaking paths to create pointers.
#[derive(Error, Debug, PartialEq)]
pub enum PathErr {
    /// Returned if there is a zero value in the middle of the path.
    #[error("{0}")]
    NullInPath(#[from] std::ffi::NulError),
    /// Returned if it's not UTF-8
    /// Because I don't want to deal with non-UTF-8 for now.
    #[error("Invalid UTF-8 found in OsString \"{0:?}\"")]
    NotUtf8(OsString),
}

/// Leaks a path an returns a pointer to it.
/// Must be UTF-8 (at least for now) because heck dealing with that.
fn path_to_ptr(path: PathBuf) -> Result<*const i8, PathErr> {
    match path.into_os_string().into_string() {
        Ok(utf) => {
            let to_leak = CString::new(utf)?;
            let out = to_leak.as_ptr();
            let _ = ManuallyDrop::new(to_leak);
            Ok(out)
        }
        Err(osstr) => Err(PathErr::NotUtf8(osstr)),
    }
}

/// The configuration parameters that are required to get a working `Renderer`.
///
/// There are other parameters that can be configured as well, but they are optionally configured.
/// see the methods on `Renderer`.
#[derive(Debug, Clone, PartialEq)]
pub struct RendererConfig {
    /// Set the frame size in pixels, including margins.
    /// The renderer will never return images that are outside the frame area.
    ///
    /// The value set with this function can influence pixel aspect ratio that is used for
    /// rendering.
    ///
    /// After compensating for configured margins the frame size is not isotropically scaled
    /// version of the video display size. For may have to `Track::set_pixel_aspect`
    pub frame_width: i32,
    #[allow(missing_docs, clippy::missing_docs_in_private_items)]
    pub frame_height: i32,
    /// Set the source image size in pixels.
    ///
    /// This affects some ASS tags like e.g. 3D transforms and is used to calculate the source
    /// aspect ratio and blur scale. If subtitles specify valid LayoutRes* headers, those will take
    /// precedence.
    ///
    /// The source image size can be reset to default by setting w and h to 0.
    ///
    /// The value set can influence pixel aspect ratio used for rendering.
    ///
    /// The values must be the actual storage size of the video stream, without any anamorphic
    /// de-squeeze applied.
    pub storage_width: i32,
    #[allow(missing_docs, clippy::missing_docs_in_private_items)]
    pub storage_height: i32,
    /// default_font path to default font to use. Must be supplied if all system fontproviders
    /// are disabled or unavailable.
    pub default_font: Option<PathBuf>,
    /// Optional default font family to use.
    pub default_font_family: Option<PathBuf>,
    /// Default font provider to use.
    ///
    /// If the requested font provider doesn't exist or fails to initialize, then Libass will
    /// behave as if `FontProvider::None` was passed.
    pub default_font_provider: FontProvider,
    /// Path the the fontconfig configuration file. Only relevant if fontconfig is used. The
    /// encoding must match whatever is accepted by fontconfig.
    ///
    /// Usually None.
    pub fontconfig_path: Option<PathBuf>,
    /// Whether the fontconfig cache should be updated/built now. Only relevant if fontconfig is
    /// used.
    pub update_fontconfig: bool,
}

/// Text shaping levels used by the renderer.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
#[non_exhaustive]
#[repr(i32)]
pub enum ShapingLevel {
    /// Fast, font-agnostic shaper that can do only substitutions.
    Simple = libass_sys::ASS_ShapingLevel::ASS_SHAPING_SIMPLE,
    #[default]
    /// Slower shaper using OpenType for substitutions and positioning.
    Complex = libass_sys::ASS_ShapingLevel::ASS_SHAPING_COMPLEX,
}

/// Font hinting type.
///
/// For use with Renderer::set_font_hinting
/// Using other variants than None may break subtitles or vsfilter scripts.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
#[non_exhaustive]
#[repr(i32)]
#[allow(missing_docs)]
pub enum FontHinting {
    #[default]
    None = libass_sys::ASS_Hinting::ASS_HINTING_NONE,
    Light = libass_sys::ASS_Hinting::ASS_HINTING_LIGHT,
    Normal = libass_sys::ASS_Hinting::ASS_HINTING_NORMAL,
    Native = libass_sys::ASS_Hinting::ASS_HINTING_NATIVE,
}

/// Describes how new images differ from the previous ones.
#[derive(Debug, Copy, Clone, PartialEq)]
#[non_exhaustive]
#[repr(i32)]
pub enum ChangeDetection {
    /// There is no change.
    Identical = 0,
    /// The content is the same, but they are in different position.
    DifferentPositions = 1,
    /// The entire content is different.
    DifferentContent = 2,
}

/// Conversion from i32 to value failed.
#[derive(Error, Debug)]
#[error("Failed to convert from int to {0}. Invalid value of {1} found instead.")]
pub struct FromIntError(String, i32);

impl TryFrom<i32> for ChangeDetection {
    type Error = FromIntError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use ChangeDetection::*;
        match value {
            0 => Ok(Identical),
            1 => Ok(DifferentPositions),
            2 => Ok(DifferentContent),
            val => Err(FromIntError("ChangeDetection".to_string(), val)),
        }
    }
}
