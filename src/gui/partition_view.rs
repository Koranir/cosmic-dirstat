use std::{collections::HashMap, ffi::OsString, path::PathBuf, sync::Arc};

use cosmic::{
    cosmic_theme::palette::{Darken, FromColor, Okhsl, ShiftHue},
    iced::{
        Background, Border, Color, Length, Limits, Point, Radius, Rectangle, Shadow, Size, Vector,
        advanced::{self, Layout, Renderer, layout, renderer::Quad, text},
        core::{
            Clipboard, Shell,
            layout::Node,
            widget::{Tree, tree},
        },
        mouse::{Button, Cursor},
    },
    prelude::ColorExt,
    widget::Widget,
};
use treemap::Mappable;

use crate::analyze::{self, AnalyzedDir, AnalyzedItem};

#[derive(Debug, Clone)]
pub enum StateBoxD {
    Branched(Vec<StateBox>),
    Leaf,
}

#[derive(Debug, Clone)]
pub struct StateBox {
    d: StateBoxD,
    placement: treemap::Rect,
    size: u64,
    name: String,
    extension: Option<OsString>,
    analyzed_item: Option<analyze::AnalyzedItem>,
    idx: usize,
}

#[derive(Debug, Clone, Copy)]
struct Scale {
    x: f32,
    y: f32,
}
impl Scale {
    const ONE: Self = Self { x: 1.0, y: 1.0 };

