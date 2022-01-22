use std::collections::BTreeMap;

use crate::{
    mutex::{Arc, Mutex, MutexGuard},
    text::{
        font::{Font, FontImpl},
        Galley, LayoutJob,
    },
    TextureAtlas,
};

/// Font of unknown size.
///
/// Which style of font: [`Monospace`][`FontFamily::Monospace`], [`Proportional`][`FontFamily::Proportional`],
/// or by user-chosen name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FontFamily {
    /// A font where each character is the same width (`w` is the same width as `i`).
    Monospace,

    /// A font where some characters are wider than other (e.g. 'w' is wider than 'i').
    Proportional,

    /// ```
    /// // User-chosen names:
    /// FontFamily::Name("arial".into());
    /// FontFamily::Name("serif".into());
    /// ```
    Name(Arc<str>),
}

/// Alias for a font of known size.
///
/// One of a few categories of styles of text, e.g. body, button or heading.
/// Useful in GUI:s.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TextStyle {
    /// Used when small text is needed.
    Small,

    /// Normal labels. Easily readable, doesn't take up too much space.
    Body,

    /// Same size as [`Self::Body]`, but used when monospace is important (for aligning number, code snippets, etc).
    Monospace,

    /// Buttons. Maybe slightly bigger than [`Self::Body]`.
    /// Signifies that he item is interactive.
    Button,

    /// Heading. Probably larger than [`Self::Body]`.
    Heading,

    /// ```
    /// // A user-chosen name of a style:
    /// TextStyle::Name("footing".into());
    /// ````
    Name(Arc<str>),
}

impl TextStyle {
    /// All default (un-named) `TextStyle`:s.
    pub fn built_in() -> impl ExactSizeIterator<Item = TextStyle> {
        [
            TextStyle::Small,
            TextStyle::Body,
            TextStyle::Monospace,
            TextStyle::Button,
            TextStyle::Heading,
        ]
        .iter()
        .cloned()
    }
}

/// A `.ttf` or `.otf` file and a font face index.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FontData {
    /// The content of a `.ttf` or `.otf` file.
    pub font: std::borrow::Cow<'static, [u8]>,

    /// Which font face in the file to use.
    /// When in doubt, use `0`.
    pub index: u32,
}

impl FontData {
    pub fn from_static(font: &'static [u8]) -> Self {
        Self {
            font: std::borrow::Cow::Borrowed(font),
            index: 0,
        }
    }

    pub fn from_owned(font: Vec<u8>) -> Self {
        Self {
            font: std::borrow::Cow::Owned(font),
            index: 0,
        }
    }
}

fn ab_glyph_font_from_font_data(name: &str, data: &FontData) -> ab_glyph::FontArc {
    match &data.font {
        std::borrow::Cow::Borrowed(bytes) => {
            ab_glyph::FontRef::try_from_slice_and_index(bytes, data.index)
                .map(ab_glyph::FontArc::from)
        }
        std::borrow::Cow::Owned(bytes) => {
            ab_glyph::FontVec::try_from_vec_and_index(bytes.clone(), data.index)
                .map(ab_glyph::FontArc::from)
        }
    }
    .unwrap_or_else(|err| panic!("Error parsing {:?} TTF/OTF font file: {}", name, err))
}

/// Describes the font data and the sizes to use.
///
/// Often you would start with [`FontDefinitions::default()`] and then add/change the contents.
///
/// ```
/// # use {epaint::text::{FontDefinitions, TextStyle, FontFamily}};
/// # struct FakeEguiCtx {};
/// # impl FakeEguiCtx { fn set_fonts(&self, _: FontDefinitions) {} }
/// # let ctx = FakeEguiCtx {};
/// let mut fonts = FontDefinitions::default();
///
/// // Large button text:
/// fonts.styles.insert(
///     TextStyle::Button,
///     (FontFamily::Proportional, 32.0)
/// );
///
/// ctx.set_fonts(fonts);
/// ```
///
/// You can also install your own custom fonts:
/// ```
/// # use {epaint::text::{FontDefinitions, TextStyle, FontFamily, FontData}};
/// # struct FakeEguiCtx {};
/// # impl FakeEguiCtx { fn set_fonts(&self, _: FontDefinitions) {} }
/// # let ctx = FakeEguiCtx {};
/// let mut fonts = FontDefinitions::default();
///
/// // Install my own font (maybe supporting non-latin characters):
/// fonts.font_data.insert("my_font".to_owned(),
///    FontData::from_static(include_bytes!("../../fonts/Ubuntu-Light.ttf"))); // .ttf and .otf supported
///
/// // Put my font first (highest priority):
/// fonts.fonts_for_family.get_mut(&FontFamily::Proportional).unwrap()
///     .insert(0, "my_font".to_owned());
///
/// // Put my font as last fallback for monospace:
/// fonts.fonts_for_family.get_mut(&FontFamily::Monospace).unwrap()
///     .push("my_font".to_owned());
///
/// ctx.set_fonts(fonts);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct FontDefinitions {
    /// List of font names and their definitions.
    ///
    /// `epaint` has built-in-default for these, but you can override them if you like.
    pub font_data: BTreeMap<String, FontData>,

    /// Which fonts (names) to use for each [`FontFamily`].
    ///
    /// The list should be a list of keys into [`Self::font_data`].
    /// When looking for a character glyph `epaint` will start with
    /// the first font and then move to the second, and so on.
    /// So the first font is the primary, and then comes a list of fallbacks in order of priority.
    // TODO: per font size-modifier.
    pub fonts_for_family: BTreeMap<FontFamily, Vec<String>>,

    /// The [`FontFamily`] and size you want to use for a specific [`TextStyle`].
    pub styles: BTreeMap<TextStyle, (f32, FontFamily)>,
}

