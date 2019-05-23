Omnipresent concerns
====================

These concerns may come up regardless of what kind of `unsafe` code you're writing.

<a id="drop-safety"></a>

Drop safety
-----------

**Things to look for:**

* Usage of `unsafe` in any generic function that doesn't have `T: Copy` bounds.
* Usage of `unsafe` near code that can panic.

**Summary:** `unsafe` code often puts data in a state where it would be dangerous for a destructor to run. The possibility that code may unwind amplifies this problem immensely.  **Most `unsafe` code needs to worry about drop safety at some point.**

### Danger: A value read using `std::ptr::read` may get dropped twice

(This also applies to `<*const T>::read`, which is basically the same function)

**Incorrect**

```rust
use std::ptr;

struct ArrayIntoIter<T> {
    array: [T; 3],
    index: usize,
}

impl<T> Iterator for ArrayIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self.index {
            3 => None,
            i => {
                self.index += 1;
                Some(unsafe { ptr::read(&self.array[i]) })
            }
        }
    }
}
```

When the `ArrayIntoIter<T>` is dropped, all of the elements will be dropped, even though ownership of some of the elements may have already been given away.

For this reason, usage of `std::ptr::read` must almost always be paired together with usage of `std::mem::forget`, or, better yet, `std::mem::ManuallyDrop` (available since 1.20.0) which is capable of solving a broader variety of problems.  (In fact, it is impossible to fix the above example using only `mem::forget`)

**Correct**

```rust
use std::mem::ManuallyDrop;
use std::ptr;

struct ArrayIntoIter<T> {
    array: [ManuallyDrop<T>; 3],
    index: usize,
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
```

### Danger: Closures can panic

**Incorrect**

```rust
pub fn filter_inplace<T>(
    vec: &mut Vec<T>,
    mut pred: impl FnMut(&mut T) -> bool,
) {
    let mut write_idx = 0;

    for read_idx in 0..vec.len() {
        if pred(&mut vec[read_idx]) {
            if read_idx != write_idx {
                unsafe {
                    ptr::copy_nonoverlapping(&vec[read_idx], &mut vec[write_idx], 1);
                }
            }
            write_idx += 1;
        } else {
            drop(unsafe { ptr::read(&vec[read_idx]) });
        }
    }
    unsafe { vec.set_len(write_idx); }
}
```

When `pred()` panics, we never reach the final `.set_len()`, and some elements may get dropped twice.

### Danger: Any method on any safe trait can panic

A generalization of the previous point.  You can't even trust `clone` to not panic!

**Incorrect**

```rust
pub fn remove_all<T: Eq>(
    vec: &mut Vec<T>,
    target: &T,
) {
    // same as filter_inplace
    // but replace   if pred(&mut vec[read_idx])
    //        with   if &vec[read_idx] == target
}
```

### Danger: Drop can panic!

This particularly nefarious special case of the prior point will leave you tearing your hair out.

**Still Incorrect:**

```rust
/// Marker trait for Eq impls that do not panic.
///
/// # Safety
/// Behavior is undefined if any of the methods of `Eq` panic.
pub unsafe trait NoPanicEq: Eq {}

pub fn remove_all<T: NoPanicEq>(
    vec: &mut Vec<T>,
    target: &T,
) {
    // same as before
}
```

In this case, the line

```rust
drop(unsafe { ptr::read(&vec[read_idx]) });
```

in the `else` block may still panic.  And in this case we should consider ourselves fortunate that the drop is even visible!  Most drops will be invisible, hidden at the end of a scope.

Many of these problems can be solved through extremely liberal use of `std::mem::ManuallyDrop`; basically, whenever you own a `T` or a container of `T`s, put it in a `std::mem::ManuallyDrop` so that it won't drop on unwind.  Then you only need to worry about the ones you don't own (anything your function receives by `&mut` reference).

<a id="pointer-alignment"></a>

Pointer alignment
-----------------

**Things to look for:** Code that parses `&[u8]` into references of other types.

**Summary:** Any attempt to convert a `*const T` into a `&T` (or to call `std::ptr::read`) requires an aligned pointer, in addition to all the other, more obvious requirements.

<a id="uninitialized"></a>

Generic usage of `std::mem::uninitialized` or `std::mem::zeroed`
----------------------------------------------------------------

**Things to look for:** Usage of either `std::mem::uninitialized` or `std::mem::zeroed` in a function with a generic type parameter `T`.

**Summary:**  Sometimes people try to use `std::mem::uninitialized` as a substitute for `T::default()` in cases where they cannot add a `T: Default` bound.  This usage is **almost always incorrect** due to multiple edge cases.

### Danger: `T` may have a destructor

Yep, these functions are yet another instance of our mortal enemy, `Drop` unsafety.

**Incorrect**

```rust
fn call_function<T>(
    func: impl FnOnce() -> T,
) -> T {
    let mut out: T;
    out = unsafe { std::mem::uninitialized() };
    out = func(); // <----
    out
}
```

This function exhibits UB because, at the marked line, the original, uninitialized value assigned to `out` is dropped.

**Still Incorrect**

```rust
fn call_function<T>(
    func: impl FnOnce() -> T,
) -> T {
    let mut out: T;
    out = unsafe { std::mem::uninitialized() };
    unsafe { std::ptr::write(&mut out, func()) };
    out
}
```

This function *still* exhibits UB because `func()` can panic, causing the uninitialized value assigned to `out` to be dropped during unwind.

### Danger: `T` may be uninhabited

**_Still_ incorrect!!**

```rust
fn call_function<T: Copy>(
    func: impl FnOnce() -> T,
) -> T {
    let mut out: T;
    out = unsafe { std::mem::uninitialized() };
    out = func(); 
    out
}
```

Here, the `Copy` bound forbids `T` from having a destructor, so we no longer have to worry about drops.  However, this function still exhibits undefined behavior in the case where `T` is uninhabited.

```rust
/// A type that is impossible to construct.
#[derive(Copy, Clone)]
enum Never {}

fn main() {
    call_function::<Never>(|| panic!("Hello, world!"));
}
```

The problem here is that `std::mem::uninitialized::<Never>` successfully returns a value of a type that cannot possibly exist.

Or at least, it used to.  Recent versions of the standard library (early rust `1.3x`) include an explicit check for uninitialized types inside `std::mem::{uninitialized, zeroed}`, and these functions will now panic with a nice error message.

### How about `std::mem::MaybeUninit`?

This new type (on the road to stabilization in 1.36.0) has none of the issues listed above.

* Dropping a `MaybeUninit` does not run destructors.
* The type `MaybeUninit<T>` is always inhabited even if `T` is not.

This makes it significantly safer.
