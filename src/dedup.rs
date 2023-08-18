use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Stream;
use pin_project::pin_project;

#[pin_project]
pub struct Dedup<S, T>
where
    S: Stream<Item = T>,
    T: PartialEq + Clone,
{
    #[pin]
    inner: S,
    last: Option<T>,
}

impl<S, T> Stream for Dedup<S, T>
where
    S: Stream<Item = T>,
    T: PartialEq + Clone,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match (this.inner.poll_next(cx), &mut this.last) {
            (Poll::Ready(Some(next)), Some(last)) => {
                if next == *last {
                    Poll::Pending
                } else {
                    *last = next.clone();
                    Poll::Ready(Some(next))
                }
            }
            (value, _) => value,
        }
    }
}

pub trait DedupExt<T: PartialEq + Clone>: Stream<Item = T> + Sized {
    fn dedup(self) -> Dedup<Self, T> {
        Dedup {
            inner: self,
            last: None,
        }
    }
}

impl<S, T> DedupExt<T> for S
where
    S: Stream<Item = T> + Sized,
    T: PartialEq + Clone,
{
}