impl Default for FontDefinitions {
    fn default() -> Self {
        #[allow(unused)]
        let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();

        let mut fonts_for_family = BTreeMap::new();

        #[cfg(feature = "default_fonts")]
        {
            font_data.insert(
                "Hack".to_owned(),
                FontData::from_static(include_bytes!("../../fonts/Hack-Regular.ttf")),
            );
            font_data.insert(
                "Ubuntu-Light".to_owned(),
                FontData::from_static(include_bytes!("../../fonts/Ubuntu-Light.ttf")),
            );

            // Some good looking emojis. Use as first priority:
            font_data.insert(
                "NotoEmoji-Regular".to_owned(),
                FontData::from_static(include_bytes!("../../fonts/NotoEmoji-Regular.ttf")),
            );
            // Bigger emojis, and more. <http://jslegers.github.io/emoji-icon-font/>:
            font_data.insert(
                "emoji-icon-font".to_owned(),
                FontData::from_static(include_bytes!("../../fonts/emoji-icon-font.ttf")),
            );

            fonts_for_family.insert(
                FontFamily::Monospace,
                vec![
                    "Hack".to_owned(),
                    "Ubuntu-Light".to_owned(), // fallback for √ etc
                    "NotoEmoji-Regular".to_owned(),
                    "emoji-icon-font".to_owned(),
                ],
            );
            fonts_for_family.insert(
                FontFamily::Proportional,
                vec![
                    "Ubuntu-Light".to_owned(),
                    "NotoEmoji-Regular".to_owned(),
                    "emoji-icon-font".to_owned(),
                ],
            );
        }

        #[cfg(not(feature = "default_fonts"))]
        {
            fonts_for_family.insert(FontFamily::Monospace, vec![]);
            fonts_for_family.insert(FontFamily::Proportional, vec![]);
        }

        let mut styles = BTreeMap::new();
        styles.insert(TextStyle::Small, (10.0, FontFamily::Proportional));
        styles.insert(TextStyle::Body, (14.0, FontFamily::Proportional));
        styles.insert(TextStyle::Button, (14.0, FontFamily::Proportional));
        styles.insert(TextStyle::Heading, (20.0, FontFamily::Proportional));
        styles.insert(TextStyle::Monospace, (14.0, FontFamily::Monospace));

        Self {
            font_data,
            fonts_for_family,
            styles,
        }
    }
}

// ----------------------------------------------------------------------------

/// The collection of fonts used by `epaint`.
///
/// Required in order to paint text.
/// Create one and reuse. Cheap to clone.
///
/// Wrapper for `Arc<Mutex<FontsAndCache>>`.
pub struct Fonts(Arc<Mutex<FontsAndCache>>);

impl Fonts {
    /// Create a new [`Fonts`] for text layout.
    /// This call is expensive, so only create one [`Fonts`] and then reuse it.
    pub fn new(pixels_per_point: f32, definitions: FontDefinitions) -> Self {
        let fonts_and_cache = FontsAndCache {
            fonts: FontsImpl::new(pixels_per_point, definitions),
            galley_cache: Default::default(),
        };
        Self(Arc::new(Mutex::new(fonts_and_cache)))
    }

    /// Access the underlying [`FontsAndCache`].
    #[doc(hidden)]
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, FontsAndCache> {
        self.0.lock()
    }

    #[inline]
    pub fn pixels_per_point(&self) -> f32 {
        self.lock().fonts.pixels_per_point
    }

    /// Call each frame to get the change to the font texture since last call.
    pub fn font_image_delta(&self) -> Option<crate::ImageDelta> {
        self.lock().fonts.atlas.lock().take_delta()
    }

    /// Current size of the font image
    pub fn font_image_size(&self) -> [usize; 2] {
        self.lock().fonts.atlas.lock().size()
    }

