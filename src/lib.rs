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
//! let mut ast_nodes = Arena::<AstNode>::new();
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

/// A trait representing the implementation behavior of an arena and how
/// identifiers are represented.
///
/// ## When should I implement `ArenaBehavior` myself?
///
/// Usually, you should just use `DefaultArenaBehavior`, which is simple and
/// correct. However, there are some scenarios where you might want to implement
/// `ArenaBehavior` yourself:
///
/// * **Space optimizations:** The default identifier is two words in size,
/// which is larger than is usually necessary. For example, if you know that an
/// arena *cannot* contain more than 256 items, you could make your own
/// identifier type that stores the index as a `u8` and then you can save some
/// space.
///
/// * **Trait Coherence:** If you need to implement an upstream crate's traits
/// for identifiers, then defining your own identifier type allows you to work
/// with trait coherence rules.
///
/// * **Share identifiers across arenas:** You can coordinate and share
/// identifiers across different arenas to enable a "struct of arrays" style
/// data representation.
pub trait ArenaBehavior {
    /// The identifier type.
    type Id: Copy;

    /// Construct a new object identifier from the given index and arena
    /// identifier.
    ///
    /// ## Panics
    ///
    /// Implementations are allowed to panic if the given index is larger than
    /// the underlying storage (e.g. the implementation uses a `u8` for storing
    /// indices and the given index value is larger than 255).
    fn new_id(index: usize, arena_id: usize) -> Self::Id;

    /// Get the given identifier's index.
    fn index(Self::Id) -> usize;

    /// Get the given identifier's arena id.
    fn arena_id(Self::Id) -> usize;

    /// Construct a new arena identifier.
    ///
    /// This is used to disambiguate `Id`s across different arenas. To make
    /// identifiers with the same index from different arenas compare false for
    /// equality, return a unique `usize` on every invocation. This is the
    /// default, provided implementation's behavior.
    ///
    /// To make identifiers with the same index from different arenas compare
    /// true for equality, return the same `usize` on every invocation.
    fn new_arena_id() -> usize {
        static ARENA_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;
        ARENA_COUNTER.fetch_add(1, atomic::Ordering::SeqCst)
    }
}

/// An identifier for an object allocated within an arena.
pub struct Id<T> {
    idx: usize,
    arena_id: usize,
    _ty: PhantomData<fn() -> T>,
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
    #[inline]
    fn eq(&self, rhs: &Self) -> bool {
        self.arena_id == rhs.arena_id && self.idx == rhs.idx
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.arena_id.hash(h);
        self.idx.hash(h);
    }
}

impl<T> Id<T> {
    /// Get the index within the arena that this id refers to.
    #[inline]
    pub fn index(&self) -> usize {
        self.idx
    }
}

/// The default `ArenaBehavior` implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultArenaBehavior<T> {
    _phantom: PhantomData<fn() -> T>,
}

impl<T> ArenaBehavior for DefaultArenaBehavior<T> {
    type Id = Id<T>;

    #[inline]
    fn new_id(idx: usize, arena_id: usize) -> Self::Id {
        Id {
            idx,
            arena_id,
            _ty: PhantomData,
        }
    }

    #[inline]
    fn index(id: Self::Id) -> usize {
        id.idx
    }

    #[inline]
    fn arena_id(id: Self::Id) -> usize {
        id.arena_id
    }
}

/// An arena of objects of type `T`.
///
/// ```
/// use id_arena::Arena;
///
/// let mut arena = Arena::<&str>::new();
///
/// let a = arena.alloc("Albert");
/// assert_eq!(arena[a], "Albert");
///
/// arena[a] = "Alice";
/// assert_eq!(arena[a], "Alice");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arena<T, A = DefaultArenaBehavior<T>> {
    arena_id: usize,
    items: Vec<T>,
    _phantom: PhantomData<fn() -> A>,
}

