Concerns for FFI
================

<a id="enums"></a>

enums are not FFI-safe
----------------------

**What to look for:** `enum`s appearing in signatures of `extern fn`s.

**Summary:** It is undefined behavior for an `enum` in rust to carry an invalid value.  Therefore, do not make it possible for C code to supply the value of an `enum` type.

**Incorrect:**

```rust
#[repr(u16)]
pub enum Mode {
    Read = 0,
    Write = 1,
}

extern "C" fn rust_from_c(mode: Mode) { ... }
```

**Also incorrect:**
```rust
extern "C" fn c_from_rust(mode: *mut Mode);
fn main() {
    let mut mode = Mode::Read;
    unsafe { c_from_rust(&mut mode); }
}
```

<a id="cstring"></a>

`CString::from_raw`
-------------------

**Things to look for:** Any usage of `CString::{into_raw, from_raw}`.

**Summary:** As documented, `CString::from_raw` recomputes the length by scanning for a null byte.  What it doesn't (currently) mention is that **this length must match the original length.**

I think you'll be hard pressed to find any C API function that mutates a `char *` without changing its length!

**Incorrect**

```rust
extern crate libc;

use std::ffi::{CString, CStr};

fn main() {
    let ptr = CString::new("Hello, world!").unwrap().into_raw();
    let delim = CString::new(" ").unwrap();
    
    let first_word_ptr = unsafe { libc::strtok(ptr, delim.as_ptr()) };
    
    assert_eq!(
        unsafe { CStr::from_ptr(first_word_ptr) },
        &CString::new("Hello,").unwrap()[..],
    );
    
    drop(unsafe { CString::from_raw(ptr) });
}
```

This is incorrect because `strtok` inserts a NUL byte after the comma in `"Hello, world!"`, causing the `CString` to have a different length once it is reconstructed.  As a result, when the CString is freed, it will pass the wrong size to the allocator.

The fix is to never use these methods.  If a C API needs to modify a string, use a `Vec<u8>` buffer instead.

**Correct**

```rust
extern crate libc;

use std::ffi::{CString, CStr};
use libc::c_char;

fn main() {
    let mut buf = CString::new("Hello, world!").unwrap().into_bytes_with_nul();
    let delim = CString::new(" ").unwrap();

    let first_word_ptr = unsafe {
        libc::strtok(buf.as_mut_ptr() as *mut c_char, delim.as_ptr())
    };

    assert_eq!(
        unsafe { CStr::from_ptr(first_word_ptr) },
        &CString::new("Hello,").unwrap()[..],
    );
}
```

<a id="cstring-as-ptr"></a>

### Also: Store a `CString` to a local before calling `as_ptr()`

Just as an aside, there's another footgun here.  If I had written:

**Incorrect:**

```rust
let delim = CString::new(" ").as_ptr();
```

the buffer would have been freed immediately.
