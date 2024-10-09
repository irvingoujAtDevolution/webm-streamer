//! # axum-range
//!
//! HTTP range responses for [`axum`][1].
//!
//! Fully generic, supports any body implementing the [`RangeBody`] trait.
//!
//! Any type implementing both [`AsyncRead`] and [`AsyncSeekStart`] can be
//! used the [`KnownSize`] adapter struct. There is also special cased support
//! for [`tokio::fs::File`], see the [`KnownSize::file`] method.
//!
//! [`AsyncSeekStart`] is a trait defined by this crate which only allows
//! seeking from the start of a file. It is automatically implemented for any
//! type implementing [`AsyncSeek`].
//!
//! ```
//! use axum::Router;
//! use axum::routing::get;
//! use axum_extra::TypedHeader;
//! use axum_extra::headers::Range;
//!
//! use tokio::fs::File;
//!
//! use axum_range::Ranged;
//! use axum_range::KnownSize;
//!
//! async fn file(range: Option<TypedHeader<Range>>) -> Ranged<KnownSize<File>> {
//!     let file = File::open("The Sims 1 - The Complete Collection.rar").await.unwrap();
//!     let body = KnownSize::file(file).await.unwrap();
//!     let range = range.map(|TypedHeader(range)| range);
//!     Ranged::new(range, body)
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // build our application with a single route
//!     let _app = Router::<()>::new().route("/", get(file));
//!
//!     // run it with hyper on localhost:3000
//!     #[cfg(feature = "run_server_in_example")]
//!     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//!        .serve(_app.into_make_service())
//!        .await
//!        .unwrap();
//! }
//! ```
//!
//! [1]: https://docs.rs/axum

mod file;
mod stream;

use std::io;
use std::ops::Bound;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::{AcceptRanges, ContentLength, ContentRange, Range};
use axum_extra::TypedHeader;
use tokio::io::{AsyncRead, AsyncSeek};

pub use file::KnownSize;
pub use stream::RangedStream;
use tracing::info;

/// [`AsyncSeek`] narrowed to only allow seeking from start.
pub trait AsyncSeekStart {
    /// Same semantics as [`AsyncSeek::start_seek`], always passing position as the `SeekFrom::Start` variant.
    fn start_seek(self: Pin<&mut Self>, position: u64) -> io::Result<()>;

    /// Same semantics as [`AsyncSeek::poll_complete`], returning `()` instead of the new stream position.
    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
}

impl<T: AsyncSeek> AsyncSeekStart for T {
    fn start_seek(self: Pin<&mut Self>, position: u64) -> io::Result<()> {
        AsyncSeek::start_seek(self, io::SeekFrom::Start(position))
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncSeek::poll_complete(self, cx).map_ok(|_| ())
    }
}

/// An [`AsyncRead`] and [`AsyncSeekStart`] with a fixed known byte size.
pub trait RangeBody: AsyncRead + AsyncSeekStart {
    /// The total size of the underlying file.
    ///
    /// This should not change for the lifetime of the object once queried.
    /// Behaviour is not guaranteed if it does change.
    fn byte_size(&self) -> Option<u64>;

    /// The maximum size of a range request in bytes.
    fn max_size_per_request(&self) -> u64;
}

/// The main responder type. Implements [`IntoResponse`].
pub struct Ranged<B: RangeBody + Send + 'static> {
    range: Option<Range>,
    body: B,
}

