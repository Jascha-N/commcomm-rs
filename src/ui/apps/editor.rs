use ui::App;

use conrod::{Positionable, Sizeable, UiCell};
use conrod::widget::{id, Canvas, Id, Scrollbar, TextEdit, Widget};



widget_ids! {
    #[allow(non_snake_case)]
    struct Widgets {
        CANVAS,
        SCROLLBOX,
        SCROLLBAR,
        TEXT
    }
}

pub struct Editor {
    text: String,
    widgets: Widgets
}

impl Editor {
    pub fn new(generator: id::Generator) -> Editor {
        Editor {
            text: String::new(),
            widgets: Widgets::new(generator)
        }
    }

    pub fn new_app(generator: id::Generator) -> Box<App> {
        Box::new(Editor::new(generator))
    }
}

impl App for Editor {
    fn title(&self) -> &str {
        "Tekstverwerker"
    }

    fn update_ui(&mut self, root: Id, ui: &mut UiCell) {
        Canvas::new()
               .scroll_kids_vertically()
               .pad(10.0)
               .kid_area_wh_of(root)
               .middle_of(root)
               .set(self.widgets.SCROLLBOX, ui);

        if let Some(new_text) = TextEdit::new(&self.text)
                                         .kid_area_wh_of(self.widgets.SCROLLBOX)
                                         .middle_of(self.widgets.SCROLLBOX)
                                         .line_spacing(8.0)
                                         .restrict_to_height(false)
                                         .set(self.widgets.TEXT, ui)
        {
            self.text = new_text;
        }

        Scrollbar::y_axis(self.widgets.SCROLLBOX)
                  .auto_hide(true)
                  .set(self.widgets.SCROLLBAR, ui);
    }
}