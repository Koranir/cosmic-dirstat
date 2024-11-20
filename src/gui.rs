use std::{ffi::OsString, path::PathBuf, sync::Arc};

mod partition_view;

use cosmic::{
    iced::{alignment::Horizontal, Color, Length, Point},
    iced_widget::scrollable,
    widget::{self, container, grid},
};

pub fn run() {
    cosmic::app::run::<App>(cosmic::app::Settings::default().transparent(true), ()).unwrap();
}

#[derive(Debug, Clone)]
enum Msg {
    CrawlPathChanged(PathBuf),
    CrawlPath { cancel: bool },
    CrawlPathDialogue,
    Crawl(PathBuf),
    ExtensionLegendChanged(Vec<(OsString, Color)>),
    PaneResize(cosmic::widget::pane_grid::ResizeEvent),
    Analyzed(Arc<crate::analyze::AnalyzedDir>),
    AnalyzedError(String),
    ClearError,
    NewItemHighlighted(Option<(Point, String, u64, PathBuf)>),
}

enum Panels {
    NamePath,
    Tree,
    Partioned,
}

struct App {
    core: cosmic::app::Core,
    crawl_path: PathBuf,
    crawling_path: bool,
    state: cosmic::widget::pane_grid::State<Panels>,
    analyzed: Option<Arc<crate::analyze::AnalyzedDir>>,
    error: Option<String>,
    extensions_ordered: Vec<(OsString, Color)>,
    highlighted: Option<(Point, String, u64, PathBuf)>,
}
impl App {
    pub fn tree_view(&self) -> cosmic::Element<Msg> {
        use cosmic::widget::{column, text};

        let heading = text::heading("Legend");

        let mut grid = grid();
        for (name, col) in self.extensions_ordered.iter() {
            let name = name.to_string_lossy().into_owned();
            let col = *col;
            let name = text(name);
            let col = container(widget::Space::new(10.0, 10.0)).class(
                cosmic::theme::Container::custom(move |theme| {
                    container::Style {
                        background: Some(col.into()),
                        ..cosmic::widget::container::Catalog::style(
                            theme,
                            &cosmic::theme::Container::Card,
                        )
                    }
                    .border(cosmic::iced::border::rounded(2.))
                }),
            );
            // .class(cosmic::widget::container::Style::default().background(col));
            grid = grid.push(col).push(name).insert_row();
        }
        let legend = scrollable(grid.row_alignment(cosmic::iced::Alignment::Center));
        column::Column::with_children(vec![heading.into(), legend.into()])
            .padding(10.0)
            .into()
    }

    pub fn partition_view(&self) -> cosmic::Element<Msg> {
        use cosmic::widget::{button, column, container, icon, row, text};

        let heading_text = text::heading(format!(
            "Directory{}{}",
            if self.analyzed.is_some() { " - " } else { "" },
            self.analyzed
                .as_ref()
                .map(|f| f.path.to_string_lossy())
                .unwrap_or_default()
        ))
        .width(Length::FillPortion(2));
        let go_up_button = button::icon(icon::from_name("go-up-symbolic").handle()).on_press_maybe(
            self.analyzed
                .as_ref()
                .and_then(|f| f.path.parent().map(std::borrow::ToOwned::to_owned))
                .map(Msg::Crawl),
        );
        let go_up_button = container(go_up_button).align_x(Horizontal::Right);
        let heading = row::with_children(vec![heading_text.into(), go_up_button.into()]);
        let d = match &self.analyzed {
            Some(d) => cosmic::widget::tooltip(
                partition_view::PartitionView::new(
                    d,
                    8.0,
                    8.0 * 8.0,
                    Msg::Crawl,
                    Msg::ExtensionLegendChanged,
                    Msg::NewItemHighlighted,
                ),
                match self.highlighted.as_ref() {
                    Some(s) => cosmic::widget::column()
                        .push(cosmic::widget::text(s.1.as_str()))
                        .push(cosmic::widget::text(humansize::format_size(
                            s.2,
                            humansize::DECIMAL,
                        )))
                        .push(cosmic::widget::text(s.3.to_string_lossy()))
                        .into(),
                    None => cosmic::iced::Element::new(cosmic::widget::Space::with_width(
                        cosmic::iced::Length::Shrink,
                    )),
                },
                widget::tooltip::Position::FollowCursor,
            )
            .class(cosmic::theme::Container::Card)
            .into(),
            None => text("No Directory Analyzed").into(),
        };

        column::with_children(vec![heading.into(), d])
            .padding(10.0)
            .into()
    }