impl<B: RangeBody + Send + 'static> Ranged<B> {
    /// Construct a ranged response over any type implementing [`RangeBody`]
    /// and an optional [`Range`] header.
    pub fn new(range: Option<Range>, body: B) -> Self {
        Ranged { range, body }
    }

    /// Responds to the request, returning headers and body as
    /// [`RangedResponse`]. Returns [`RangeNotSatisfiable`] error if requested
    /// range in header was not satisfiable.
    pub fn try_respond(self) -> Result<RangedResponse<B>, RangeNotSatisfiable> {
        let total_bytes = self.body.byte_size(); // Now Option<u64>
        let max_size_per_request = self.body.max_size_per_request();

        // Extract the requested range
        let range = self.range;

        // Determine the start and end positions, and construct Content-Range
        let (seek_start, adjusted_seek_end, content_range) = match (range, total_bytes) {
            (Some(range_header), Some(total_bytes)) => {
                // Total size is known, range is specified
                let satisfiable_range = range_header.satisfiable_ranges(total_bytes).nth(0);
                let (seek_start, seek_end_excl) = match satisfiable_range {
                    Some((Bound::Included(seek_start), Bound::Included(end))) => {
                        (seek_start, end + 1)
                    }
                    Some((Bound::Included(seek_start), Bound::Unbounded)) => {
                        (seek_start, total_bytes)
                    }
                    _ => (0, total_bytes),
                };

                // Validate the requested range
                if seek_start >= seek_end_excl || seek_end_excl > total_bytes {
                    let content_range = ContentRange::unsatisfied_bytes(total_bytes);
                    return Err(RangeNotSatisfiable(content_range));
                }

                // Enforce the maximum size per request
                let requested_length = seek_end_excl - seek_start;
                let actual_length = std::cmp::min(requested_length, max_size_per_request);
                let adjusted_seek_end = seek_start + actual_length;

                let content_range = Some(
                    ContentRange::bytes(seek_start..adjusted_seek_end, total_bytes)
                        .expect("ContentRange::bytes cannot panic in this usage"),
                );

                (seek_start, adjusted_seek_end, content_range)
            }
            (Some(range_header), None) => {
                // we here assume range_header is of form "bytes=0-", i.e. unbounded
                let itr = range_header
                    .satisfiable_ranges(0)
                    .nth(0)
                    .expect("range_header is not empty");

                let (Bound::Included(seek_start), Bound::Unbounded) = itr else {
                    todo!()
                };

                let seek_end_excl = seek_start + max_size_per_request;
                let content_range =
                    Some(ContentRange::bytes(seek_start..seek_end_excl, None).unwrap());
                (seek_start, seek_end_excl, content_range)
            }
            (None, Some(total_bytes)) => {
                // Total size is known, no range specified
                let seek_start = 0;
                let seek_end_excl = total_bytes;

                // Enforce max_size_per_request
                let requested_length = seek_end_excl - seek_start;
                let actual_length = std::cmp::min(requested_length, max_size_per_request);
                let adjusted_seek_end = seek_start + actual_length;

                let content_range = None; // For full content, no Content-Range header

                (seek_start, adjusted_seek_end, content_range)
            }
            (None, None) => {
                // Total size is unknown, no range specified
                let seek_start = 0;
                let adjusted_seek_end = seek_start + max_size_per_request;

                let content_range = None; // No Content-Range header

                (seek_start, adjusted_seek_end, content_range)
            }
        };

        info!(
            seek_start,
            adjusted_seek_end,
            ?content_range,
            "Responding with range"
        );

        // Adjust content length
        let content_length = ContentLength(adjusted_seek_end - seek_start);

        // Build the response
        let stream = RangedStream::new(self.body, seek_start, content_length.0);

        Ok(RangedResponse {
            content_range,
            content_length,
            stream,
        })
    }
}

impl<B: RangeBody + Send + 'static> IntoResponse for Ranged<B> {
    fn into_response(self) -> Response {
        self.try_respond().into_response()
    }
}

/// Error type indicating that the requested range was not satisfiable. Implements [`IntoResponse`].
#[derive(Debug, Clone)]
pub struct RangeNotSatisfiable(pub ContentRange);

impl IntoResponse for RangeNotSatisfiable {
    fn into_response(self) -> Response {
        let status = StatusCode::RANGE_NOT_SATISFIABLE;
        let header = TypedHeader(self.0);
        (status, header, ()).into_response()
    }
}

/// Data type containing computed headers and body for a range response. Implements [`IntoResponse`].
pub struct RangedResponse<B> {
    pub content_range: Option<ContentRange>,
    pub content_length: ContentLength,
    pub stream: RangedStream<B>,
}

