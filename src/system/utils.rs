use std::{task::{Context, Poll}, pin::Pin};
use tokio::sync::mpsc;
use tokio_stream::Stream;

pin_project_lite::pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct UnboundedReceiverStream<T> {
        #[pin]
        inner: mpsc::UnboundedReceiver<T>,
    }
}

impl<T> UnboundedReceiverStream<T> {
    pub fn new(inner: mpsc::UnboundedReceiver<T>) -> Self {
        UnboundedReceiverStream { inner: inner }
    }
}

impl<T> Stream for UnboundedReceiverStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_recv(cx)
    }
}

pin_project_lite::pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct ReceiverStream<T> {
        #[pin]
        inner: mpsc::Receiver<T>,
    }
}

impl<T> ReceiverStream<T> {
    pub fn new(inner: mpsc::Receiver<T>) -> Self {
        ReceiverStream { inner: inner }
    }
}

impl<T> Stream for ReceiverStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_recv(cx)
    }
}
