use std::rc::Rc;
use std::cell::RefCell;
use std::{ops, fmt};

pub struct DropLog<T> {
    log: Rc<RefCell<Vec<T>>>,
}

pub struct LogOnDrop<T> {
    value: Option<T>,
    log: Rc<RefCell<Vec<T>>>,
}

impl<T> Drop for LogOnDrop<T> {
    fn drop(&mut self) {
        self.log.borrow_mut().push(self.value.take().unwrap())
    }
}

impl<T> DropLog<T> {
    pub fn new() -> Self
    { DropLog {
        log: Rc::new(RefCell::new(vec![])),
    }}

    pub fn wrap(&self, value: T) -> LogOnDrop<T>
    { LogOnDrop {
        value: Some(value),
        log: self.log.clone(),
    }}

    // NOTE: Reads to Vec so that the RefCell lock can be released.
    /// Read the log of all values that were dropped after
    /// passing through `self.wrap()`.
    pub fn read(&self) -> Vec<T>
    where T: Clone,
    { self.log.borrow().to_vec() }
}

impl<T> ops::Deref for LogOnDrop<T> {
    type Target = T;

    fn deref(&self) -> &T
    { self.value.as_ref().unwrap() }
}

impl<T> ops::DerefMut for LogOnDrop<T> {
    fn deref_mut(&mut self) -> &mut T
    { self.value.as_mut().unwrap() }
}

impl<T: PartialEq> PartialEq<T> for LogOnDrop<T> {
    fn eq(&self, other: &T) -> bool
    { self.value.as_ref().unwrap() == other }
}

impl<T: fmt::Debug> fmt::Debug for LogOnDrop<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("LogOnDrop")
            .field(&self.value)
            .finish()
    }
}
