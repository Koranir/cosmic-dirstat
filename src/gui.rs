use std::{collections::HashMap, ffi::OsString, path::PathBuf, sync::Arc};

mod partition_view;

use cosmic::{
    app::Task,
    iced::{Color, Length, Point, alignment::Horizontal},
    iced_widget::scrollable,
    widget::{self, container, grid},
};

pub fn run() {
    cosmic::app::run::<App>(cosmic::app::Settings::default().transparent(true), ()).unwrap();
}

#[derive(Debug, Clone)]
enum Msg {
    ItemClicked(PathBuf),
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
    Tree(cosmic_files::tab::Message),
    ModifiersChanged(cosmic::iced::keyboard::Modifiers),
    NewItems(Vec<cosmic_files::tab::Item>),
    TreeSelect(PathBuf),
    Frame,
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
    tree: cosmic_files::tab::Tab,
    tree_binds: HashMap<cosmic::widget::menu::KeyBind, cosmic_files::app::Action>,
    modifiers: cosmic::iced::keyboard::Modifiers,
    analyzed: Option<Arc<crate::analyze::AnalyzedDir>>,
    error: Option<String>,
    extensions_ordered: Vec<(OsString, Color)>,
    highlighted: Option<(Point, String, u64, PathBuf)>,
    focus_next_frame: bool,
}
impl App {
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

    pub fn tree_view(&self) -> cosmic::Element<'_, Msg> {
        self.tree
            .view(&self.tree_binds, &self.modifiers, false)
            .map(Msg::Tree)
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
                    8.0,
                    8.0 * 8.0,
                    Msg::ItemClicked,
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

    pub fn rescan(&mut self) -> Task<Msg> {
        let loc = self.tree.location.clone();

        cosmic::Task::perform(
            async move { loc.scan(cosmic_files::config::IconSizes::default()) },
            |(_, items)| cosmic::Action::App(Msg::NewItems(items)),
        )
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

        pub fn tree_key_binds() -> HashMap<widget::menu::KeyBind, cosmic_files::app::Action> {
            use cosmic::iced::keyboard::{Key, key::Named};
            use cosmic_files::app::Action;
            use widget::menu::{KeyBind, key_bind::Modifier};

            let mut key_binds = HashMap::new();

            macro_rules! bind {
                ([$($modifier:ident),* $(,)?], $key:expr, $action:ident) => {{
                    key_binds.insert(
                        KeyBind {
                            modifiers: vec![$(Modifier::$modifier),*],
                            key: $key,
                        },
                        Action::$action,
                    );
                }};
            }

            // Common keys
            bind!([], Key::Named(Named::ArrowDown), ItemDown);
            bind!([], Key::Named(Named::ArrowLeft), ItemLeft);
            bind!([], Key::Named(Named::ArrowRight), ItemRight);
            bind!([], Key::Named(Named::ArrowUp), ItemUp);
            bind!([], Key::Named(Named::F5), Reload);
            bind!([], Key::Named(Named::Home), SelectFirst);
            bind!([], Key::Named(Named::End), SelectLast);
            bind!([Shift], Key::Named(Named::ArrowDown), ItemDown);
            bind!([Shift], Key::Named(Named::ArrowLeft), ItemLeft);
            bind!([Shift], Key::Named(Named::ArrowRight), ItemRight);
            bind!([Shift], Key::Named(Named::ArrowUp), ItemUp);
            bind!([Shift], Key::Named(Named::Home), SelectFirst);
            bind!([Shift], Key::Named(Named::End), SelectLast);
            bind!([Ctrl, Shift], Key::Character("n".into()), NewFolder);
            bind!([], Key::Named(Named::Enter), Open);
            bind!([Ctrl], Key::Character(" ".into()), Preview);
            bind!([], Key::Character(" ".into()), Gallery);

            bind!([Ctrl], Key::Character("h".into()), ToggleShowHidden);
            bind!([Ctrl], Key::Character("a".into()), SelectAll);
            bind!([Ctrl], Key::Character("=".into()), ZoomIn);
            bind!([Ctrl], Key::Character("+".into()), ZoomIn);
            bind!([Ctrl], Key::Character("0".into()), ZoomDefault);
            bind!([Ctrl], Key::Character("-".into()), ZoomOut);
            // Switch view
            bind!([Ctrl], Key::Character("1".into()), TabViewList);
            bind!([Ctrl], Key::Character("2".into()), TabViewGrid);

            key_binds
        }

        let cwd = std::env::current_dir().unwrap_or_default();

        let mut app = Self {
            core,
            crawl_path: cwd.clone(),
            crawling_path: false,
            state,
            analyzed: None,
            error: None,
            extensions_ordered: Vec::new(),
            highlighted: None,
            tree: cosmic_files::tab::Tab::new(
                cosmic_files::tab::Location::Path(cwd),
                files_config.tab,
                files_config.thumb_cfg,
                None,
                cosmic::iced::widget::Id::unique(),
                None,
            ),
            tree_binds: tree_key_binds(),
            modifiers: cosmic::iced::keyboard::Modifiers::empty(),
            focus_next_frame: false,
        };

        let task = app.rescan();

        (app, task)
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Msg::ItemClicked(s) => {
                if s.is_dir() {
                    return self.update(Msg::CrawlPathChanged(s));
                } else {
                    if let Some(parent) = s.parent() {
                        return self
                            .update(Msg::CrawlPathChanged(parent.into()))
                            .chain(Task::done(cosmic::Action::App(Msg::TreeSelect(s))));
                    }
                }
            }
            Msg::CrawlPathChanged(s) => {
                if s == self.crawl_path {
                    return Task::none();
                }

                self.crawl_path = s.clone();
                self.tree
                    .change_location(&cosmic_files::tab::Location::Path(s), None);
                return self.rescan();
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
            Msg::Tree(message) => {
                let commands = self.tree.update(message, self.modifiers);

                return Task::batch(commands.into_iter().filter_map(|c| match c {
                    cosmic_files::tab::Command::Iced(task) => {
                        Some(task.0.map(|t| cosmic::Action::App(Msg::Tree(t))))
                    }
                    // cosmic_files::tab::Command::Action(action) => {
                    //     self.tree.update(action.message(), self.modifiers);
                    // }
                    cosmic_files::tab::Command::ChangeLocation(_, loc, _) => {
                        loc.path_opt().map(|p| {
                            self.crawl_path.clone_from(p);
                        });
                        // dbg!(loc);
                        Some(self.rescan())
                    }
                    cosmic_files::tab::Command::OpenFile(files) => {
                        for f in files {
                            open::that_detached(f).unwrap();
                        }
                        None
                    }
                    _ => None,
                }));
            }
            Msg::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            Msg::NewItems(items) => self.tree.set_items(items),
            Msg::TreeSelect(s) => {
                self.tree.select_paths([s].into());
                self.focus_next_frame = true;
            }
            Msg::Frame => {
                if self.focus_next_frame {
                    self.focus_next_frame = false;

                    return self.update(Msg::Tree(cosmic_files::tab::Message::ScrollToFocused));
                }
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

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::Subscription::batch([
            cosmic::iced::keyboard::listen().filter_map(|k| match k {
                cosmic::iced::keyboard::Event::ModifiersChanged(modifiers) => {
                    Some(Msg::ModifiersChanged(modifiers))
                }
                _ => None,
            }),
            self.tree.subscription(true).map(Msg::Tree),
            if self.focus_next_frame {
                cosmic::iced::window::frames().map(|_| Msg::Frame)
            } else {
                cosmic::iced::Subscription::none()
            },
        ])
    }
}
