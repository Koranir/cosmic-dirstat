use std::{collections::HashMap, path::PathBuf};

use cosmic::{
    Task,
    iced::{self, keyboard::Modifiers},
};

#[derive(Debug, Clone)]
pub enum Msg {
    Tab(cosmic_files::tab::Message),
    ModifiersChanged(Modifiers),
    NewItems(Vec<cosmic_files::tab::Item>),
    SelectPath(PathBuf),
    Frame,
}

pub struct FileTree {
    tab: cosmic_files::tab::Tab,
    tree_binds: HashMap<cosmic::widget::menu::KeyBind, cosmic_files::app::Action>,
    modifiers: Modifiers,
    focus_next_frame: bool,
}

impl FileTree {
    pub fn new(location: PathBuf, files_config: cosmic_files::config::Config) -> Self {
        Self {
            tab: cosmic_files::tab::Tab::new(
                cosmic_files::tab::Location::Path(location),
                files_config.tab,
                files_config.thumb_cfg,
                None,
                cosmic::iced::widget::Id::unique(),
                None,
            ),
            tree_binds: tree_key_binds(),
            modifiers: Modifiers::empty(),
            focus_next_frame: false,
        }
    }

    pub fn view(&self) -> cosmic::Element<'_, Msg> {
        self.tab
            .view(&self.tree_binds, &self.modifiers, false, &[])
            .map(Msg::Tab)
    }

    pub fn rescan(&self) -> Task<Msg> {
        let loc = self.tab.location.clone();

        cosmic::Task::perform(
            async move { loc.scan(cosmic_files::config::IconSizes::default()) },
            |(_, items)| Msg::NewItems(items),
        )
    }

    pub fn change_location(&mut self, location: PathBuf) {
        self.tab
            .change_location(&cosmic_files::tab::Location::Path(location), None);
    }

    pub fn update(&mut self, message: Msg) -> Vec<cosmic_files::tab::Command> {
        match message {
            Msg::Tab(message) => self.tab.update(message, self.modifiers),
            Msg::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                Vec::new()
            }
            Msg::NewItems(items) => {
                self.tab.set_items(items);
                Vec::new()
            }
            Msg::SelectPath(path) => {
                self.tab.select_paths([path].into());
                self.focus_next_frame = true;
                Vec::new()
            }
            Msg::Frame => {
                if self.focus_next_frame {
                    self.focus_next_frame = false;

                    return self.update(Msg::Tab(cosmic_files::tab::Message::ScrollToFocused));
                }

                Vec::new()
            }
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Msg> {
        iced::Subscription::batch([
            iced::keyboard::listen().filter_map(|k| match k {
                iced::keyboard::Event::ModifiersChanged(modifiers) => {
                    Some(Msg::ModifiersChanged(modifiers))
                }
                _ => None,
            }),
            self.tab.subscription(true).map(Msg::Tab),
            if self.focus_next_frame {
                iced::window::frames().map(|_| Msg::Frame)
            } else {
                iced::Subscription::none()
            },
        ])
    }
}

fn tree_key_binds() -> HashMap<cosmic::widget::menu::KeyBind, cosmic_files::app::Action> {
    use cosmic::iced::keyboard::{Key, key::Named};
    use cosmic::widget::menu::{KeyBind, key_bind::Modifier};
    use cosmic_files::app::Action;

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
