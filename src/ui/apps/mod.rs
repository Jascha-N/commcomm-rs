pub use self::editor::Editor;
pub use self::speech::Speech;

use conrod::UiCell;
use conrod::widget::{id, Id};

mod editor;
mod speech;

pub trait App {
    fn title(&self) -> &str;
    fn update_ui(&mut self, root: Id, ui: &mut UiCell);
}

pub type AppFactory = Fn(id::Generator) -> Box<App>;