    /// Width of this character in points.
    #[inline]
    pub fn glyph_width(&self, text_style: &TextStyle, c: char) -> f32 {
        self.lock().fonts.glyph_width(text_style, c)
    }

    /// Height of one row of text. In points
    #[inline]
    pub fn row_height(&self, text_style: &TextStyle) -> f32 {
        self.lock().fonts.row_height(text_style)
    }

    /// Layout some text.
    /// This is the most advanced layout function.
    /// See also [`Self::layout`], [`Self::layout_no_wrap`] and
    /// [`Self::layout_delayed_color`].
    ///
    /// The implementation uses memoization so repeated calls are cheap.
    /// #[inline]
    pub fn layout_job(&self, job: LayoutJob) -> Arc<Galley> {
        self.lock().layout_job(job)
    }

    pub fn num_galleys_in_cache(&self) -> usize {
        self.lock().galley_cache.num_galleys_in_cache()
    }

    /// Must be called once per frame to clear the [`Galley`] cache.
    pub fn end_frame(&self) {
        self.lock().galley_cache.end_frame();
    }

    /// Will wrap text at the given width and line break at `\n`.
    ///
    /// The implementation uses memoization so repeated calls are cheap.
    pub fn layout(
        &self,
        text: String,
        text_style: TextStyle,
        color: crate::Color32,
        wrap_width: f32,
    ) -> Arc<Galley> {
        let job = LayoutJob::simple(text, text_style, color, wrap_width);
        self.layout_job(job)
    }

    /// Will line break at `\n`.
    ///
    /// The implementation uses memoization so repeated calls are cheap.
    pub fn layout_no_wrap(
        &self,
        text: String,
        text_style: TextStyle,
        color: crate::Color32,
    ) -> Arc<Galley> {
        let job = LayoutJob::simple(text, text_style, color, f32::INFINITY);
        self.layout_job(job)
    }

    /// Like [`Self::layout`], made for when you want to pick a color for the text later.
    ///
    /// The implementation uses memoization so repeated calls are cheap.
    pub fn layout_delayed_color(
        &self,
        text: String,
        text_style: TextStyle,
        wrap_width: f32,
    ) -> Arc<Galley> {
        self.layout_job(LayoutJob::simple(
            text,
            text_style,
            crate::Color32::TEMPORARY_COLOR,
            wrap_width,
        ))
    }
}

// ----------------------------------------------------------------------------

pub struct FontsAndCache {
    pub fonts: FontsImpl,
    galley_cache: GalleyCache,
}

impl FontsAndCache {
    fn layout_job(&mut self, job: LayoutJob) -> Arc<Galley> {
        self.galley_cache.layout(&mut self.fonts, job)
    }
}

// ----------------------------------------------------------------------------

/// The collection of fonts used by `epaint`.
///
/// Required in order to paint text.
pub struct FontsImpl {
    pixels_per_point: f32,
    definitions: FontDefinitions,
    atlas: Arc<Mutex<TextureAtlas>>,
    font_impl_cache: FontImplCache,
    styles: BTreeMap<TextStyle, Font>,
}

impl FontsImpl {
    /// Create a new [`FontsImpl`] for text layout.
    /// This call is expensive, so only create one [`FontsImpl`] and then reuse it.
    pub fn new(pixels_per_point: f32, definitions: FontDefinitions) -> Self {
        assert!(
            0.0 < pixels_per_point && pixels_per_point < 100.0,
            "pixels_per_point out of range: {}",
            pixels_per_point
        );

        // We want an atlas big enough to be able to include all the Emojis in the `TextStyle::Heading`,
        // so we can show the Emoji picker demo window.
        let mut atlas = TextureAtlas::new([2048, 64]);

        {
            // Make the top left pixel fully white:
            let (pos, image) = atlas.allocate((1, 1));
            assert_eq!(pos, (0, 0));
            image[pos] = 255;
        }

        let atlas = Arc::new(Mutex::new(atlas));

        let font_impl_cache =
            FontImplCache::new(atlas.clone(), pixels_per_point, &definitions.font_data);

        let mut slf = Self {
            pixels_per_point,
            definitions,
            atlas,
            font_impl_cache,
            styles: Default::default(),
        };

        // pre-cache all styles:
        let styles: Vec<TextStyle> = slf.definitions.styles.keys().cloned().collect();
        for style in styles {
            slf.font_for_style(&style);
        }

        slf
    }

    #[inline(always)]
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    #[inline]
    pub fn definitions(&self) -> &FontDefinitions {
        &self.definitions
    }

