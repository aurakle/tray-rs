//! Example demonstrating how to spawn an iced window on tray icon click.
//!
//! This example shows the pattern for integrating tray icons with iced applications.
//! When the tray icon is clicked, a popup window appears at the cursor position.
//!
//! Run with: cargo run -p tray --example iced-popup

use std::collections::BTreeMap;
use std::time::Duration;

use iced::widget::{button, center, column, text};
use iced::window;
use iced::{Element, Point, Size, Subscription, Task};

use tray::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub fn main() -> iced::Result {
    iced::daemon(
        |state: &App, window_id| state.title(window_id),
        App::update,
        App::view,
    )
    .subscription(App::subscription)
    .run_with(App::new)
}

struct App {
    _tray_icon: TrayIcon,
    windows: BTreeMap<window::Id, WindowState>,
}

struct WindowState {
    counter: i32,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    Increment(window::Id),
    Close(window::Id),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");
        let icon = load_icon(std::path::Path::new(path));

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Iced Popup Example")
            .with_icon(icon)
            .build()
            .expect("Failed to create tray icon");

        println!("Tray icon created. Click to spawn popup windows.");

        (
            Self {
                _tray_icon: tray_icon,
                windows: BTreeMap::new(),
            },
            Task::none(),
        )
    }

    fn title(&self, window_id: window::Id) -> String {
        self.windows
            .get(&window_id)
            .map(|_| "Tray Popup".to_string())
            .unwrap_or_default()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        println!("Left click at ({}, {})", position.x, position.y);

                        let (id, open) = window::open(window::Settings {
                            size: Size::new(250.0, 180.0),
                            position: window::Position::Specific(Point::new(
                                position.x as f32,
                                position.y as f32,
                            )),
                            decorations: false,
                            resizable: false,
                            level: window::Level::AlwaysOnTop,
                            ..Default::default()
                        });

                        self.windows.insert(id, WindowState { counter: 0 });

                        return open.map(Message::WindowOpened);
                    }
                }
                Task::none()
            }
            Message::WindowOpened(_id) => Task::none(),
            Message::WindowClosed(id) => {
                self.windows.remove(&id);
                Task::none()
            }
            Message::Increment(id) => {
                if let Some(state) = self.windows.get_mut(&id) {
                    state.counter += 1;
                }
                Task::none()
            }
            Message::Close(id) => window::close(id),
        }
    }

    fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        if let Some(state) = self.windows.get(&window_id) {
            center(
                column![
                    text("Tray Popup").size(24),
                    text("Click position popup").size(12),
                    text(format!("Counter: {}", state.counter)).size(18),
                    button("Increment").on_press(Message::Increment(window_id)),
                    button("Close").on_press(Message::Close(window_id)),
                ]
                .spacing(10)
                .padding(20),
            )
            .into()
        } else {
            center(text("Loading...")).into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            window::close_events().map(Message::WindowClosed),
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
        ])
    }
}

fn load_icon(path: &std::path::Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to create icon")
}