    fn from_sizes(source: Size<f32>, target: Size<f32>) -> Self {
        if source.width > 0.0 && source.height > 0.0 {
            Self {
                x: target.width / source.width,
                y: target.height / source.height,
            }
        } else {
            Self::ONE
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MapTransform {
    origin: Point,
    at: (f32, f32),
    scale: Scale,
}
impl MapTransform {
    fn root(origin: Point, scale: Scale) -> Self {
        Self {
            origin,
            at: (0.0, 0.0),
            scale,
        }
    }

    fn child(self, bounds: treemap::Rect) -> Self {
        Self {
            at: (self.at.0 + bounds.x as f32, self.at.1 + bounds.y as f32),
            ..self
        }
    }

    fn bounds(self, bounds: treemap::Rect) -> Rectangle {
        Rectangle::new(
            Point::new(
                self.origin.x + (bounds.x as f32 + self.at.0) * self.scale.x,
                self.origin.y + (bounds.y as f32 + self.at.1) * self.scale.y,
            ),
            Size::new(
                bounds.w as f32 * self.scale.x,
                bounds.h as f32 * self.scale.y,
            ),
        )
    }
}

impl StateBox {
    fn recurse_find(
        &self,
        transform: MapTransform,
        p: (f32, f32),
    ) -> Option<(&Self, Option<&Self>)> {
        let bounds = self.placement;

        let quad_bounds = transform.bounds(bounds);

        if quad_bounds.contains(Point::new(p.0, p.1)) {
            if let StateBoxD::Branched(d) = &self.d {
                for ele in d {
                    if let Some(p) = ele.recurse_find(transform.child(bounds), p) {
                        return Some((p.0, Some(p.1.unwrap_or(self))));
                    }
                }
            }
            Some((self, None))
        } else {
            None
        }
    }

    fn draw<R: Renderer + text::Renderer>(
        &self,
        transform: MapTransform,
        renderer: &mut R,
        // level: usize,
        to_highlight: usize,
        text_size: f32,
        colors: &HashMap<OsString, Color>,
    ) -> Option<Quad> {
        let bounds = self.placement;

        let quad_bounds = transform.bounds(bounds);

        let col = self
            .extension
            .as_ref()
            .and_then(|f| colors.get(f).copied())
            .unwrap_or(Color::from_rgb8(100, 100, 100));

        renderer.fill_quad(
            Quad {
                bounds: quad_bounds,
                border: Border::default(),
                shadow: Default::default(),
                snap: true,
            },
            Background::Gradient(cosmic::iced::Gradient::Linear(
                cosmic::iced::gradient::Linear::new(std::f32::consts::PI / 4.0)
                    .add_stop(0.0, col)
                    .add_stop(1.0, col.blend_alpha(Color::BLACK, 0.5)),
            )),
        );

        let mut maybe_highlight = None;
        if let StateBoxD::Branched(d) = &self.d {
            let scaled_text_size = text_size * transform.scale.y;
            if quad_bounds.height > scaled_text_size {
                let mut bounds = quad_bounds.size();
                bounds.height = scaled_text_size;
                // renderer.fill_text(
                //     cosmic::iced_core::text::Text {
                //         content: &self.name,
                //         bounds,
                //         size: text_size.into(),
                //         font: renderer.default_font(),
                //         horizontal_alignment: cosmic::iced::alignment::Horizontal::Left,
                //         vertical_alignment: cosmic::iced::alignment::Vertical::Top,
                //         line_height: text::LineHeight::default(),
                //         shaping: text::Shaping::Advanced,
                //         wrap: text::Wrap::WordOrGlyph,
                //     },
                //     Point::new(
                //         quad_bounds.x + 1.0,
                //         quad_bounds.y + 1.0, /* + text_size / 2.0*/
                //     ),
                //     Color::BLACK,
                //     quad_bounds,
                // );
                let f = format!(
                    "{} - {}",
                    &self.name,
                    humansize::format_size(self.size, humansize::FormatSizeOptions::default())
                );
                renderer.fill_text(
                    advanced::Text {
                        content: f,
                        bounds,
                        size: scaled_text_size.into(),
                        font: renderer.default_font(),
                        align_x: text::Alignment::Default,
                        align_y: cosmic::iced::alignment::Vertical::Top,
                        line_height: text::LineHeight::default(),
                        shaping: text::Shaping::Advanced,
                        wrapping: text::Wrapping::WordOrGlyph,
                        ellipsize: text::Ellipsize::Middle(text::EllipsizeHeightLimit::Lines(1)),
                    },
                    Point::new(quad_bounds.x, quad_bounds.y /* + text_size / 2.0*/),
                    Color::WHITE.blend_alpha(Color::BLACK, 0.8),
                    quad_bounds,
                );
            }

            for ele in d {
                if let Some(r) = ele.draw(
                    transform.child(bounds),
                    renderer,
                    // level + 1,
                    to_highlight,
                    text_size,
                    colors,
                ) {
                    maybe_highlight = Some(r);
                }
            }
        }

        if self.idx == to_highlight {
            maybe_highlight = Some(Quad {
                bounds: quad_bounds,
                border: Border {
                    color: Color::WHITE,
                    width: 1.0,
                    radius: Radius::default(),
                },
                shadow: Shadow {
                    color: Color::BLACK,
                    offset: Vector::new(0.0, 0.0),
                    blur_radius: 6.0,
                },
                snap: true,
            });
        }

        maybe_highlight
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartitionViewRebuild {
    size: Size<f32>,
    path: PathBuf,
    text_size: f32,
    minimum_area: f32,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct PartitionViewBuild {
    request: PartitionViewRebuild,
    boxes: Vec<StateBox>,
    ordered_extension_map: Vec<(OsString, Color)>,
    extension_map: HashMap<OsString, Color>,
}

#[derive(Debug, Clone)]
pub struct PartitionViewState {
    boxes: Vec<StateBox>,
    /// Extension -> Number of Files
    ordered_extension_map: Vec<(OsString, Color)>,
    extension_map: HashMap<OsString, Color>,
    constructed_for: Option<PartitionViewRebuild>,
    rebuild_in_flight: Option<PartitionViewRebuild>,
    queued_rebuild: Option<PartitionViewRebuild>,
    generation: u64,
}
impl PartitionViewState {
    pub fn new() -> Self {
        Self {
            boxes: Vec::new(),
            ordered_extension_map: Vec::new(),
            extension_map: HashMap::new(),
            constructed_for: None,
            rebuild_in_flight: None,
            queued_rebuild: None,
            generation: 0,
        }
    }

    pub fn clear(&mut self) {
        self.boxes.clear();
        self.ordered_extension_map.clear();
        self.extension_map.clear();
        self.constructed_for = None;
        self.rebuild_in_flight = None;
        self.queued_rebuild = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn rebuild_request(
        &self,
        size: Size<f32>,
        path: PathBuf,
        text_size: f32,
        minimum_area: f32,
    ) -> PartitionViewRebuild {
        PartitionViewRebuild {
            size,
            path,
            text_size,
            minimum_area,
            generation: self.generation,
        }
    }

    pub fn needs_rebuild(&self, request: &PartitionViewRebuild) -> bool {
        self.constructed_for.as_ref() != Some(request)
            && self.rebuild_in_flight.as_ref() != Some(request)
            && self.queued_rebuild.as_ref() != Some(request)
    }

    pub fn request_rebuild(
        &mut self,
        request: PartitionViewRebuild,
    ) -> Option<PartitionViewRebuild> {
        if request.generation != self.generation {
            return None;
        }

        if !self.needs_rebuild(&request) {
            return None;
        }

        if self.rebuild_in_flight.is_some() {
            self.queued_rebuild = Some(request);
            None
        } else {
            self.rebuild_in_flight = Some(request.clone());
            Some(request)
        }
    }

    pub fn finish_rebuild(
        &mut self,
        build: PartitionViewBuild,
    ) -> (bool, Option<PartitionViewRebuild>) {
        if self.rebuild_in_flight.as_ref() != Some(&build.request) {
            return (false, None);
        }

        self.boxes = build.boxes;
        self.ordered_extension_map = build.ordered_extension_map;
        self.extension_map = build.extension_map;
        self.constructed_for = Some(build.request);
        self.rebuild_in_flight = None;

        let next = self.queued_rebuild.take().and_then(|request| {
            if self.needs_rebuild(&request) {
                self.rebuild_in_flight = Some(request.clone());
                Some(request)
            } else {
                None
            }
        });

        (true, next)
    }

    pub fn ordered_extensions(&self) -> &[(OsString, Color)] {
        &self.ordered_extension_map
    }

    fn scale_for(&self, size: Size<f32>) -> Scale {
        self.constructed_for
            .as_ref()
            .map_or(Scale::ONE, |request| Scale::from_sizes(request.size, size))
    }

    pub fn build(
        request: PartitionViewRebuild,
        dir: Arc<AnalyzedDir>,
        base_col: cosmic::theme::CosmicColor,
    ) -> PartitionViewBuild {
        fn recursive_box(
            space: (f64, f64),
            min: f64,
            dir: &AnalyzedDir,
            text_offset: f64,
            extension_map: &mut HashMap<OsString, usize>,
            next_idx: &mut usize,
        ) -> Vec<StateBox> {
            if space.1 < text_offset * 1.4 {
                return vec![];
            }

            let partitioned =
                analyze::partition((space.0, text_offset.mul_add(-1.4, space.1)), min, dir);

            partitioned
                .into_iter()
                .map(|mut item| {
                    let mut bounds_ = *item.bounds();
                    bounds_.y = text_offset.mul_add(1.4, bounds_.y);
                    item.set_bounds(bounds_);
                    let d = match item.item {
                        Some(analyze::AnalyzedItem::Dir(d)) => StateBoxD::Branched(recursive_box(
                            (item.bounds().w, item.bounds().h),
                            min,
                            d,
                            text_offset,
                            extension_map,
                            next_idx,
                        )),
                        _ => StateBoxD::Leaf,
                    };

                    let ext = item.item.and_then(|f| {
                        if let AnalyzedItem::File(f) = f {
                            f.path.extension()
                        } else {
                            None
                        }
                    });
                    if let Some(ext) = ext {
                        if extension_map.contains_key(ext) {
                            *extension_map.get_mut(ext).unwrap() += item.size as usize;
                        } else {
                            extension_map.insert(ext.to_owned(), item.size as _);
                        }
                    }

                    let idx = *next_idx;
                    *next_idx += 1;

                    StateBox {
                        d,
                        name: item.item.map_or("<files>".into(), |f| {
                            f.name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        }),
                        idx,
                        analyzed_item: item.item.cloned(),
                        placement: item.placement,
                        size: item.size,
                        extension: ext.map(std::ffi::OsStr::to_os_string),
                    }
                })
                .collect()
        }

        let mut extension_map = HashMap::new();
        let mut next_idx = 0;
        let boxes = recursive_box(
            (
                f64::from(request.size.width),
                f64::from(request.size.height),
            ),
            f64::from(request.minimum_area),
            &dir,
            f64::from(request.text_size),
            &mut extension_map,
            &mut next_idx,
        );

        let len = extension_map.len();

        let cols = Vec::from_iter((0usize..).take(extension_map.len()).map(|f| {
            let shifted = (f as f32 * 1.618).rem_euclid(1.0);

            let new =
                ShiftHue::shift_hue(Okhsl::from_color(base_col.color), shifted * 360.0).darken(0.5);
            let rgba = cosmic::cosmic_theme::palette::Srgb::from_color(new);
            cosmic::iced::Color::from_linear_rgba(rgba.red, rgba.green, rgba.blue, 1.0)
        }));
        let mut ext = extension_map.into_iter().collect::<Vec<_>>();
        ext.sort_by_key(|f| f.1);

        let ordered_extension_map = ext
            .into_iter()
            .rev()
            .enumerate()
            .take(len)
            .map(|(index, f)| (f.0, cols[index]))
            .collect::<Vec<_>>();
        let extension_map = ordered_extension_map.clone().into_iter().collect();

        PartitionViewBuild {
            request,
            boxes,
            ordered_extension_map,
            extension_map,
        }
    }
}
impl Default for PartitionViewState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct State {
    highlighted: usize,
    rebuild_request: Option<PartitionViewRebuild>,
}

#[allow(clippy::type_complexity)]
pub struct PartitionView<'a, Msg> {
    items: &'a AnalyzedDir,
    state: &'a PartitionViewState,
    text_size: f32,
    minimum_area: f32,
    on_rebuild_needed: Box<dyn FnMut(PartitionViewRebuild) -> Msg>,
    on_click: Box<dyn FnMut(PathBuf) -> Msg>,
    on_item_hovered: Box<dyn FnMut(Option<(Point, String, u64, PathBuf)>) -> Msg>,
}
impl<'a, Msg> PartitionView<'a, Msg> {
    pub fn new(
        items: &'a AnalyzedDir,
        state: &'a PartitionViewState,
        text_size: f32,
        minimum_area: f32,
        on_rebuild_needed: impl FnMut(PartitionViewRebuild) -> Msg + 'static,
        on_click: impl FnMut(PathBuf) -> Msg + 'static,
        on_item_hovered: impl FnMut(Option<(Point, String, u64, PathBuf)>) -> Msg + 'static,
    ) -> Self {
        Self {
            items,
            state,
            text_size,
            minimum_area,
            on_rebuild_needed: Box::new(on_rebuild_needed),
            on_click: Box::new(on_click),
            on_item_hovered: Box::new(on_item_hovered),
        }
    }
}
impl<Message, Theme, Renderer: advanced::Renderer + text::Renderer> Widget<Message, Theme, Renderer>
    for PartitionView<'_, Message>
{
    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State {
            highlighted: usize::MAX,
            rebuild_request: None,
        }))
    }

    fn size(&self) -> cosmic::iced::Size<cosmic::iced::Length> {
        cosmic::iced::Size {
            width: cosmic::iced::Length::Fill,
            height: cosmic::iced::Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        let layout = layout::atomic(limits, Length::Fill, Length::Fill);

        let state: &mut State = tree.state.downcast_mut();

        let request = self.state.rebuild_request(
            layout.bounds().size(),
            self.items.path.clone(),
            self.text_size,
            self.minimum_area,
        );

        if self.state.needs_rebuild(&request) {
            state.rebuild_request = Some(request);
        } else {
            state.rebuild_request = None;
        }

        layout
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &cosmic::iced::Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state: &mut State = state.state.downcast_mut();

        if let Some(request) = state.rebuild_request.take() {
            shell.publish((self.on_rebuild_needed)(request));
            shell.request_redraw();
        }

        if let cosmic::iced::Event::Mouse(mev) = event {
            let pos = cursor.position().unwrap_or_default();
            let scale = self.state.scale_for(layout.bounds().size());
            let origin = Point::new(layout.bounds().x, layout.bounds().y);
            let transform = MapTransform::root(origin, scale);

            let highlighted = self
                .state
                .boxes
                .iter()
                .find_map(|b| b.recurse_find(transform, (pos.x, pos.y)));
            match mev {
                cosmic::iced::mouse::Event::CursorMoved { position: _ } => {
                    shell.publish((self.on_item_hovered)(highlighted.map(|(f, _)| {
                        (
                            pos,
                            f.name.clone(),
                            f.size,
                            f.analyzed_item
                                .as_ref()
                                .map(|f| f.path().to_owned())
                                .unwrap_or_default(),
                        )
                    })));
                    state.highlighted = highlighted.map_or(usize::MAX, |(f, _)| f.idx);
                }
                cosmic::iced::mouse::Event::ButtonPressed(Button::Left) => {
                    if let Some((f, parent)) = highlighted {
                        shell.publish((self.on_click)(f.analyzed_item.as_ref().map_or_else(
                            || {
                                parent.map_or_else(
                                    || f.analyzed_item.as_ref().unwrap().path().to_owned(),
                                    |f| f.analyzed_item.as_ref().unwrap().path().to_owned(),
                                )
                            },
                            |f| f.path().to_owned(),
                        )));
                    }
                }
                _ => {}
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &advanced::renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &cosmic::iced::Rectangle,
    ) {
        let state: &State = tree.state.downcast_ref();
        let scale = self.state.scale_for(layout.bounds().size());
        let origin = Point::new(layout.bounds().x, layout.bounds().y);
        let transform = MapTransform::root(origin, scale);

        let mut highlight = None;
        for ele in &self.state.boxes {
            if let Some(r) = ele.draw(
                transform,
                renderer,
                // 0,
                state.highlighted,
                self.text_size,
                &self.state.extension_map,
            ) {
                highlight = Some(r);
            }
        }
        if let Some(r) = highlight {
            renderer.fill_quad(r, Background::Color(Color::TRANSPARENT));
        }
    }
}
impl<'a, Message: 'static> From<PartitionView<'a, Message>> for cosmic::Element<'a, Message> {
    fn from(value: PartitionView<'a, Message>) -> Self {
        Self::new(value)
    }
}
