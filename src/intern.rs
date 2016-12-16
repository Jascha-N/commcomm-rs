use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

thread_local!(static INTERNER: RefCell<HashMap<String, Weak<String>>> = RefCell::new(HashMap::new()));

pub trait Intern {
    fn intern(&self) -> Rc<String>;
}

impl<T: AsRef<str>> Intern for T {
    fn intern(&self) -> Rc<String> {
        let string = self.as_ref();
        INTERNER.with(|interner| {
            interner.borrow().get(string).and_then(Weak::upgrade).unwrap_or_else(|| {
                let interned = Rc::new(string.to_string());
                interner.borrow_mut().insert(string.to_string(), Rc::downgrade(&interned));
                interned
            })
        })
    }
}