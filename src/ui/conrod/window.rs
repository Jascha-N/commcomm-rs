use super::apps::{App, AppFactory};
use decoder::Decoder;
use error::*;

use conrod::color;
use conrod::{Colorable, Labelable, Positionable, Sizeable, Theme, Ui, UiBuilder};
use conrod::backend::glutin as conrod_glutin;
use conrod::backend::glium::Renderer;
use conrod::image::Map;
use conrod::event::Input;
use conrod::text::{self, FontCollection};
use conrod::theme::WidgetDefault;
use conrod::widget::{list_select, title_bar, Button, Canvas, ListSelect, TextBox, TitleBar, Widget};

use glium::{DisplayBuild, Display};
use glium::debug::{DebugCallbackBehavior, MessageType, Severity, Source};
use glium::texture::Texture2d;
use glutin::{self, Api, ElementState, Event as GlutinEvent, GlRequest, VirtualKeyCode, WindowBuilder};

use log::LogLevel;

use std::any::TypeId;
use std::mem;



const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const FONT: &'static [u8] = include_bytes!("../../resources/fonts/univers-light-normal.ttf");



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
        CONTROL_TITLE,
        INPUT_LINE
    }
}

pub struct Window {
    display_builders: (WindowBuilder<'static>, WindowBuilder<'static>),
    display: Display,
    renderer: Renderer,
    ui: Ui,
    image_map: Map<Texture2d>,
    widgets: Widgets,
    apps: Vec<Box<App>>,
    active_app: usize
}

