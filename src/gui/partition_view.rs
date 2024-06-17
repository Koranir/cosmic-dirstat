use std::{path::PathBuf, sync::atomic::AtomicUsize};

use cosmic::{
    cosmic_theme::palette::{Darken, FromColor, Okhsl, ShiftHue},
    iced::{
        mouse::Button, Background, Border, Color, Length, Point, Radius, Rectangle, Size, Vector,
    },
    iced_core::{layout, text, Layout, Renderer, Shadow},
    prelude::ColorExt,
    widget::Widget,
};
use treemap::Mappable;

use crate::analyze::{self, AnalyzedDir};

pub enum StateBoxD {
    Branched(Vec<StateBox>),
    Leaf,
}

pub struct StateBox {
    d: StateBoxD,
    placement: treemap::Rect,
    size: u64,
    name: String,
    analyzed_item: Option<analyze::AnalyzedItem>,
    idx: usize,
}
impl StateBox {
    pub fn recurse_find(&self, at: (f32, f32), p: (f32, f32)) -> Option<(&Self, Option<&Self>)> {
        let bounds = self.placement;

        let quad_bounds = Rectangle::new(
            Point::new(bounds.x as f32 + at.0, bounds.y as f32 + at.1),
            Size::new(bounds.w as f32, bounds.h as f32),
        );

        if quad_bounds.contains(Point::new(p.0, p.1)) {
            if let StateBoxD::Branched(d) = &self.d {
                for ele in d {
                    if let Some(p) = ele.recurse_find((quad_bounds.x, quad_bounds.y), p) {
                        return Some((p.0, Some(p.1.unwrap_or(self))));
                    }
                }
            }
            Some((self, None))
        } else {
            None
        }
    }

    pub fn draw<R: Renderer + cosmic::iced_core::text::Renderer>(
        &self,
        at: (f32, f32),
        renderer: &mut R,
        level: usize,
        to_highlight: usize,
        text_size: f32,
    ) -> Option<cosmic::iced_core::renderer::Quad> {
        let bounds = self.placement;

        // let level = if self.idx == to_highlight { 0 } else { level };
        let cols = [
            0.0, 0.33, 0.66, 0.25, 0.75, 0.1, 0.5, 0.9, 0.0, 0.33, 0.66, 0.25, 0.75, 0.1, 0.5, 0.9,
            0.0, 0.33, 0.66, 0.25, 0.75, 0.1, 0.5, 0.9, 0.0, 0.33, 0.66, 0.25, 0.75, 0.1, 0.5, 0.9,
        ]
        .map(|f| {
            let base_col = cosmic::theme::active().cosmic().accent.base;
            let new = ShiftHue::shift_hue(Okhsl::from_color(base_col.color), f * 360.0).darken(0.5);
            let rgba = cosmic::cosmic_theme::palette::Srgb::from_color(new);
            cosmic::iced::Color::from_linear_rgba(rgba.red, rgba.green, rgba.blue, 1.0)
        });

        let quad_bounds = Rectangle::new(
            Point::new(bounds.x as f32 + at.0, bounds.y as f32 + at.1),
            Size::new(bounds.w as f32, bounds.h as f32),
        );

        renderer.fill_quad(
            cosmic::iced_core::renderer::Quad {
                bounds: quad_bounds,
                border: Border::default(),
                shadow: Default::default(),
            },
            Background::Gradient(cosmic::iced::Gradient::Linear(
                cosmic::iced::gradient::Linear::new(std::f32::consts::PI / 4.0)
                    .add_stop(0.0, cols[level])
                    .add_stop(1.0, cols[level].blend_alpha(Color::BLACK, 0.5)),
            )),
        );

        let mut maybe_highlight = None;
        if let StateBoxD::Branched(d) = &self.d {
            if quad_bounds.height > text_size {
                let mut bounds = quad_bounds.size();
                bounds.height = text_size;
                renderer.fill_text(
                    cosmic::iced_core::text::Text {
                        content: &self.name,
                        bounds,
                        size: text_size.into(),
                        font: renderer.default_font(),
                        horizontal_alignment: cosmic::iced::alignment::Horizontal::Left,
                        vertical_alignment: cosmic::iced::alignment::Vertical::Top,
                        line_height: text::LineHeight::default(),
                        shaping: text::Shaping::Advanced,
                    },
                    Point::new(
                        quad_bounds.x + 1.0,
                        quad_bounds.y + 1.0, /* + text_size / 2.0*/
                    ),
                    Color::BLACK,
                    quad_bounds,
                );
                renderer.fill_text(
                    cosmic::iced_core::text::Text {
                        content: &self.name,
                        bounds,
                        size: text_size.into(),
                        font: renderer.default_font(),
                        horizontal_alignment: cosmic::iced::alignment::Horizontal::Left,
                        vertical_alignment: cosmic::iced::alignment::Vertical::Top,
                        line_height: text::LineHeight::default(),
                        shaping: text::Shaping::Advanced,
                    },
                    Point::new(quad_bounds.x, quad_bounds.y /* + text_size / 2.0*/),
                    Color::WHITE.blend_alpha(Color::BLACK, 0.8),
                    quad_bounds,
                );
            }

            for ele in d {
                if let Some(r) = ele.draw(
                    (quad_bounds.x, quad_bounds.y),
                    renderer,
                    level + 1,
                    to_highlight,
                    text_size,
                ) {
                    maybe_highlight = Some(r);
                }
            }
        }

        if self.idx == to_highlight {
            maybe_highlight = Some(cosmic::iced_core::renderer::Quad {
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
            });
        }

        maybe_highlight
    }
}

