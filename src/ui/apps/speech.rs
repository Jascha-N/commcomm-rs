use ui::App;
use speech::{SpeechEngine, SpeechEngineImpl, Voice};

use chrono::{Local, NaiveTime};

use conrod::{Colorable, Positionable, Sizeable, UiCell};
use conrod::color;
use conrod::text as raw_text;
use conrod::widget::{id, text, text_box, Id, List, Text, TextBox, Widget};

use std::collections::VecDeque;
use std::mem;



struct Line {
    time: NaiveTime,
    text: String
}

widget_ids! {
    #[allow(non_snake_case)]
    struct Widgets {
        CANVAS,
        TEXT,
        LINES
    }
}

pub struct Speech {
    text: String,
    lines: VecDeque<Line>,
    widgets: Widgets,
    voice: Voice
}

impl Speech {
    pub fn new(generator: id::Generator) -> Speech {
        let engine = SpeechEngine::new().unwrap();

        let mut voice = engine.voice().unwrap();
        voice.set_voice(engine.token_from_id(r#"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech\Voices\Tokens\MSTTS_V110_nlNL_Frank"#).unwrap()).unwrap();
        //voice.set_language(w::MAKELANGID(w::LANG_DUTCH, w::SUBLANG_DUTCH)).unwrap();

        Speech {
            text: String::new(),
            lines: VecDeque::new(),
            widgets: Widgets::new(generator),
            voice: voice
        }
    }

    pub fn new_app(generator: id::Generator) -> Box<App> {
        Box::new(Speech::new(generator))
    }
}

impl App for Speech {
    fn title(&self) -> &str {
        "Spraak"
    }

    fn update_ui(&mut self, root: Id, ui: &mut UiCell) {
        let font_size = text::Style::new().font_size(&ui.theme);

        let text_events = TextBox::new(&self.text)
                                  .color(color::LIGHT_GREY)
                                  .kid_area_w_of(root)
                                  .mid_top_of(root)
                                  .set(self.widgets.TEXT, ui);

        for event in text_events {
            match event {
                text_box::Event::Update(text) => {
                    self.text = text;
                }
                text_box::Event::Enter if !self.text.is_empty() => {
                    self.voice.speak(&self.text).unwrap();
                    self.lines.push_front(Line {
                        time: Local::now().time(),
                        text: mem::replace(&mut self.text, String::new())
                    })
                }
                _ => {}
            }
        }

        let (mut items, _) = List::new(self.lines.len(), raw_text::height(1, font_size, 0.0) * 2.0)
                                  .kid_area_wh_of(root)
                                  .down_from(self.widgets.TEXT, 20.0)
                                  .set(self.widgets.LINES, ui);

        while let Some(item) = items.next(ui) {
            let line = &self.lines[item.i];
            item.set(Text::new(&format!("{}   {}", line.time.format("%T").to_string(), line.text)), ui);
        }
    }
}