    pub fn path_and_title(&self) -> cosmic::Element<Msg> {
        use cosmic::widget::{button, column, container, icon, row, text, text_input};

        let title = text::title1("COSMIC DirStat");
        let sub = text::caption(concat!("v", env!("CARGO_PKG_VERSION")));

        let title_box = container(
            column::with_children(vec![title.into(), sub.into()])
                .align_x(cosmic::iced::Alignment::End)
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .align_x(Horizontal::Right);

        let path_input = text_input("path/to/analyzed/dir", self.crawl_path.to_string_lossy())
            .on_input(|f| Msg::CrawlPathChanged(PathBuf::from(f)));
        let submit_button = button::standard(if !self.crawling_path {
            "Scan"
        } else {
            "Cancel"
        })
        .on_press(Msg::CrawlPath {
            cancel: self.crawling_path,
        });

        let open_folder = button::icon(icon::from_name("folder-open-symbolic").handle())
            .on_press(Msg::CrawlPathDialogue);

        let path_input = row::with_children(vec![path_input.into(), open_folder.into()])
            .spacing(5.0)
            .align_y(cosmic::iced::Alignment::Center);

        let input_box =
            column::with_children(vec![path_input.into(), submit_button.into()]).spacing(5.0);

        column::with_children(vec![title_box.into(), input_box.into()])
            .padding(10.0)
            .into()
    }
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Msg;

    const APP_ID: &'static str = "com.koranir.CosmicDirStat";

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn init(
        mut core: cosmic::app::Core,
        _flags: Self::Flags,
    ) -> (Self, cosmic::app::Task<Self::Message>) {
        let (mut state, tree_panel) = cosmic::widget::pane_grid::State::new(Panels::Tree);
        let (_partitioned_panel, header_partitioned_split) = state
            .split(
                widget::pane_grid::Axis::Horizontal,
                tree_panel,
                Panels::Partioned,
            )
            .unwrap();
        state.resize(header_partitioned_split, 0.33);
        let (_name_path_panel, name_path_tree_split) = state
            .split(
                widget::pane_grid::Axis::Vertical,
                tree_panel,
                Panels::NamePath,
            )
            .unwrap();
        state.resize(name_path_tree_split, 0.4);

        core.set_header_title("COSMIC DirStat".into());

        let app = Self {
            core,
            crawl_path: PathBuf::new(),
            crawling_path: false,
            state,
            analyzed: None,
            error: None,
            extensions_ordered: Vec::new(),
            highlighted: None,
        };

        (app, cosmic::Task::none())
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Msg::CrawlPathChanged(s) => {
                self.crawl_path = s;
                self.core.set_header_title(format!(
                    "COSMIC DirStat - {}",
                    self.crawl_path.to_string_lossy().into_owned()
                ));
            }
            Msg::Crawl(s) => {
                self.crawl_path = s.clone();
                self.core.set_header_title(format!(
                    "COSMIC DirStat - {}",
                    self.crawl_path.to_string_lossy().into_owned()
                ));
                return cosmic::Task::perform(
                    async move { crate::analyze::analyze_dir(&s, &crate::analyze::Context {}) },
                    |a| {
                        match a {
                            Ok(a) => Msg::Analyzed(Arc::new(a)),
                            Err(e) => Msg::AnalyzedError(e.to_string()),
                        }
                        .into()
                    },
                );
            }
            Msg::CrawlPath { cancel } => {
                if !cancel {
                    self.crawling_path = true;
                    let crawl_path = self.crawl_path.clone();

                    return self.update(Msg::Crawl(crawl_path));
                }
                self.crawling_path = false;
            }
            Msg::CrawlPathDialogue => {
                return cosmic::Task::perform(
                    rfd::AsyncFileDialog::new().pick_folder(),
                    |f| match f {
                        Some(f) => Msg::CrawlPathChanged(f.path().to_path_buf()).into(),
                        None => cosmic::app::Message::None,
                    },
                );
            }
            Msg::PaneResize(f) => self.state.resize(f.split, f.ratio),
            Msg::Analyzed(a) => {
                self.crawling_path = false;
                self.analyzed = Some(a);
            }
            Msg::AnalyzedError(e) => {
                self.crawling_path = false;
                self.error = Some(e);
            }
            Msg::ClearError => self.error = None,
            Msg::ExtensionLegendChanged(l) => self.extensions_ordered = l,
            Msg::NewItemHighlighted(h) => match h {
                Some(s) => self.highlighted = Some(s),
                None => self.highlighted = None,
            },
        }

        cosmic::Task::none()
    }

    fn dialog(&self) -> Option<cosmic::Element<Self::Message>> {
        self.error.as_ref().map(|e| {
            cosmic::widget::dialog()
                .title(format!("Error: {e}"))
                .primary_action(cosmic::widget::button::standard("OK").on_press(Msg::ClearError))
                .into()
        })
    }

    fn view(&self) -> cosmic::Element<Self::Message> {
        use cosmic::widget::container;

        let grid =
            cosmic::widget::pane_grid::PaneGrid::new(&self.state, move |_pane, t, _maximized| {
                match t {
                    Panels::NamePath => container(self.path_and_title())
                        .class(cosmic::theme::Container::Card)
                        .height(Length::FillPortion(2))
                        .width(Length::FillPortion(1))
                        .into(),
                    Panels::Tree => container(self.tree_view())
                        .class(cosmic::theme::Container::Card)
                        .height(Length::FillPortion(2))
                        .width(Length::FillPortion(2))
                        .into(),
                    Panels::Partioned => container(self.partition_view())
                        .class(cosmic::theme::Container::Card)
                        .height(Length::FillPortion(3))
                        .width(Length::Fill)
                        .into(),
                }
            })
            .on_resize(10.0, Msg::PaneResize)
            .spacing(10.0);

        grid.into()
    }
}