impl<T, A> Default for Arena<T, A>
where
    A: ArenaBehavior,
{
    #[inline]
    fn default() -> Arena<T, A> {
        Arena {
            arena_id: A::new_arena_id(),
            items: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<T, A> Arena<T, A>
where
    A: ArenaBehavior,
{
    /// Construct a new, empty `Arena`.
    ///
    /// ```
    /// use id_arena::Arena;
    ///
    /// let mut arena = Arena::<usize>::new();
    /// arena.alloc(42);
    /// ```
    #[inline]
    pub fn new() -> Arena<T, A> {
        Default::default()
    }

    /// Allocate `item` within this arena and return its id.
    ///
    /// ```
    /// use id_arena::{Arena, DefaultArenaBehavior};
    ///
    /// let mut arena = Arena::<usize>::new();
    /// let _id = arena.alloc(42);
    /// ```
    ///
    /// ## Panics
    ///
    /// Panics if the number of elements in the arena overflows a `usize` or
    /// `Id`'s index storage representation.
    #[inline]
    pub fn alloc(&mut self, item: T) -> A::Id {
        let arena_id = self.arena_id;
        let idx = self.items.len();
        self.items.push(item);
        A::new_id(idx, arena_id)
    }

    /// Get a shared reference to the object associated with the given `id` if
    /// it exists.
    ///
    /// If there is no object associated with `id` (for example, it might
    /// reference an object allocated within a different arena) then return
    /// `None`.
    ///
    /// ```
    /// use id_arena::Arena;
    ///
    /// let mut arena = Arena::<usize>::new();
    /// let id = arena.alloc(42);
    /// assert!(arena.get(id).is_some());
    ///
    /// let other_arena = Arena::<usize>::new();
    /// assert!(other_arena.get(id).is_none());
    /// ```
    #[inline]
    pub fn get(&self, id: A::Id) -> Option<&T> {
        if A::arena_id(id) != self.arena_id {
            None
        } else {
            self.items.get(A::index(id))
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
    /// use id_arena::Arena;
    ///
    /// let mut arena = Arena::<usize>::new();
    /// let id = arena.alloc(42);
    /// assert!(arena.get_mut(id).is_some());
    ///
    /// let mut other_arena = Arena::<usize>::new();
    /// assert!(other_arena.get_mut(id).is_none());
    /// ```
    #[inline]
    pub fn get_mut(&mut self, id: A::Id) -> Option<&mut T> {
        if A::arena_id(id) != self.arena_id {
            None
        } else {
            self.items.get_mut(A::index(id))
        }
    }

    /// Iterate over this arena's items and their ids.
    ///
    /// ```
    /// use id_arena::Arena;
    ///
    /// let mut arena = Arena::<&str>::new();
    ///
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
    pub fn iter(&self) -> Iter<T, A> {
        IntoIterator::into_iter(self)
    }

    /// Get the number of objects allocated in this arena.
    ///
    /// ```
    /// use id_arena::Arena;
    ///
    /// let mut arena = Arena::<&str>::new();
    ///
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

impl<T, A> ops::Index<A::Id> for Arena<T, A>
where
    A: ArenaBehavior,
{
    type Output = T;

    #[inline]
    fn index(&self, id: A::Id) -> &T {
        assert_eq!(self.arena_id, A::arena_id(id));
        &self.items[A::index(id)]
    }
}

impl<T, A> ops::IndexMut<A::Id> for Arena<T, A>
where
    A: ArenaBehavior,
{
    #[inline]
    fn index_mut(&mut self, id: A::Id) -> &mut T {
        assert_eq!(self.arena_id, A::arena_id(id));
        &mut self.items[A::index(id)]
    }
}

/// An iterator over `(Id, &T)` pairs in an arena.
///
/// See [the `Arena::iter()` method](./struct.Arena.html#method.iter) for details.
#[derive(Debug)]
pub struct Iter<'a, T: 'a, A: 'a> {
    arena_id: usize,
    iter: iter::Enumerate<slice::Iter<'a, T>>,
    _phantom: PhantomData<fn() -> A>,
}

impl<'a, T: 'a, A: 'a> Iterator for Iter<'a, T, A>
where
    A: ArenaBehavior,
{
    type Item = (A::Id, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(idx, item)| {
            let arena_id = self.arena_id;
            (A::new_id(idx, arena_id), item)
        })
    }
}

impl<'a, T, A> IntoIterator for &'a Arena<T, A>
where
    A: ArenaBehavior,
{
    type Item = (A::Id, &'a T);
    type IntoIter = Iter<'a, T, A>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T, A> {
        Iter {
            arena_id: self.arena_id,
            iter: self.items.iter().enumerate(),
            _phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        struct Foo;
        assert_send_sync::<Id<Foo>>();
    }
}
