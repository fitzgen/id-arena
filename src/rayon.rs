extern crate rayon;

use self::rayon::iter::plumbing::{Consumer, Folder, UnindexedConsumer};
use self::rayon::iter::plumbing::{ProducerCallback, Producer};
use self::rayon::prelude::*;
use super::*;

impl<T, A> Arena<T, A>
where
    A: ArenaBehavior,
{
    /// Returns an iterator of shared references which can be used to iterate
    /// over this arena in parallel with the `rayon` crate.
    ///
    /// # Features
    ///
    /// This API requires the `rayon` feature of this crate to be enabled.
    pub fn par_iter(&self) -> ParIter<T, A>
    where
        T: Sync,
        A::Id: Send,
    {
        ParIter {
            arena_id: self.arena_id,
            iter: self.items.par_iter().enumerate(),
            _phantom: PhantomData,
        }
    }

    /// Returns an iterator of mutable references which can be used to iterate
    /// over this arena in parallel with the `rayon` crate.
    ///
    /// # Features
    ///
    /// This API requires the `rayon` feature of this crate to be enabled.
    pub fn par_iter_mut(&mut self) -> ParIterMut<T, A>
    where
        T: Send + Sync,
        A::Id: Send,
    {
        ParIterMut {
            arena_id: self.arena_id,
            iter: self.items.par_iter_mut().enumerate(),
            _phantom: PhantomData,
        }
    }
}

/// A parallel iterator over shared references in an arena.
///
/// See `Arena::par_iter` for more information.
#[derive(Debug)]
pub struct ParIter<'a, T, A>
where
    T: Sync,
{
    arena_id: u32,
    iter: rayon::iter::Enumerate<rayon::slice::Iter<'a, T>>,
    _phantom: PhantomData<fn() -> A>,
}

impl<'a, T, A> ParallelIterator for ParIter<'a, T, A>
where
    T: Sync,
    A: ArenaBehavior,
    A::Id: Send,
{
    type Item = (A::Id, &'a T);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        self.iter.opt_len()
    }
}

impl<'a, T, A> IndexedParallelIterator for ParIter<'a, T, A>
where
    T: Sync,
    A: ArenaBehavior,
    A::Id: Send,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive(consumer)
    }

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        return self.iter.with_producer(Callback::<A, _> {
            callback,
            arena_id: self.arena_id,
            _phantom: PhantomData,
        });

        struct Callback<A, CB> {
            callback: CB,
            arena_id: u32,
            _phantom: PhantomData<fn() -> A>,
        }

        impl<A, T, CB> ProducerCallback<(usize, T)> for Callback<A, CB>
        where
            CB: ProducerCallback<(A::Id, T)>,
            A: ArenaBehavior,
            A::Id: Send,
            T: Send,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = (usize, T)>,
            {
                let producer = AddIdProducer::<A, _> {
                    base,
                    arena_id: self.arena_id,
                    _phantom: PhantomData,
                };
                self.callback.callback(producer)
            }
        }
    }
}

impl<'data, T, A> IntoParallelIterator for &'data Arena<T, A>
    where A: ArenaBehavior,
          A::Id: Send,
          T: Sync,
{
    type Item = (A::Id, &'data T);
    type Iter = ParIter<'data, T, A>;

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

/// A parallel iterator over mutable references in an arena.
///
/// See `Arena::par_iter_mut` for more information.
#[derive(Debug)]
pub struct ParIterMut<'a, T, A>
where
    T: Send + Sync,
{
    arena_id: u32,
    iter: rayon::iter::Enumerate<rayon::slice::IterMut<'a, T>>,
    _phantom: PhantomData<fn() -> A>,
}

impl<'a, T, A> ParallelIterator for ParIterMut<'a, T, A>
where
    T: Send + Sync,
    A: ArenaBehavior,
    A::Id: Send,
{
    type Item = (A::Id, &'a mut T);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        self.iter.opt_len()
    }
}

impl<'a, T, A> IndexedParallelIterator for ParIterMut<'a, T, A>
where
    T: Send + Sync,
    A: ArenaBehavior,
    A::Id: Send,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive(consumer)
    }

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        return self.iter.with_producer(Callback::<A, _> {
            callback,
            arena_id: self.arena_id,
            _phantom: PhantomData,
        });

        struct Callback<A, CB> {
            callback: CB,
            arena_id: u32,
            _phantom: PhantomData<fn() -> A>,
        }

        impl<A, T, CB> ProducerCallback<(usize, T)> for Callback<A, CB>
        where
            CB: ProducerCallback<(A::Id, T)>,
            A: ArenaBehavior,
            A::Id: Send,
            T: Send,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = (usize, T)>,
            {
                let producer = AddIdProducer::<A, _> {
                    base,
                    arena_id: self.arena_id,
                    _phantom: PhantomData,
                };
                self.callback.callback(producer)
            }
        }
    }
}

impl<'data, T, A> IntoParallelIterator for &'data mut Arena<T, A>
    where A: ArenaBehavior,
          A::Id: Send,
          T: Send + Sync,
{
    type Item = (A::Id, &'data mut T);
    type Iter = ParIterMut<'data, T, A>;

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter_mut()
    }
}

/// A parallel iterator over items in an arena.
///
/// See `Arena::into_par_iter` for more information.
#[derive(Debug)]
pub struct IntoParIter<T, A>
where
    T: Send,
{
    arena_id: u32,
    iter: rayon::iter::Enumerate<rayon::vec::IntoIter<T>>,
    _phantom: PhantomData<fn() -> A>,
}

