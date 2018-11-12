//! [![](https://docs.rs/id-arena/badge.svg)](https://docs.rs/id-arena/)
//! [![](https://img.shields.io/crates/v/id-arena.svg)](https://crates.io/crates/id-arena)
//! [![](https://img.shields.io/crates/d/id-arena.svg)](https://crates.io/crates/id-arena)
//! [![Travis CI Build Status](https://travis-ci.org/fitzgen/id-arena.svg?branch=master)](https://travis-ci.org/fitzgen/id-arena)
//!
//! A simple, id-based arena.
//!
//! ## Id-based
//!
//! Allocate objects and get an identifier for that object back, *not* a
//! reference to the allocated object. Given an id, you can get a shared or
//! exclusive reference to the allocated object from the arena. This id-based
//! approach is useful for constructing mutable graph data structures.
//!
//! If you want allocation to return a reference, consider [the `typed-arena`
//! crate](https://github.com/SimonSapin/rust-typed-arena/) instead.
//!
//! ## No Deletion
//!
//! This arena does not support deletion, which makes its implementation simple
//! and allocation fast. If you want deletion, you need a way to solve the ABA
//! problem. Consider using [the `generational-arena`
//! crate](https://github.com/fitzgen/generational-arena) instead.
//!
//! ## Homogeneous
//!
//! This crate's arenas can only contain objects of a single type `T`. If you
//! need an arena of objects with heterogeneous types, consider another crate.
//!
//! ## `#![no_std]` Support
//!
//! Requires the `alloc` nightly feature. Disable the on-by-default `"std"` feature:
//!
//! ```toml
//! [dependencies.id-arena]
//! version = "1"
//! default-features = false
//! ```
//!
//! ## Example
//!
//! ```rust
//! use id_arena::{Arena, Id};
//!
//! type AstNodeId = Id<AstNode>;
//!
//! #[derive(Debug, Eq, PartialEq)]
//! pub enum AstNode {
//!     Const(i64),
//!     Var(String),
//!     Add {
//!         lhs: AstNodeId,
//!         rhs: AstNodeId,
//!     },
//!     Sub {
//!         lhs: AstNodeId,
//!         rhs: AstNodeId,
//!     },
//!     Mul {
//!         lhs: AstNodeId,
//!         rhs: AstNodeId,
//!     },
//!     Div {
//!         lhs: AstNodeId,
//!         rhs: AstNodeId,
//!     },
//! }
//!
//! let mut ast_nodes = Arena::new();
//!
//! // Create the AST for `a * (b + 3)`.
//! let three = ast_nodes.alloc(AstNode::Const(3));
//! let b = ast_nodes.alloc(AstNode::Var("b".into()));
//! let b_plus_three = ast_nodes.alloc(AstNode::Add {
//!     lhs: b,
//!     rhs: three,
//! });
//! let a = ast_nodes.alloc(AstNode::Var("a".into()));
//! let a_times_b_plus_three = ast_nodes.alloc(AstNode::Mul {
//!     lhs: a,
//!     rhs: b_plus_three,
//! });
//!
//! // Can use indexing to access allocated nodes.
//! assert_eq!(ast_nodes[three], AstNode::Const(3));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

// In no-std mode, use the alloc crate to get `Vec`.
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc))]

#[cfg(feature = "std")]
mod imports {
    pub use std::fmt;
    pub use std::hash::{Hash, Hasher};
    pub use std::iter;
    pub use std::marker::PhantomData;
    pub use std::ops;
    pub use std::slice;
    pub use std::sync::atomic::{self, AtomicUsize, ATOMIC_USIZE_INIT};
}

#[cfg(not(feature = "std"))]
mod imports {
    extern crate alloc;
    pub use self::alloc::vec::Vec;
    pub use core::fmt;
    pub use core::hash::{Hash, Hasher};
    pub use core::iter;
    pub use core::marker::PhantomData;
    pub use core::ops;
    pub use core::slice;
    pub use core::sync::atomic::{self, AtomicUsize, ATOMIC_USIZE_INIT};
}

use imports::*;

/// An identifier for an object allocated within an arena.
pub struct Id<T> {
    idx: usize,
    arena_id: usize,
    _ty: PhantomData<*const T>,
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Id").field("idx", &self.idx).finish()
    }
}

impl<T> Copy for Id<T> {}