impl Window {
    pub fn new(app_factories: &[&AppFactory]) -> Result<Window> {
        let base_builder = WindowBuilder::new()
                                         .with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)))
                                         .with_vsync()
                                         .with_title("commcomm-rs");

        let window_builder = base_builder.clone()
                                         .with_min_dimensions(WIDTH, HEIGHT)
                                         .with_dimensions(WIDTH, HEIGHT);

        let fullscreen_builder = base_builder.with_fullscreen(glutin::get_primary_monitor());

        let display = Window::build_display(window_builder.clone())?;

        let (width, height) = {
            let window = display.get_window().unwrap();
            window.get_inner_size_pixels().unwrap()
        };

        info!(t!("Window created. OpenGL version: {}."), display.get_opengl_version_string());

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
                                .map_err(Error::from)
                                .chain_err(|| t!("Could not create glium renderer"))?;
        let apps = app_factories.iter().map(|factory| factory(ui.widget_id_generator())).collect();

        Ok(Window {
            display_builders: (window_builder, fullscreen_builder),
            display: display,
            renderer: renderer,
            ui: ui,
            image_map: Map::new(),
            widgets: widgets,
            apps: apps,
            active_app: 0
        })
    }

    fn build_display(display_build: WindowBuilder<'static>) -> Result<Display> {
        if cfg!(debug_assertions) {
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
        }.chain_err(|| t!("Could not create the window"))
    }

    fn toggle_fullscreen(&mut self) -> Result<()> {
        self.display = Window::build_display(self.display_builders.1.clone())?;
        self.renderer = Renderer::new(&self.display)
                                 .map_err(Error::from)
                                 .chain_err(|| t!("Could not create glium renderer"))?;

        let window = self.display.get_window().unwrap();
        if let Some(win_rect) = self.ui.rect_of(self.ui.window) {
            let (win_w, win_h) = (win_rect.w() as u32, win_rect.h() as u32);
            let (w, h) = window.get_inner_size_points().unwrap();
            if w != win_w || h != win_h {
                self.ui.handle_event(Input::Resize(w, h));
            }
        }

        mem::swap(&mut self.display_builders.0, &mut self.display_builders.1);

        Ok(())
    }

    fn handle_events(&mut self, decoder: &mut Decoder) -> Result<bool> {
        let mut closing = false;
        let mut toggle_fullscreen = false;

        {
            let window = self.display.get_window().unwrap();
            let mut ignore_next_char = false;
            for event in self.display.poll_events() {
                match event {
                    GlutinEvent::Resized(0, 0) => {
                        continue;
                    }
                    GlutinEvent::Closed => {
                        closing = true;
                    }
                    GlutinEvent::KeyboardInput(ElementState::Released, _, Some(VirtualKeyCode::F11)) => {
                        toggle_fullscreen = true;
                    }
                    GlutinEvent::KeyboardInput(state, _, Some(keycode)) => {
                        let input = match keycode {
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
                        };

                        if let Some(input) = input {
                            if let ElementState::Released = state {
                                decoder.process_input(input);
                            } else {
                                ignore_next_char = true;
                            }
                            continue;
                        }
                    }
                    GlutinEvent::ReceivedCharacter(c) if c.is_digit(10) && ignore_next_char => {
                        ignore_next_char = false;
                        continue;
                    }
                    _ => {}
                }

                if let Some(event) = conrod_glutin::convert(event, &*window) {
                    self.ui.handle_event(event);
                }
            }
        }

        if closing {
            Ok(false)
        } else {
            if toggle_fullscreen {
                self.toggle_fullscreen()?;
            }

            Ok(true)
        }
    }

    fn update_ui(&mut self, decoder: &mut Decoder) {
        let ui = &mut self.ui.set_widgets();

        let font_size = title_bar::Style::new().font_size(&ui.theme);

        let labeled_canvas = Canvas::new()
                                    .pad(10.0)
                                    .pad_top(title_bar::calc_height(font_size) + 10.0);

        Canvas::new()
               .flow_right(&[
                   (self.widgets.CONTENT_CANVAS,
                    Canvas::new()
                           .pad(10.0)
                           .pad_top(title_bar::calc_height(font_size) * 2.0 + 20.0)),
                   (self.widgets.SIDEBAR_CANVAS,
                    Canvas::new()
                           .length(200.0)
                           .flow_down(&[
                               (self.widgets.MODE_CANVAS, labeled_canvas.length(200.0)),
                               (self.widgets.CONTROL_CANVAS, labeled_canvas),
                               //(self.widgets.CONTROL_CANVAS, labeled_canvas.length_weight(0.33))
                           ]))
               ])
               .set(self.widgets.ROOT_CANVAS, ui);

        TitleBar::new(t!("Mode"), self.widgets.MODE_CANVAS)
                 .place_on_kid_area(false)
                 .set(self.widgets.MODE_TITLE, ui);

        TitleBar::new(t!("Control"), self.widgets.CONTROL_CANVAS)
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

        let text_events = TextBox::new(&decoder.line())
                                  .padded_w_of(self.widgets.CONTENT_CANVAS, 10.0)
                                  .middle_of(self.widgets.CONTENT_CANVAS)
                                  .color(color::LIGHT_GREY)
                                  .place_on_kid_area(false)
                                  .down_from(self.widgets.CONTENT_TITLE, 10.0)
                                  .set(self.widgets.INPUT_LINE, ui);

        // for event in text_events {
        //     match event {
        //         text_box::Event::Update(text) => {
        //             //self.text = text;
        //         }
        //         text_box::Event::Enter if !decoder.line().is_empty() => {
        //             //self.voice.speak(&self.text).unwrap();
        //             app.process_line(decoder.line());
        //         }
        //         //_ => {}
        //     }
        // }

        app.update_ui(self.widgets.CONTENT_CANVAS, ui);
    }

    fn draw_if_changed(&mut self) -> Result<()> {
        while let Some(primitives) = self.ui.draw_if_changed() {
            self.renderer.fill(&self.display, primitives, &self.image_map);
            let mut target = self.display.draw();
            self.renderer.draw(&self.display, &mut target, &self.image_map)
                         .map_err(Error::from)
                         .chain_err(|| t!("An error occured while drawing"))?;
            target.finish().chain_err(|| t!("Error while swapping buffers"))?;
        }

        Ok(())
    }

    pub fn update(&mut self, decoder: &mut Decoder) -> Result<bool> {
        if self.handle_events(decoder)? {
            self.update_ui(decoder);
            self.draw_if_changed()?;

            Ok(true)
        } else {
            Ok(false)
        }
    }
}
