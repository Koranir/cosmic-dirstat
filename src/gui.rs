use std::{ffi::OsString, path::PathBuf, sync::Arc};

mod partition_view;
mod tree;

use cosmic::{
    Task,
    app::Settings,
    iced::widget::scrollable,
    iced::{Color, Length, Point, alignment::Horizontal},
    widget::{self, container, grid},
};

pub fn run() {
    cosmic::app::run::<App>(Settings::default(), ()).unwrap();
}

#[derive(Debug, Clone)]
enum Msg {
    ItemClicked(PathBuf),
    CrawlPathChanged(PathBuf),
    CrawlPath { cancel: bool },
    CrawlPathDialogue,
    Crawl(PathBuf),
    PartitionViewRebuildRequested(partition_view::PartitionViewRebuild),
    PartitionViewRebuilt(Arc<partition_view::PartitionViewBuild>),
    PaneResize(cosmic::widget::pane_grid::ResizeEvent),
    Analyzed(Arc<crate::analyze::AnalyzedDir>),
    AnalyzedError(String),
    ClearError,
    NewItemHighlighted(Option<(Point, String, u64, PathBuf)>),
    Tree(tree::Msg),
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
    tree: tree::FileTree,
    analyzed: Option<Arc<crate::analyze::AnalyzedDir>>,
    partition_view: partition_view::PartitionViewState,
    partition_rebuild_handle: Option<cosmic::iced::task::Handle>,
    error: Option<String>,
    extensions_ordered: Vec<(OsString, Color)>,
    highlighted: Option<(Point, String, u64, PathBuf)>,
}
impl App {
    fn partition_rebuild_task(
        &mut self,
        request: partition_view::PartitionViewRebuild,
    ) -> cosmic::app::Task<Msg> {
        let Some(analyzed) = self.analyzed.clone() else {
            return Task::none();
        };
        let base_col = cosmic::theme::active().cosmic().accent.base;

        if let Some(handle) = self.partition_rebuild_handle.take() {
            handle.abort();
        }

        let (task, handle) = cosmic::Task::perform(
            async move { partition_view::PartitionViewState::build(request, analyzed, base_col) },
            |build| Msg::PartitionViewRebuilt(Arc::new(build)).into(),
        )
        .abortable();
        self.partition_rebuild_handle = Some(handle);

        task
    }

    fn request_partition_rebuild(
        &mut self,
        request: partition_view::PartitionViewRebuild,
    ) -> cosmic::app::Task<Msg> {
        if let Some(request) = self.partition_view.request_rebuild(request) {
            self.partition_rebuild_task(request)
        } else {
            Task::none()
        }
    }

    #[allow(dead_code)]
    pub fn legend_view(&self) -> cosmic::Element<'_, Msg> {
        use cosmic::widget::{column, text};

        let heading = text::heading("Legend");

