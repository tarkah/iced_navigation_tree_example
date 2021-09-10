use iced::executor;
use iced::scrollable::{self, Scrollable};
use iced::{Application, Clipboard, Command, Element, Length, Row, Settings, Text};

use std::env;
use std::path::PathBuf;

fn main() {
    let current_dir = env::current_dir().expect("Getting current directory");

    let settings = Settings {
        flags: current_dir,
        ..Default::default()
    };

    App::run(settings).expect("Running Iced");
}

#[derive(Debug, Clone)]
enum Message {
    Navigation(navigation::Message),
}

struct App {
    navigation: navigation::State,
    read_file: Option<(PathBuf, String)>,
    scrollable: scrollable::State,
}

impl Application for App {
    type Message = Message;
    type Flags = PathBuf;
    type Executor = executor::Default;

    fn new(current_dir: Self::Flags) -> (Self, Command<Self::Message>) {
        let navigation = navigation::State::Loading(current_dir.clone());

        let command = Command::perform(navigation.read_directory(current_dir), Message::Navigation);

        (
            Self {
                navigation,
                read_file: Default::default(),
                scrollable: Default::default(),
            },
            command,
        )
    }

    fn title(&self) -> String {
        "Navigation Tree Example".into()
    }

    fn update(
        &mut self,
        message: Self::Message,
        _clipboard: &mut Clipboard,
    ) -> Command<Self::Message> {
        match message {
            Message::Navigation(message) => {
                let (command, event) = self.navigation.update(message);

                if let Some(event) = event {
                    match event {
                        navigation::Event::FileRead(path, content) => {
                            self.read_file = Some((path, content));
                        }
                    }
                }

                command.map(Message::Navigation)
            }
        }
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        let nav_tree =
            navigation::NavigationTree::view(&mut self.navigation).map(Message::Navigation);

        let read_file = if let Some((path, content)) = self.read_file.as_ref() {
            format!("File: {:?}\n\n{}", path, content)
        } else {
            "Click a file to view it's content".into()
        };

        let scollable =
            Scrollable::new(&mut self.scrollable).push(Text::new(read_file).width(Length::Fill));

        Row::new().push(nav_tree).push(scollable).into()
    }
}

mod navigation {
    use iced::button::{self, Button};
    use iced::futures::FutureExt;
    use iced::scrollable::{self, Scrollable};
    use iced::{Column, Command, Container, Element, Length, Text};

    use std::fs;
    use std::future::Future;
    use std::path::PathBuf;

    #[derive(Debug, Clone)]
    pub enum Message {
        ChangeDirectory(PathBuf),
        DirectoryRead(Option<(PathBuf, Vec<Entry>)>),
        ReadFile(PathBuf),
        FileRead(Option<(PathBuf, String)>),
    }

    #[derive(Debug, Clone)]
    pub enum Event {
        FileRead(PathBuf, String),
    }

    pub struct NavigationTree {}

    impl NavigationTree {
        pub fn view(state: &mut State) -> Element<Message> {
            let content: Element<_> = match state {
                State::Loading(directory) => {
                    let text = Text::new(format!("Loading {:?}...", directory));

                    Container::new(text).center_x().center_y().into()
                }
                State::Loaded {
                    directory,
                    entries,
                    entry_buttons: buttons,
                    up_button,
                    scrollable,
                } => {
                    let mut scrollable = Scrollable::new(scrollable);

                    if let Some(parent) = directory.parent() {
                        let content = Text::new("..");

                        let button = Button::new(up_button, content)
                            .on_press(Message::ChangeDirectory(parent.to_path_buf()));

                        scrollable = scrollable.push(button);
                    };

                    for (entry, button) in entries.iter_mut().zip(buttons.iter_mut()) {
                        let name = entry.name();
                        let message = entry.message();

                        let content = Text::new(name);

                        let button = Button::new(button, content).on_press(message);

                        scrollable = scrollable.push(button);
                    }

                    let header = Text::new(format!("Entries for {:?}", directory));

                    Column::new()
                        .spacing(10)
                        .push(header)
                        .push(scrollable)
                        .into()
                }
            };

            Container::new(content).width(Length::Units(300)).into()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Entry {
        File { path: PathBuf, name: String },
        Directory { path: PathBuf, name: String },
    }

    impl Entry {
        fn name(&self) -> String {
            match self {
                Entry::File { name, .. } => format!("F - {}", name),
                Entry::Directory { name, .. } => format!("D - {}", name),
            }
        }

        fn message(&self) -> Message {
            match self {
                Entry::File { path, .. } => Message::ReadFile(path.clone()),
                Entry::Directory { path, .. } => Message::ChangeDirectory(path.clone()),
            }
        }
    }

    impl Ord for Entry {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            use std::cmp::Ordering;
            use Entry::*;

            match (self, other) {
                (Directory { .. }, File { .. }) => Ordering::Less,
                (File { .. }, Directory { .. }) => Ordering::Greater,
                (File { name: a, .. }, File { name: b, .. })
                | (Directory { name: a, .. }, Directory { name: b, .. }) => a.cmp(b),
            }
        }
    }

    impl PartialOrd for Entry {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    pub enum State {
        Loading(PathBuf),
        Loaded {
            directory: PathBuf,
            entries: Vec<Entry>,
            entry_buttons: Vec<button::State>,
            up_button: button::State,
            scrollable: scrollable::State,
        },
    }

    impl State {
        pub fn update(&mut self, message: Message) -> (Command<Message>, Option<Event>) {
            match message {
                Message::ChangeDirectory(path) => {
                    if path.is_dir() {
                        return (
                            Command::perform(self.read_directory(path), |message| message),
                            None,
                        );
                    }
                }
                Message::DirectoryRead(result) => {
                    if let Some((directory, entries)) = result {
                        let buttons = vec![button::State::new(); entries.len()];

                        *self = Self::Loaded {
                            directory,
                            entries,
                            entry_buttons: buttons,
                            up_button: button::State::new(),
                            scrollable: scrollable::State::new(),
                        };
                    }
                }
                Message::ReadFile(path) => {
                    if path.is_file() {
                        return (
                            Command::perform(self.read_file(path), |message| message),
                            None,
                        );
                    }
                }
                Message::FileRead(result) => {
                    if let Some((path, content)) = result {
                        return (Command::none(), Some(Event::FileRead(path, content)));
                    }
                }
            }

            (Command::none(), None)
        }

        pub fn read_directory(&self, path: PathBuf) -> impl Future<Output = Message> {
            read_directory(path).map(Message::DirectoryRead)
        }

        pub fn read_file(&self, path: PathBuf) -> impl Future<Output = Message> {
            read_file(path).map(Message::FileRead)
        }
    }

    async fn read_directory(path: PathBuf) -> Option<(PathBuf, Vec<Entry>)> {
        let read_dir = fs::read_dir(&path).ok()?;

        let mut entries = vec![];

        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();

            if path.is_file() {
                entries.push(Entry::File { path, name })
            } else if path.is_dir() {
                entries.push(Entry::Directory { path, name })
            }
        }

        entries.sort();

        Some((path, entries))
    }

    async fn read_file(path: PathBuf) -> Option<(PathBuf, String)> {
        let contents = fs::read_to_string(&path).ok()?;

        Some((path, contents))
    }
}
