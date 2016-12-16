use self::apps::*;
use arduino::{Event as ArduinoEvent, Port};
use arduino::thread::ArduinoController;
use error::*;

use conrod::color;
use conrod::{Colorable, Labelable, Positionable, Sizeable, Theme, Ui, UiBuilder};
use conrod::backend::glutin;
use conrod::backend::glium::Renderer;
use conrod::image::Map;
use conrod::text::{self, FontCollection};
use conrod::theme::WidgetDefault;
use conrod::widget::{list_select, title_bar, Button, Canvas, ListSelect, TitleBar, Widget};

use glium::{DisplayBuild, Display};
use glium::debug::{DebugCallbackBehavior, MessageType, Severity, Source};
use glium::texture::Texture2d;
use glutin::{Api, ElementState, Event as GlutinEvent, GlRequest, VirtualKeyCode, WindowBuilder};

use log::LogLevel;

use std::any::TypeId;
use std::fmt::Write;
use std::mem;
use std::result::Result as StdResult;
use std::thread;
use std::time::Duration;

mod apps;



const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const FONT: &'static [u8] = include_bytes!("../resources/fonts/univers-light-normal.ttf");



widget_ids! {
    #[allow(non_snake_case)]
    struct Widgets {
        ROOT_CANVAS,
        CONTENT_CANVAS,
        CONTENT_TITLE,
        SIDEBAR_CANVAS,
        MODE_CANVAS,
        MODE_TITLE,
        MODE_TABS,
        CONTROL_CANVAS,
        CONTROL_TITLE
    }
}

pub struct Window {
    display: Display,
    renderer: Renderer,
    ui: Ui,
    image_map: Map<Texture2d>,
    widgets: Widgets,
    apps: Vec<Box<App>>,
    active_app: usize,
    arduino: ArduinoController
}

