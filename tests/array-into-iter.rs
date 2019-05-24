//---- Copy the definition from src/omnipresent.md below -----
use std::mem::ManuallyDrop;
use std::ptr;

pub struct ArrayIntoIter<T> {
    array: [ManuallyDrop<T>; 3],
    index: usize,
}

impl<T> ArrayIntoIter<T> {
    pub fn new(array: [T; 3]) -> Self {
        let [a, b, c] = array;
        let wrap = ManuallyDrop::new;
        ArrayIntoIter {
            array: [wrap(a), wrap(b), wrap(c)],
            index: 0,
        }
    }
}

impl<T> Iterator for ArrayIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self.index {
            3 => None,
            i => {
                self.index += 1;
                Some(ManuallyDrop::into_inner(unsafe { ptr::read(&self.array[i]) }))
            }
        }
    }
}

impl<T> Drop for ArrayIntoIter<T> {
    fn drop(&mut self) {
        // Run to completion
        self.for_each(drop);
    }
}
//------------------------------------------------------------

mod util;

use crate::util::DropLog;

#[test]
fn no_iteration() {
    let log = DropLog::new();
    {
        let array = [log.wrap(1), log.wrap(2), log.wrap(3)];
        let _ = ArrayIntoIter::new(array);
    }
    assert_eq!(log.read(), vec![1, 2, 3])
}

#[test]
fn partial_iter() {
    let log = DropLog::new();
    {
        let array = [log.wrap(1), log.wrap(2), log.wrap(3)];
        let mut iter = ArrayIntoIter::new(array);
        assert_eq!(iter.next().unwrap(), 1);
        assert_eq!(iter.next().unwrap(), 2);
    }
    assert_eq!(log.read(), vec![1, 2, 3])
}

#[test]
fn over_iter() {
    let log = DropLog::new();
    {
        let array = [log.wrap(1), log.wrap(2), log.wrap(3)];
        let mut iter = ArrayIntoIter::new(array);
        assert_eq!(iter.next().unwrap(), 1);
        assert_eq!(iter.next().unwrap(), 2);
        assert_eq!(iter.next().unwrap(), 3);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    assert_eq!(log.read(), vec![1, 2, 3])
}