        let mut grid = grid();
        for (name, col) in &self.extensions_ordered {
            let name = name.to_string_lossy().into_owned();
            let col = *col;
            let name = text(name);
            let col = container(widget::Space::new().width(10.0).height(10.0)).class(
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

    pub fn partition_view(&self) -> cosmic::Element<'_, Msg> {
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
                    &self.partition_view,
                    8.0,
                    8.0 * 8.0,
                    Msg::PartitionViewRebuildRequested,
                    Msg::ItemClicked,
                    Msg::NewItemHighlighted,
                ),
                match self.highlighted.as_ref() {
                    Some(s) => cosmic::widget::column([
                        cosmic::widget::text(s.1.as_str()).into(),
                        cosmic::widget::text(humansize::format_size(s.2, humansize::DECIMAL))
                            .into(),
                        cosmic::widget::text(s.3.to_string_lossy()).into(),
                    ])
                    .into(),
                    None => cosmic::iced::Element::new(
                        cosmic::widget::Space::new().width(cosmic::iced::Length::Shrink),
                    ),
                },
                widget::tooltip::Position::FollowCursor,
            )
            .class(cosmic::theme::Container::Card)
            .into(),
            None => text("No Directory Analyzed, press 'Scan' to start.").into(),
        };

        column::with_children(vec![heading.into(), d])
            .padding(10.0)
            .into()
    }

    pub fn path_and_title(&self) -> cosmic::Element<'_, Msg> {
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
        let submit_button = button::standard(if self.crawling_path { "Cancel" } else { "Scan" })
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
        state.resize(name_path_tree_split, 0.66);

        core.set_header_title("COSMIC DirStat".into());

        let files_config = cosmic_files::config::Config::load().1;

        let cwd = std::env::current_dir().unwrap_or_default();

        let app = Self {
            core,
            crawl_path: cwd.clone(),
            crawling_path: false,
            state,
            analyzed: None,
            partition_view: partition_view::PartitionViewState::new(),
            partition_rebuild_handle: None,
            error: None,
            extensions_ordered: Vec::new(),
            highlighted: None,
            tree: tree::FileTree::new(cwd, files_config),
        };

        let task = app.tree.rescan();

        (app, task.map(Msg::Tree).map(cosmic::Action::App))
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Msg::ItemClicked(s) => {
                if s.is_dir() {
                    return self.update(Msg::CrawlPathChanged(s));
                } else {
                    if let Some(parent) = s.parent() {
                        return self.update(Msg::CrawlPathChanged(parent.into())).chain(
                            Task::done(cosmic::Action::App(Msg::Tree(tree::Msg::SelectPath(s)))),
                        );
                    }
                }
            }
            Msg::CrawlPathChanged(s) => {
                if s == self.crawl_path {
                    return Task::none();
                }

                self.crawl_path = s.clone();
                self.tree.change_location(s);
                return self.tree.rescan().map(Msg::Tree).map(cosmic::Action::App);
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
                return cosmic::Task::perform(rfd::AsyncFileDialog::new().pick_folder(), |f| {
                    f.map(|f| Msg::CrawlPathChanged(f.path().to_path_buf()).into())
                })
                .and_then(cosmic::app::Task::done);
            }
            Msg::PaneResize(f) => self.state.resize(f.split, f.ratio),
            Msg::Analyzed(a) => {
                self.crawling_path = false;
                self.analyzed = Some(a);
                self.partition_view.clear();
                if let Some(handle) = self.partition_rebuild_handle.take() {
                    handle.abort();
                }
                self.extensions_ordered.clear();
                self.highlighted = None;
            }
            Msg::AnalyzedError(e) => {
                self.crawling_path = false;
                self.error = Some(e);
            }
            Msg::ClearError => self.error = None,
            Msg::PartitionViewRebuildRequested(request) => {
                return self.request_partition_rebuild(request);
            }
            Msg::PartitionViewRebuilt(build) => {
                let build = Arc::try_unwrap(build).unwrap_or_else(|build| (*build).clone());
                let applied = self.partition_view.finish_rebuild(build);

                if applied {
                    self.partition_rebuild_handle = None;
                    self.extensions_ordered = self.partition_view.ordered_extensions().to_vec();
                }
            }
            Msg::NewItemHighlighted(h) => match h {
                Some(s) => self.highlighted = Some(s),
                None => self.highlighted = None,
            },
            Msg::Tree(message) => {
                let commands = self.tree.update(message);

                return Task::batch(commands.into_iter().filter_map(|c| {
                    match c {
                        cosmic_files::tab::Command::Iced(task) => Some(
                            task.0
                                .map(|t| cosmic::Action::App(Msg::Tree(tree::Msg::Tab(t)))),
                        ),
                        cosmic_files::tab::Command::ChangeLocation(_, loc, _) => {
                            if let Some(p) = loc.path_opt() {
                                self.crawl_path.clone_from(p);
                            }
                            Some(self.tree.rescan().map(Msg::Tree).map(cosmic::Action::App))
                        }
                        cosmic_files::tab::Command::OpenFile(files) => {
                            for f in files {
                                open::that_detached(f).unwrap();
                            }
                            None
                        }
                        _ => None,
                    }
                }));
            }
        }

        cosmic::Task::none()
    }

    fn dialog(&self) -> Option<cosmic::Element<'_, Self::Message>> {
        self.error.as_ref().map(|e| {
            cosmic::widget::dialog()
                .title(format!("Error: {e}"))
                .primary_action(cosmic::widget::button::standard("OK").on_press(Msg::ClearError))
                .into()
        })
    }

    fn view(&self) -> cosmic::Element<'_, Self::Message> {
        use cosmic::widget::container;

        let grid =
            cosmic::widget::pane_grid::PaneGrid::new(&self.state, move |_pane, t, _maximized| {
                match t {
                    Panels::NamePath => container(self.path_and_title())
                        .class(cosmic::theme::Container::Card)
                        .height(Length::FillPortion(2))
                        .width(Length::FillPortion(1))
                        .into(),
                    Panels::Tree => container(self.tree.view().map(Msg::Tree))
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

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        self.tree.subscription().map(Msg::Tree)
    }
}