impl Window {
    pub fn new(app_factories: &[&AppFactory]) -> Result<Window> {
        let display_build = WindowBuilder::new()
                                          .with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)))
                                          .with_vsync()
                                          .with_min_dimensions(WIDTH, HEIGHT)
                                          .with_dimensions(WIDTH, HEIGHT)
                                          .with_title("commcomm-rs");

        let display = if cfg!(debug_assertions) {
            fn callback(source: Source, typ: MessageType, severity: Severity, id: u32, report: bool, message: &str) {
                if report {
                    let level = match severity {
                        Severity::Notification => LogLevel::Debug,
                        Severity::Low => LogLevel::Info,
                        Severity::Medium => LogLevel::Warn,
                        Severity::High => LogLevel::Error
                    };
                    log!(target: &format!("<glium>::{:?}/{:?}:{}", source, typ, id), level, "{}", message);
                }
            }

            display_build.build_glium_debug(DebugCallbackBehavior::Custom {
                callback: Box::new(callback),
                synchronous: false
            })
        } else {
            display_build.build_glium()
        }.chain_err(|| text!("Could not create the window"))?;

        let (width, height) = {
            let window = display.get_window().unwrap();
            window.get_inner_size_pixels().unwrap()
        };

        info!(text!("Window created. OpenGL version: {}."), display.get_opengl_version_string());

        let mut theme = Theme {
            name: "commcomm-rs standard".to_string(),
            background_color: color::LIGHT_GREY,
            label_color: color::BLACK,
            shape_color: color::BLACK,
            border_color: color::DARK_GRAY,
            ..Theme::default()
        };

        let title_bar_style = title_bar::Style {
            color: Some(color::GRAY),
            ..title_bar::Style::new()
        };

        theme.widget_styling.insert(TypeId::of::<title_bar::Style>(),
                                    WidgetDefault::new(Box::new(title_bar_style)));

        let mut ui = UiBuilder::new([width as f64, height as f64])
                               .theme(theme)
                               .build();

        let widgets = Widgets::new(ui.widget_id_generator());
        ui.fonts.insert(FontCollection::from_bytes(FONT).into_font().unwrap());

        let renderer = Renderer::new(&display)
                                .map_err(IntoBoxedError::into_boxed_error)
                                .unwrap();//.chain_err(|| text!("Could not create glium renderer"))?;
        let apps = app_factories.iter().map(|factory| factory(ui.widget_id_generator())).collect();

        Ok(Window {
            display: display,
            renderer: renderer,
            ui: ui,
            image_map: Map::new(),
            widgets: widgets,
            apps: apps,
            active_app: 0,
            arduino: ArduinoController::new(Port::new("COM3"), Vec::new())
        })
    }

    fn handle_events(&mut self) -> bool {
        let window = self.display.get_window().unwrap();
        for event in self.display.poll_events() {
            // if let GlutinEvent::Resized(x, y) = event {
            //     if x == 0 || y == 0 {
            //         continue;
            //     }
            // }

            if let Some(event) = glutin::convert(event.clone(), &*window) {
                self.ui.handle_event(event);
            }

            if let GlutinEvent::Closed = event {
                return false;
            }
        }

        // if let Some(win_rect) = self.ui.rect_of(self.ui.window) {
        //     let (win_w, win_h) = (win_rect.w() as u32, win_rect.h() as u32);
        //     let (w, h) = window.get_inner_size_points().unwrap();
        //     if w != win_w || h != win_h {
        //         let event = ::conrod::event::Input::Resize(w, h);
        //         self.ui.handle_event(event);
        //     }
        // }

        true
    }

    fn update_ui(&mut self) {
        let ui = &mut self.ui.set_widgets();

        let font_size = title_bar::Style::new().font_size(&ui.theme);

        let labeled_canvas = Canvas::new()
                                    .pad(10.0)
                                    .pad_top(title_bar::calc_height(font_size) + 10.0);

        Canvas::new()
               .flow_right(&[
                   (self.widgets.CONTENT_CANVAS, labeled_canvas),
                   (self.widgets.SIDEBAR_CANVAS, Canvas::new()
                                                        .length(200.0)
                                                        .flow_down(&[
                                                            (self.widgets.MODE_CANVAS, labeled_canvas.length_weight(0.33)),
                                                            (self.widgets.CONTROL_CANVAS, labeled_canvas.length_weight(0.33)),
                                                            //(self.widgets.CONTROL_CANVAS, labeled_canvas.length_weight(0.33))
                                                        ]))
               ])
               .set(self.widgets.ROOT_CANVAS, ui);

        TitleBar::new(text!("Mode"), self.widgets.MODE_CANVAS)
                 .place_on_kid_area(false)
                 .set(self.widgets.MODE_TITLE, ui);

        TitleBar::new(text!("Control"), self.widgets.CONTROL_CANVAS)
                 .place_on_kid_area(false)
                 .set(self.widgets.CONTROL_TITLE, ui);

        let (mut tab_events, _) = ListSelect::single(self.apps.len(), text::height(1, font_size, 0.0) * 2.0)
                                             .kid_area_wh_of(self.widgets.MODE_CANVAS)
                                             .mid_top_of(self.widgets.MODE_CANVAS)
                                             .set(self.widgets.MODE_TABS, ui);

        let active_app = self.active_app;
        while let Some(event) = tab_events.next(ui, |i| i == active_app) {
            match event {
                list_select::Event::Item(item) => {
                    let (color, label_color) = if item.i == active_app {
                        (color::GREY, color::BLACK)
                    } else {
                        (color::LIGHT_GREY, color::BLACK)
                    };

                    let button = Button::new()
                                        .color(color)
                                        .label(self.apps[item.i].title())
                                        .label_color(label_color);

                    item.set(button, ui);
                }
                list_select::Event::Selection(index) => {
                    self.active_app = index;
                }
                _ => {}
            }
        }

        let app = &mut self.apps[self.active_app];

        TitleBar::new(app.title(), self.widgets.CONTENT_CANVAS)
                 .place_on_kid_area(false)
                 .set(self.widgets.CONTENT_TITLE, ui);

        app.update_ui(self.widgets.CONTENT_CANVAS, ui);
    }

    fn draw_if_changed(&mut self) -> Result<()> {
        while let Some(primitives) = self.ui.draw_if_changed() {
            self.renderer.fill(&self.display, primitives, &self.image_map);
            let mut target = self.display.draw();
            self.renderer.draw(&self.display, &mut target, &self.image_map)
                         .map_err(IntoBoxedError::into_boxed_error)
                         .unwrap();//.chain_err(|| text!("An error occured while drawing"))?;
            target.finish().chain_err(|| text!("Error while swapping buffers"))?;
        }

        Ok(())
    }

    pub fn update(&mut self) -> Result<bool> {
        if self.handle_events() {
            self.update_ui();
            self.draw_if_changed()?;

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn glutin_to_arduino_event(event: &GlutinEvent) -> Option<ArduinoEvent> {
    match *event {
        GlutinEvent::KeyboardInput(state, _, Some(keycode)) => {
            match keycode {
                VirtualKeyCode::Numpad0 => Some(0),
                VirtualKeyCode::Numpad1 => Some(1),
                VirtualKeyCode::Numpad2 => Some(2),
                VirtualKeyCode::Numpad3 => Some(3),
                VirtualKeyCode::Numpad4 => Some(4),
                VirtualKeyCode::Numpad5 => Some(5),
                VirtualKeyCode::Numpad6 => Some(6),
                VirtualKeyCode::Numpad7 => Some(7),
                VirtualKeyCode::Numpad8 => Some(8),
                VirtualKeyCode::Numpad9 => Some(9),
                _ => None
            }.map(|keycode| {
                match state {
                    ElementState::Pressed => ArduinoEvent::SensorFlexed(keycode),
                    ElementState::Released => ArduinoEvent::SensorExtended(keycode)
                }
            })
        }
        _ => None
    }
}

pub fn run() -> StdResult<(), ()> {
    fn run() -> Result<()> {
        info!(text!("Application started. Version: {}. Debug mode: {}."),
            env!("CARGO_PKG_VERSION"),
            if cfg!(debug_assertions) { text!("Yes") } else { text!("No") });
        let mut window = Window::new(&[&Speech::new_app, &Editor::new_app])?;
        while window.update()? {
            thread::sleep(Duration::from_millis(1));
        }
        info!(text!("The window was closed."));
        mem::drop(window);
        info!(text!("The application is shutting down."));

        Ok(())
    }

    #[cfg(windows)]
    fn error_message_box(message: &str) {
        ::platform::windows::error_message_box(message);
    }

    #[cfg(not(windows))]
    fn error_message_box(_: &str) {}

    if let Err(error) = run() {
        let mut chain = error.iter();
        let mut message = String::new();
        let _ = write!(message, text!("An error has occurred: {}."), chain.next().unwrap());
        for cause in chain {
            let _ = write!(message, text!("\n  Caused by:\n    {}."), cause);
        }

        println!("{}", message);
        error_message_box(&message);

        Err(())
    } else {
        Ok(())
    }
}