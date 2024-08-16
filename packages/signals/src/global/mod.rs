use dioxus_core::{
    prelude::{provide_root_context, try_consume_context},
    ScopeId,
};
use std::{any::Any, cell::RefCell, collections::HashMap, panic::Location, rc::Rc};

mod memo;
pub use memo::*;

mod signal;
pub use signal::*;

use crate::Signal;

/// A trait for an item that can be constructed from an initialization function
pub trait InitializeFromFunction<T> {
    /// Create an instance of this type from an initialization function
    fn initialize_from_function(f: fn() -> T) -> Self;
}

impl<T> InitializeFromFunction<T> for T {
    fn initialize_from_function(f: fn() -> T) -> Self {
        f()
    }
}

/// A lazy value that is created once per application and can be accessed from anywhere in that application
pub struct LazyGlobal<T, R = T> {
    constructor: fn() -> R,
    key: GlobalKey<'static>,
    phantom: std::marker::PhantomData<fn() -> T>,
}

impl<T: Clone + 'static, R> LazyGlobal<T, R>
where
    T: InitializeFromFunction<R>,
{
    #[track_caller]
    /// Create a new global value
    pub const fn new(constructor: fn() -> R) -> Self {
        let key = std::panic::Location::caller();
        Self {
            constructor,
            key: GlobalKey::new(key),
            phantom: std::marker::PhantomData,
        }
    }

    /// Create this global signal with a specific key.
    /// This is useful for ensuring that the signal is unique across the application and accessible from
    /// outside the application too.
    #[track_caller]
    pub const fn with_key(constructor: fn() -> R, key: &'static str) -> Self {
        Self {
            constructor,
            key: GlobalKey::new_from_str(key),
            phantom: std::marker::PhantomData,
        }
    }

    /// Get the key for this global
    pub fn key(&self) -> GlobalKey<'static> {
        self.key.clone()
    }

    /// Resolve the global value. This will try to get the existing value from the current virtual dom, and if it doesn't exist, it will create a new one.
    // NOTE: This is not called "get" or "value" because those methods overlap with Readable and Writable
    pub fn resolve(&self) -> T {
        let key = self.key();

        let context = get_global_context();

        let read = context.signal.borrow();
        match read.get(&key) {
            Some(signal) => signal.downcast_ref::<T>().cloned().unwrap(),
            None => {
                drop(read);
                // Constructors are always run in the root scope
                let signal =
                    ScopeId::ROOT.in_runtime(|| T::initialize_from_function(self.constructor));
                context
                    .signal
                    .borrow_mut()
                    .insert(key, Box::new(signal.clone()));
                signal
            }
        }
    }

    /// Get the scope the signal was created in.
    pub fn origin_scope(&self) -> ScopeId {
        ScopeId::ROOT
    }
}

/// The context for global signals
#[derive(Clone)]
pub struct GlobalSignalContext {
    signal: Rc<RefCell<HashMap<GlobalKey<'static>, Box<dyn Any>>>>,
}

/// A key used to identify a signal in the global signal context
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlobalKey<'a> {
    // We create an extra wrapper around location so we can construct it manually for hot reloading
    file: &'a str,
    line: u32,
    column: u32,
}

impl<'a> GlobalKey<'a> {
    /// Create a new key from a location
    pub const fn new(key: &'a Location<'a>) -> Self {
        GlobalKey {
            file: key.file(),
            line: key.line(),
            column: key.column(),
        }
    }

    /// Create a new key from a static string
    #[allow(unused)]
    pub const fn new_from_str(key: &'a str) -> Self {
        GlobalKey {
            file: key,
            line: 0,
            column: 0,
        }
    }
}

impl From<&'static str> for GlobalKey<'static> {
    fn from(key: &'static str) -> Self {
        Self::new_from_str(key)
    }
}

impl From<&'static Location<'static>> for GlobalKey<'static> {
    fn from(key: &'static Location<'static>) -> Self {
        Self::new(key)
    }
}

impl GlobalSignalContext {
    /// Get a signal with the given string key
    /// The key will be converted to a UUID with the appropriate internal namespace
    pub fn get_signal_with_key<T>(&self, key: &str) -> Option<Signal<T>> {
        let key = GlobalKey::new_from_str(key);

        self.signal.borrow().get(&key).map(|f| {
            *f.downcast_ref::<Signal<T>>().unwrap_or_else(|| {
                panic!(
                    "Global signal with key {:?} is not of the expected type. Keys are {:?}",
                    key,
                    self.signal.borrow().keys()
                )
            })
        })
    }
}

/// Get the global context for signals
pub fn get_global_context() -> GlobalSignalContext {
    match try_consume_context() {
        Some(context) => context,
        None => {
            let context = GlobalSignalContext {
                signal: Rc::new(RefCell::new(HashMap::new())),
            };
            provide_root_context(context)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that keys of global signals are correctly generated and different from one another.
    /// We don't want signals to merge, but we also want them to use both string IDs and memory addresses.
    #[test]
    fn test_global_keys() {
        // we're using consts since it's harder than statics due to merging - these won't be merged
        const MYSIGNAL: GlobalSignal<i32> = GlobalSignal::new(|| 42);
        const MYSIGNAL2: GlobalSignal<i32> = GlobalSignal::new(|| 42);
        const MYSIGNAL3: GlobalSignal<i32> = GlobalSignal::with_key(|| 42, "custom-keyed");

        let a = MYSIGNAL.key();
        let b = MYSIGNAL.key();
        let c = MYSIGNAL.key();
        assert_eq!(a, b);
        assert_eq!(b, c);

        let d = MYSIGNAL2.key();
        assert_ne!(a, d);

        let e = MYSIGNAL3.key();
        assert_ne!(a, e);
    }
}