impl<T, A> ParallelIterator for IntoParIter<T, A>
where
    T: Send,
    A: ArenaBehavior,
    A::Id: Send,
{
    type Item = (A::Id, T);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        self.iter.opt_len()
    }
}

impl<T, A> IndexedParallelIterator for IntoParIter<T, A>
where
    T: Send,
    A: ArenaBehavior,
    A::Id: Send,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        let consumer = AddIdConsumer::<A, _> {
            arena_id: self.arena_id,
            consumer,
            _phantom: PhantomData,
        };
        self.iter.drive(consumer)
    }

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        return self.iter.with_producer(Callback::<A, _> {
            callback,
            arena_id: self.arena_id,
            _phantom: PhantomData,
        });

        struct Callback<A, CB> {
            callback: CB,
            arena_id: u32,
            _phantom: PhantomData<fn() -> A>,
        }

        impl<A, T, CB> ProducerCallback<(usize, T)> for Callback<A, CB>
        where
            CB: ProducerCallback<(A::Id, T)>,
            A: ArenaBehavior,
            A::Id: Send,
            T: Send,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = (usize, T)>,
            {
                let producer = AddIdProducer::<A, _> {
                    base,
                    arena_id: self.arena_id,
                    _phantom: PhantomData,
                };
                self.callback.callback(producer)
            }
        }
    }
}

impl<T, A> IntoParallelIterator for Arena<T, A>
    where A: ArenaBehavior,
          A::Id: Send,
          T: Send,
{
    type Item = (A::Id, T);
    type Iter = IntoParIter<T, A>;

    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            arena_id: self.arena_id,
            iter: self.items.into_par_iter().enumerate(),
            _phantom: PhantomData,
        }
    }
}

//  ======================================================================

struct AddIdProducer<A, P> {
    base: P,
    arena_id: u32,
    _phantom: PhantomData<fn() -> A>,
}

impl<A, T, P> Producer for AddIdProducer<A, P>
where
    P: Producer<Item = (usize, T)>,
    A: ArenaBehavior,
    A::Id: Send,
    T: Send,
{
    type Item = (A::Id, T);
    type IntoIter = AddIdIter<A, P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        AddIdIter {
            iter: self.base.into_iter(),
            arena_id: self.arena_id,
            _phantom: PhantomData,
        }
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }

    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        let arena_id = self.arena_id;
        let _phantom = PhantomData;
        (
            AddIdProducer { base: left, arena_id, _phantom },
            AddIdProducer { base: right, arena_id, _phantom },
        )
    }

    fn fold_with<G>(self, folder: G) -> G
    where
        G: Folder<Self::Item>,
    {
        let folder = AddIdFolder::<A, _> {
            base: folder,
            arena_id: self.arena_id,
            _phantom: PhantomData,
        };
        self.base.fold_with(folder).base
    }
}

struct AddIdIter<A, I> {
    arena_id: u32,
    iter: I,
    _phantom: PhantomData<fn() -> A>,
}

impl<A, I, T> Iterator for AddIdIter<A, I>
where
    I: Iterator<Item = (usize, T)>,
    A: ArenaBehavior,
{
    type Item = (A::Id, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(idx, item)| (A::new_id(self.arena_id, idx), item))
    }
}

impl<A, I, T> DoubleEndedIterator for AddIdIter<A, I>
where
    I: DoubleEndedIterator<Item = (usize, T)>,
    A: ArenaBehavior,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(idx, item)| (A::new_id(self.arena_id, idx), item))
    }
}

impl<A, I, T> ExactSizeIterator for AddIdIter<A, I>
where
    I: ExactSizeIterator<Item = (usize, T)>,
    A: ArenaBehavior,
{
    fn len(&self) -> usize {
        self.iter.len()
    }
}

//  ======================================================================

struct AddIdConsumer<A, C> {
    consumer: C,
    arena_id: u32,
    _phantom: PhantomData<fn() -> A>,
}

impl<T, A, C> Consumer<(usize, T)> for AddIdConsumer<A, C>
where
    C: Consumer<(A::Id, T)>,
    A: ArenaBehavior,
{
    type Folder = AddIdFolder<A, C::Folder>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.consumer.split_at(index);
        let arena_id = self.arena_id;
        let _phantom = PhantomData;
        (
            AddIdConsumer {
                consumer: left,
                arena_id,
                _phantom,
            },
            AddIdConsumer {
                consumer: right,
                arena_id,
                _phantom,
            },
            reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        AddIdFolder {
            base: self.consumer.into_folder(),
            arena_id: self.arena_id,
            _phantom: PhantomData,
        }
    }

    fn full(&self) -> bool {
        self.consumer.full()
    }
}

impl<T, A, C> UnindexedConsumer<(usize, T)> for AddIdConsumer<A, C>
where
    C: UnindexedConsumer<(A::Id, T)>,
    A: ArenaBehavior,
    A::Id: Send,
{
    fn split_off_left(&self) -> Self {
        AddIdConsumer {
            consumer: self.consumer.split_off_left(),
            arena_id: self.arena_id,
            _phantom: PhantomData,
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.consumer.to_reducer()
    }
}

struct AddIdFolder<A, F> {
    base: F,
    arena_id: u32,
    _phantom: PhantomData<fn() -> A>,
}

impl<T, A, F> Folder<(usize, T)> for AddIdFolder<A, F>
where
    F: Folder<(A::Id, T)>,
    A: ArenaBehavior,
{
    type Result = F::Result;

    fn consume(self, (idx, item): (usize, T)) -> Self {
        AddIdFolder {
            base: self.base.consume((A::new_id(self.arena_id, idx), item)),
            arena_id: self.arena_id,
            _phantom: PhantomData,
        }
    }

    fn complete(self) -> F::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