pub struct State {
    boxes: Vec<StateBox>,
    highlighted: usize,
    highlighted_popup: Option<(Point, String, u64, PathBuf)>,
}

pub struct PartitionView<'a, Msg> {
    items: &'a AnalyzedDir,
    text_size: f32,
    minimum_area: f32,
    on_click: Box<dyn FnMut(PathBuf) -> Msg>,
}
impl<'a, Msg> PartitionView<'a, Msg> {
    pub fn new(
        items: &'a AnalyzedDir,
        text_size: f32,
        minimum_area: f32,
        on_click: impl FnMut(PathBuf) -> Msg + 'static,
    ) -> Self {
        Self {
            items,
            text_size,
            minimum_area,
            on_click: Box::new(on_click),
        }
    }
}
impl<
        'a,
        Message,
        Theme,
        Renderer: cosmic::iced_core::Renderer + cosmic::iced_core::text::Renderer,
    > Widget<Message, Theme, Renderer> for PartitionView<'a, Message>
{
    fn state(&self) -> cosmic::iced_core::widget::tree::State {
        cosmic::iced_core::widget::tree::State::Some(Box::new(State {
            boxes: vec![],
            highlighted: usize::MAX,
            highlighted_popup: None,
        }))
    }

    fn size(&self) -> cosmic::iced::Size<cosmic::iced::Length> {
        cosmic::iced::Size {
            width: cosmic::iced::Length::Fill,
            height: cosmic::iced::Length::Fill,
        }
    }

    fn layout(
        &self,
        tree: &mut cosmic::iced_core::widget::Tree,
        _renderer: &Renderer,
        limits: &cosmic::iced_core::layout::Limits,
    ) -> cosmic::iced_core::layout::Node {
        let layout = layout::atomic(limits, Length::Fill, Length::Fill);

        let state: &mut State = tree.state.downcast_mut();

        fn recursive_box(
            space: (f64, f64),
            min: f64,
            dir: &AnalyzedDir,
            text_offset: f64,
            text_size: f32,
        ) -> Vec<StateBox> {
            static IDX: AtomicUsize = AtomicUsize::new(0);

            if space.1 < text_offset * 1.4 {
                return vec![];
            }

            let partitioned =
                analyze::partition((space.0, text_offset.mul_add(-1.4, space.1)), min, dir);

            partitioned
                .into_iter()
                .map(|mut item| {
                    let mut bounds_ = *item.bounds();
                    bounds_.y += text_offset * 1.4;
                    item.set_bounds(bounds_);
                    // dbg!(opt_dir);
                    let d = match item.item {
                        Some(analyze::AnalyzedItem::Dir(d)) => StateBoxD::Branched(recursive_box(
                            (item.bounds().w, item.bounds().h),
                            min,
                            d,
                            text_offset,
                            text_size,
                        )),
                        _ => StateBoxD::Leaf,
                    };
                    StateBox {
                        d,
                        name: item.item.map_or("<files>".into(), |f| {
                            f.name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        }),
                        idx: IDX.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                        analyzed_item: item.item.cloned(),
                        placement: item.placement,
                        size: item.size,
                    }
                })
                .collect()
        }

        state.boxes = recursive_box(
            (
                f64::from(layout.bounds().width),
                f64::from(layout.bounds().height),
            ),
            f64::from(self.minimum_area),
            self.items,
            f64::from(self.text_size),
            self.text_size,
        );

        layout
    }

    fn on_event(
        &mut self,
        state: &mut cosmic::iced_core::widget::Tree,
        event: cosmic::iced::Event,
        layout: Layout<'_>,
        cursor: cosmic::iced_core::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn cosmic::iced_core::Clipboard,
        shell: &mut cosmic::iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> cosmic::iced_core::event::Status {
        let state: &mut State = state.state.downcast_mut();

        if let cosmic::iced::Event::Mouse(mev) = event {
            let pos = cursor.position().unwrap_or_default();

            let highlighted = state.boxes.iter().find_map(|b| {
                b.recurse_find((layout.bounds().x, layout.bounds().y), (pos.x, pos.y))
            });
            match mev {
                cosmic::iced::mouse::Event::CursorMoved { position: _ } => {
                    state.highlighted_popup = highlighted.map(|(f, _)| {
                        (
                            pos,
                            f.name.clone(),
                            f.size,
                            f.analyzed_item
                                .as_ref()
                                .map(|f| f.path().to_owned())
                                .unwrap_or_default(),
                        )
                    });
                    state.highlighted = highlighted.map_or(usize::MAX, |(f, _)| f.idx);
                }
                cosmic::iced::mouse::Event::ButtonPressed(Button::Left) => {
                    if let Some((f, parent)) = highlighted {
                        shell.publish((self.on_click)(
                            f.analyzed_item
                                .as_ref()
                                .map(|f| f.path().to_owned())
                                .unwrap_or_else(|| {
                                    parent
                                        .map(|f| {
                                            f.analyzed_item.as_ref().unwrap().path().to_owned()
                                        })
                                        .unwrap_or_else(|| {
                                            f.analyzed_item.as_ref().unwrap().path().to_owned()
                                        })
                                }),
                        ));
                    }
                }
                _ => {}
            }
        }

        cosmic::iced_core::event::Status::Ignored
    }

    fn draw(
        &self,
        tree: &cosmic::iced_core::widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &cosmic::iced_core::renderer::Style,
        layout: cosmic::iced_core::Layout<'_>,
        _cursor: cosmic::iced_core::mouse::Cursor,
        _viewport: &cosmic::iced::Rectangle,
    ) {
        let state: &State = tree.state.downcast_ref();

        let mut highlight = None;
        for ele in &state.boxes {
            if let Some(r) = ele.draw(
                (layout.bounds().x, layout.bounds().y),
                renderer,
                0,
                state.highlighted,
                self.text_size,
            ) {
                highlight = Some(r);
            }
        }
        if let Some(r) = highlight {
            renderer.fill_quad(r, Background::Color(Color::TRANSPARENT));
        }
    }

    fn overlay<'b>(
        &mut self,
        _state: &mut cosmic::iced_core::widget::Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
    ) -> Option<cosmic::iced_core::overlay::Element<Message, Theme, Renderer>> {
        let state: &mut State = _state.state.downcast_mut();

        state.highlighted_popup.clone().map(|f| {
            cosmic::iced_core::overlay::Element::new(
                f.0,
                Box::new(Overlay {
                    name: f.1,
                    size: f.2,
                    path: f.3,
                    text_size: self.text_size,
                }),
            )
        })
    }
}
impl<'a, Message: 'static> From<PartitionView<'a, Message>> for cosmic::Element<'a, Message> {
    fn from(value: PartitionView<'a, Message>) -> Self {
        Self::new(value)
    }
}

