Concerns for thread synchronization
===================================

<a id="shared-mut-wo-unsafecell"></a>

Shared mutability without `UnsafeCell`
--------------------------------------

**What to look for:** Mutable data that is shared by multiple threads, but isn't
atomic or wrapped in an `UnsafeCell`. Casts from `*const _` to `*mut _`.

**Summary:** Threads usually exchange data by reading and writing to shared
memory locations. But by default, Rust assumes that non-atomic data accessed via
a shared `&` reference cannot change. This assumption must be disabled using an
`UnsafeCell` in objects meant for thread synchronization.

**Incorrect:**

```rust
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    data: T,
    locked: AtomicBool,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Option<&mut T> {
        let was_locked = self.locked.swap(true, Ordering::Acquire);
        if was_locked {
            None
        } else {
            let data_ptr = &self.data as *const _ as *mut _;
            Some(unsafe { &mut *data_ptr })
        }
    }

    pub fn unlock(&self) {
        let was_locked = self.locked.compare_and_swap(true, false, Ordering::Release);
        assert!(was_locked, "Incorrect lock usage detected!");
    }
}
```

**Correct:**

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    cell: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: UnsafeCell::new(data),
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Option<&mut T> {
        let was_locked = self.locked.swap(true, Ordering::Acquire);
        if was_locked {
            None
        } else {
            Some(unsafe { &mut *self.cell.get() })
        }
    }

    pub fn unlock(&self) {
        let was_locked = self.locked.compare_and_swap(true, false, Ordering::Release);
        assert!(was_locked, "Incorrect lock usage detected!");
    }
}
```

<a id="multiple-mut-ref"></a>

Multiple `&mut` to the same data
--------------------------------

**What to look for:** Multiple `&mut`s to a single piece of data, or APIs that
allow creating them.

**Summary:** As seen above, to synchronize threads through shared memory, we
need to cheat Rust's "no shared mutability" rule using `UnsafeCell`. This makes
it easy to accidentally expose an API that allows creating multiple `&mut`s to a
single piece of data, which is Undefined Behavior.

**Incorrect:**

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct RecursiveSpinLock<T> {
    cell: UnsafeCell<T>,
    owner_id: AtomicU32,
}

const NO_THREAD_ID: u32 = 0;
static THREAD_ID_CTR: AtomicU32 = AtomicU32::new(1);
thread_local!(static THREAD_ID: u32 = THREAD_ID_CTR.fetch_add(1, Ordering::Relaxed));

impl<T> RecursiveSpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: UnsafeCell::new(data),
            owner_id: AtomicU32::new(NO_THREAD_ID),
        }
    }

    pub fn try_lock(&self) -> Option<&mut T> {
        THREAD_ID.with(|&my_id| {
            let old_id = self.owner_id.compare_and_swap(NO_THREAD_ID, my_id, Ordering::Acquire);
            if old_id == NO_THREAD_ID || old_id == my_id {
                Some(unsafe { &mut *self.cell.get() })
            } else {
                None
            }
        })
    }

    pub fn unlock(&self) {
        THREAD_ID.with(|&my_id| {
            let old_id = self.owner_id.compare_and_swap(my_id, NO_THREAD_ID, Ordering::Release);
            assert_eq!(old_id, my_id, "Incorrect lock usage detected!");
        })
    }
}
```

Here, a single thread calling `try_lock()` multiple times on a
`RecursiveSpinLock` object will get multiple mutable references to its inner
data, which is illegal in Rust.

If you really need a recursive lock, you will need to make its API return
a shared `&` reference, or to turn it into an unsafe API that returns a raw
`*mut` pointer (possibly wrapped in `NonNull`).

<a id="data-races"></a>

Data races
----------

**What to look for:** One thread writing to a piece of data in a fashion that is
observable by another thread writing to or reading from it.

**Summary:** Even in the presence of an `UnsafeCell`, data races are undefined
behavior. Intuitions of memory accesses based on reading the code may not match
the actual memory access patterns of optimized binaries running on modern
out-of-order CPUs. Please ensure that other threads wait for writes to be
finished before accessing the shared data.

**Incorrect:**

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Racey<T> {
    cell: UnsafeCell<T>,
    writing: AtomicBool,
}

impl<T> Racey<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: UnsafeCell::new(data),
            writing: AtomicBool::new(false),
        }
    }

    pub fn read(&self) -> &T {
        unsafe { &*self.cell.get() }
    }

    pub fn try_write(&self) -> Option<WriteGuard<T>> {
        let was_writing = self.writing.swap(true, Ordering::Acquire);
        if was_writing {
            None
        } else {
            Some(WriteGuard(&self))
        }
    }
}

pub struct WriteGuard<'a, T>(&'a Racey<T>);

impl<'a, T> WriteGuard<'a, T> {
    // Notice the use of &mut self, which prevents multiple &mut T to be created
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.cell.get() }
    }
}

impl<'a, T> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        self.0.writing.store(false, Ordering::Release);
    }
}
```

Although this design correctly prevents multiple writers from acquiring an
`&mut` to the data at the same time (which, as we've seen, is UB even if they
don't use those references), it does not prevents readers from observing the
writes of the writers.

<a id="insufficient-synchronization"></a>

Insufficient synchronization
----------------------------

**What to look for:** Insufficient atomic memory orderings and unforeseen
interleavings of thread operations on shared memory.

**Summary:** Modern optimizing compilers and CPUs will add, remove, and reorder
memory accesses in a fashion that is observable by other threads. It is your
responsability to tell the compiler which of these alterations should be
prevented so that your code remains correct.

**Incorrect:**

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    cell: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: UnsafeCell::new(data),
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Option<&mut T> {
        let was_locked = self.locked.swap(true, Ordering::Relaxed);
        if was_locked {
            None
        } else {
            Some(unsafe { &mut *self.cell.get() })
        }
    }

    pub fn unlock(&self) {
        let was_locked = self.locked.compare_and_swap(true, false, Ordering::Relaxed);
        assert!(was_locked, "Incorrect lock usage detected!");
    }
}
```

Use of `Relaxed` memory ordering means that the compiler and CPU are allowed to
move reads and writes to the lock-protected data before the atomic swap that
acquires the lock or after the atomic CAS that releases the lock. This may
result in data races.

**Correct:**

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    cell: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            cell: UnsafeCell::new(data),
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Option<&mut T> {
        let was_locked = self.locked.swap(true, Ordering::Acquire);
        if was_locked {
            None
        } else {
            Some(unsafe { &mut *self.cell.get() })
        }
    }

    pub fn unlock(&self) {
        let was_locked = self.locked.compare_and_swap(true, false, Ordering::Release);
        assert!(was_locked, "Incorrect lock usage detected!");
    }
}
```

`Acquire` ordering ensures that no reads and writes can be speculatively carried
out on the locked data before the lock has been acquired. `Release` ordering
ensures that all reads and writes to locked data have been flushed to shared
memory before the lock is released.

Together, these memory orderings guarantee that a thread acquiring the lock will
see the inner data as the thread that previously released the lock saw it.