impl<B: RangeBody + Send + 'static> IntoResponse for RangedResponse<B> {
    fn into_response(self) -> Response {
        let content_range = self.content_range.map(TypedHeader);
        let content_length = TypedHeader(self.content_length);
        let accept_ranges = TypedHeader(AcceptRanges::bytes());
        let stream = self.stream;

        let status = match content_range {
            Some(_) => StatusCode::PARTIAL_CONTENT,
            None => StatusCode::OK,
        };

        (status, content_range, content_length, accept_ranges, stream).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use axum::http::HeaderValue;
    use axum_extra::headers::{ContentRange, Header, Range};
    use bytes::Bytes;
    use futures::{pin_mut, Stream, StreamExt};
    use tokio::fs::File;

    use crate::axum_range::KnownSize;
    use crate::axum_range::Ranged;

    async fn collect_stream(stream: impl Stream<Item = io::Result<Bytes>>) -> String {
        let mut string = String::new();
        pin_mut!(stream);
        while let Some(chunk) = stream.next().await.transpose().unwrap() {
            string += std::str::from_utf8(&chunk).unwrap();
        }
        string
    }

    fn range(header: &str) -> Option<Range> {
        let val = HeaderValue::from_str(header).unwrap();
        Some(Range::decode(&mut [val].iter()).unwrap())
    }

    async fn body() -> KnownSize<File> {
        let file = File::open("test/fixture.txt").await.unwrap();
        KnownSize::file(file).await.unwrap()
    }

    #[tokio::test]
    async fn test_full_response() {
        let ranged = Ranged::new(None, body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(54, response.content_length.0);
        assert!(response.content_range.is_none());
        assert_eq!(
            "Hello world this is a file to test range requests on!\n",
            &collect_stream(response.stream).await
        );
    }

    #[tokio::test]
    async fn test_partial_response_1() {
        let ranged = Ranged::new(range("bytes=0-29"), body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(30, response.content_length.0);

        let expected_content_range = ContentRange::bytes(0..30, 54).unwrap();
        assert_eq!(Some(expected_content_range), response.content_range);

        assert_eq!(
            "Hello world this is a file to ",
            &collect_stream(response.stream).await
        );
    }

    #[tokio::test]
    async fn test_partial_response_2() {
        let ranged = Ranged::new(range("bytes=30-53"), body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(24, response.content_length.0);

        let expected_content_range = ContentRange::bytes(30..54, 54).unwrap();
        assert_eq!(Some(expected_content_range), response.content_range);

        assert_eq!(
            "test range requests on!\n",
            &collect_stream(response.stream).await
        );
    }

    #[tokio::test]
    async fn test_unbounded_start_response() {
        // unbounded ranges in HTTP are actually a suffix

        let ranged = Ranged::new(range("bytes=-20"), body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(20, response.content_length.0);

        let expected_content_range = ContentRange::bytes(34..54, 54).unwrap();
        assert_eq!(Some(expected_content_range), response.content_range);

        assert_eq!(
            " range requests on!\n",
            &collect_stream(response.stream).await
        );
    }

    #[tokio::test]
    async fn test_unbounded_end_response() {
        let ranged = Ranged::new(range("bytes=40-"), body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(14, response.content_length.0);

        let expected_content_range = ContentRange::bytes(40..54, 54).unwrap();
        assert_eq!(Some(expected_content_range), response.content_range);

        assert_eq!(" requests on!\n", &collect_stream(response.stream).await);
    }

    #[tokio::test]
    async fn test_one_byte_response() {
        let ranged = Ranged::new(range("bytes=30-30"), body().await);

        let response = ranged.try_respond().expect("try_respond should return Ok");

        assert_eq!(1, response.content_length.0);

        let expected_content_range = ContentRange::bytes(30..31, 54).unwrap();
        assert_eq!(Some(expected_content_range), response.content_range);

        assert_eq!("t", &collect_stream(response.stream).await);
    }

    #[tokio::test]
    async fn test_invalid_range() {
        let ranged = Ranged::new(range("bytes=30-29"), body().await);

        let err = ranged
            .try_respond()
            .err()
            .expect("try_respond should return Err");

        let expected_content_range = ContentRange::unsatisfied_bytes(54);
        assert_eq!(expected_content_range, err.0)
    }

    #[tokio::test]
    async fn test_range_end_exceed_length() {
        let ranged = Ranged::new(range("bytes=30-99"), body().await);

        let err = ranged
            .try_respond()
            .err()
            .expect("try_respond should return Err");

        let expected_content_range = ContentRange::unsatisfied_bytes(54);
        assert_eq!(expected_content_range, err.0)
    }

    #[tokio::test]
    async fn test_range_start_exceed_length() {
        let ranged = Ranged::new(range("bytes=99-"), body().await);

        let err = ranged
            .try_respond()
            .err()
            .expect("try_respond should return Err");

        let expected_content_range = ContentRange::unsatisfied_bytes(54);
        assert_eq!(expected_content_range, err.0)
    }
}