impl<T> Clone for Id<T> {
    #[inline]
    fn clone(&self) -> Id<T> {
        *self
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.idx == rhs.idx
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.arena_id.hash(h);
        self.idx.hash(h);
    }
}

static ARENA_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;

/// An arena of objects of type `T`.
///
/// ```
/// let mut arena = id_arena::Arena::new();
///
/// let a = arena.alloc("Albert");
/// assert_eq!(arena[a], "Albert");
///
/// arena[a] = "Alice";
/// assert_eq!(arena[a], "Alice");
/// ```
#[derive(Debug)]
pub struct Arena<T> {
    arena_id: usize,
    items: Vec<T>,
}

impl<T> Default for Arena<T> {
    #[inline]
    fn default() -> Arena<T> {
        Arena {
            arena_id: ARENA_COUNTER.fetch_add(1, atomic::Ordering::SeqCst),
            items: Vec::new(),
        }
    }
}

impl<T> Arena<T> {
    /// Construct a new, empty `Arena`.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// arena.alloc(42);
    /// ```
    #[inline]
    pub fn new() -> Arena<T> {
        Default::default()
    }

    /// Allocate `item` within this arena and return its id.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// arena.alloc(42);
    /// ```
    #[inline]
    pub fn alloc(&mut self, item: T) -> Id<T> {
        let arena_id = self.arena_id;
        let idx = self.items.len();
        self.items.push(item);
        Id {
            arena_id,
            idx,
            _ty: PhantomData,
        }
    }

    /// Get a shared reference to the object associated with the given `id` if
    /// it exists.
    ///
    /// If there is no object associated with `id` (for example, it might
    /// reference an object allocated within a different arena) then return
    /// `None`.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// let id = arena.alloc(42);
    /// assert!(arena.get(id).is_some());
    ///
    /// let other_arena = id_arena::Arena::new();
    /// assert!(other_arena.get(id).is_none());
    /// ```
    #[inline]
    pub fn get(&self, id: Id<T>) -> Option<&T> {
        if id.arena_id != self.arena_id {
            None
        } else {
            self.items.get(id.idx)
        }
    }

    /// Get an exclusive reference to the object associated with the given `id`
    /// if it exists.
    ///
    /// If there is no object associated with `id` (for example, it might
    /// reference an object allocated within a different arena) then return
    /// `None`.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// let id = arena.alloc(42);
    /// assert!(arena.get_mut(id).is_some());
    ///
    /// let mut other_arena = id_arena::Arena::new();
    /// assert!(other_arena.get_mut(id).is_none());
    /// ```
    #[inline]
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        if id.arena_id != self.arena_id {
            None
        } else {
            self.items.get_mut(id.idx)
        }
    }

    /// Iterate over this arena's items and their ids.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// arena.alloc("hello");
    /// arena.alloc("hi");
    /// arena.alloc("yo");
    ///
    /// for (id, s) in arena.iter() {
    ///     assert_eq!(arena.get(id).unwrap(), s);
    ///     println!("{:?} -> {}", id, s);
    /// }
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        IntoIterator::into_iter(self)
    }

    /// Get the number of objects allocated in this arena.
    ///
    /// ```
    /// let mut arena = id_arena::Arena::new();
    /// arena.alloc("hello");
    /// arena.alloc("hi");
    ///
    /// assert_eq!(arena.len(), 2);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

impl<T> ops::Index<Id<T>> for Arena<T> {
    type Output = T;

    fn index(&self, id: Id<T>) -> &T {
        assert_eq!(self.arena_id, id.arena_id);
        &self.items[id.idx]
    }
}

impl<T> ops::IndexMut<Id<T>> for Arena<T> {
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        assert_eq!(self.arena_id, id.arena_id);
        &mut self.items[id.idx]
    }
}

/// An iterator over `(Id<T>, &T)` pairs in an arena.
///
/// See [the `Arena::iter()` method](./struct.Arena.html#method.iter) for details.
#[derive(Debug)]
pub struct Iter<'a, T: 'a> {
    arena_id: usize,
    iter: iter::Enumerate<slice::Iter<'a, T>>,
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = (Id<T>, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(idx, item)| {
            let arena_id = self.arena_id;
            (
                Id {
                    arena_id,
                    idx,
                    _ty: PhantomData,
                },
                item,
            )
        })
    }
}

impl<'a, T> IntoIterator for &'a Arena<T> {
    type Item = (Id<T>, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        Iter {
            arena_id: self.arena_id,
            iter: self.items.iter().enumerate(),
        }
    }
}