    pub fn font_for_style(&mut self, text_style: &TextStyle) -> &mut Font {
        self.styles.entry(text_style.clone()).or_insert_with(|| {
            let (scale_in_points, family) = self
                .definitions
                .styles
                .get(text_style)
                .or_else(|| {
                    eprintln!(
                        "Missing TextStyle {:?} in font definitions; falling back to Body",
                        text_style
                    );
                    self.definitions.styles.get(&TextStyle::Body)
                })
                .unwrap_or_else(|| panic!("Missing {:?} in font definitions", TextStyle::Body));

            let fonts = &self.definitions.fonts_for_family.get(family);
            let fonts = fonts
                .unwrap_or_else(|| panic!("FontFamily::{:?} is not bound to any fonts", family));

            let fonts: Vec<Arc<FontImpl>> = fonts
                .iter()
                .map(|font_name| self.font_impl_cache.font_impl(*scale_in_points, font_name))
                .collect();

            Font::new(fonts)
        })
    }

    /// Width of this character in points.
    fn glyph_width(&mut self, text_style: &TextStyle, c: char) -> f32 {
        self.font_for_style(text_style).glyph_width(c)
    }

    /// Height of one row of text. In points
    fn row_height(&mut self, text_style: &TextStyle) -> f32 {
        self.font_for_style(text_style).row_height()
    }
}

// ----------------------------------------------------------------------------

struct CachedGalley {
    /// When it was last used
    last_used: u32,
    galley: Arc<Galley>,
}

#[derive(Default)]
struct GalleyCache {
    /// Frame counter used to do garbage collection on the cache
    generation: u32,
    cache: nohash_hasher::IntMap<u64, CachedGalley>,
}

impl GalleyCache {
    fn layout(&mut self, fonts: &mut FontsImpl, job: LayoutJob) -> Arc<Galley> {
        let hash = crate::util::hash(&job); // TODO: even faster hasher?

        match self.cache.entry(hash) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let cached = entry.into_mut();
                cached.last_used = self.generation;
                cached.galley.clone()
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let galley = super::layout(fonts, job.into());
                let galley = Arc::new(galley);
                entry.insert(CachedGalley {
                    last_used: self.generation,
                    galley: galley.clone(),
                });
                galley
            }
        }
    }

    pub fn num_galleys_in_cache(&self) -> usize {
        self.cache.len()
    }

    /// Must be called once per frame to clear the [`Galley`] cache.
    pub fn end_frame(&mut self) {
        let current_generation = self.generation;
        self.cache.retain(|_key, cached| {
            cached.last_used == current_generation // only keep those that were used this frame
        });
        self.generation = self.generation.wrapping_add(1);
    }
}

// ----------------------------------------------------------------------------

struct FontImplCache {
    atlas: Arc<Mutex<TextureAtlas>>,
    pixels_per_point: f32,
    ab_glyph_fonts: BTreeMap<String, ab_glyph::FontArc>,

    /// Map font pixel sizes and names to the cached `FontImpl`.
    cache: ahash::AHashMap<(u32, String), Arc<FontImpl>>,
}

impl FontImplCache {
    pub fn new(
        atlas: Arc<Mutex<TextureAtlas>>,
        pixels_per_point: f32,
        font_data: &BTreeMap<String, FontData>,
    ) -> Self {
        let ab_glyph_fonts = font_data
            .iter()
            .map(|(name, font_data)| (name.clone(), ab_glyph_font_from_font_data(name, font_data)))
            .collect();

        Self {
            atlas,
            pixels_per_point,
            ab_glyph_fonts,
            cache: Default::default(),
        }
    }

    pub fn font_impl(&mut self, scale_in_points: f32, font_name: &str) -> Arc<FontImpl> {
        let y_offset = if font_name == "emoji-icon-font" {
            scale_in_points * 0.235 // TODO: remove font alignment hack
        } else {
            0.0
        };
        let y_offset = y_offset - 3.0; // Tweaked to make text look centered in buttons and text edit fields

        let scale_in_points = if font_name == "emoji-icon-font" {
            scale_in_points * 0.8 // TODO: remove HACK!
        } else {
            scale_in_points
        };

        let scale_in_pixels = self.pixels_per_point * scale_in_points;

        // Round to an even number of physical pixels to get even kerning.
        // See https://github.com/emilk/egui/issues/382
        let scale_in_pixels = scale_in_pixels.round() as u32;

        self.cache
            .entry((scale_in_pixels, font_name.to_owned()))
            .or_insert_with(|| {
                let ab_glyph_font = self
                    .ab_glyph_fonts
                    .get(font_name)
                    .unwrap_or_else(|| panic!("No font data found for {:?}", font_name))
                    .clone();

                Arc::new(FontImpl::new(
                    self.atlas.clone(),
                    self.pixels_per_point,
                    ab_glyph_font,
                    scale_in_pixels,
                    y_offset,
                ))
            })
            .clone()
    }
}