struct Overlay {
    // pos: Point,
    name: String,
    size: u64,
    path: PathBuf,
    text_size: f32,
}
impl<Message, Theme, Renderer: cosmic::iced_core::Renderer + cosmic::iced_core::text::Renderer>
    cosmic::iced_core::Overlay<Message, Theme, Renderer> for Overlay
{
    fn layout(
        &mut self,
        _renderer: &Renderer,
        bounds: Size,
        position: Point,
        _translation: Vector,
    ) -> layout::Node {
        let pos = position + Vector::new(400.0, 0.0);
        layout::Node::new(Size::new(-pos.x, pos.y).expand(bounds)).move_to(pos)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &cosmic::iced_core::renderer::Style,
        layout: Layout<'_>,
        _cursor: cosmic::iced_core::mouse::Cursor,
    ) {
        let bounds = Rectangle::new(
            layout.position()
                - Vector::new(
                    if layout.bounds().size().width > 0.0 || layout.position().x - 400.0 < 400.0 {
                        400.0
                    } else {
                        800.0
                    },
                    self.text_size * 3.0 * 1.4,
                ),
            Size::new(400.0, self.text_size * 3.0 * 1.4),
        );

        renderer.fill_quad(
            cosmic::iced_core::renderer::Quad {
                bounds,
                border: Border::with_radius(2),
                shadow: Shadow {
                    color: Color::BLACK,
                    offset: Vector::new(4.0, 4.0),
                    blur_radius: 6.0,
                },
                // shadow: Default::default(),
            },
            Background::Color(Color::BLACK.blend_alpha(Color::WHITE, 0.5)),
        );

        let string = format!(
            "{}\n{}\n{}",
            self.name,
            humansize::format_size(self.size, humansize::DECIMAL),
            self.path.display()
        );

        renderer.fill_text(
            cosmic::iced_core::text::Text {
                content: &string,
                bounds: bounds.size(),
                size: self.text_size.into(),
                font: renderer.default_font(),
                horizontal_alignment: cosmic::iced::alignment::Horizontal::Left,
                vertical_alignment: cosmic::iced::alignment::Vertical::Top,
                line_height: text::LineHeight::default(),
                shaping: text::Shaping::Advanced,
            },
            Point::new(bounds.x + 1.0, bounds.y + 1.0 /* + text_size / 2.0*/),
            Color::BLACK,
            bounds,
        );
        renderer.fill_text(
            cosmic::iced_core::text::Text {
                content: &string,
                bounds: bounds.size(),
                size: self.text_size.into(),
                font: renderer.default_font(),
                horizontal_alignment: cosmic::iced::alignment::Horizontal::Left,
                vertical_alignment: cosmic::iced::alignment::Vertical::Top,
                line_height: text::LineHeight::default(),
                shaping: text::Shaping::Advanced,
            },
            Point::new(bounds.x, bounds.y /* + text_size / 2.0*/),
            Color::WHITE.blend_alpha(Color::BLACK, 0.8),
            bounds,
        );
    }
}
