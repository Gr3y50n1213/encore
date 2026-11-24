//! Reading part of a remote file with HTTP range requests (Phase 10, docs/adr/core-0019).
//!
//! Sound analysis needs about 30 seconds from the middle of a song. A decoder asks for it as a
//! seekable `Read`, and this turns each read into a small `Range` request on the original file:
//! no transcoding, so the media server only reads and sends bytes, and the song is never
//! downloaded whole. A server that ignores `Range` and answers with the whole file still works:
//! the body is kept and served from memory.
//!
//! `Read` is blocking and the transport is async, so a reader must be used from a blocking
//! thread (`spawn_blocking`), never from inside the runtime's own threads.

use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use url::Url;

use crate::error::SourceError;
use crate::transport::{Method, Request, Transport};

/// One request fetches this much.
pub const BLOCK_BYTES: u64 = 256 * 1024;
/// How many blocks are kept for a decoder that steps back a little.
const KEPT_BLOCKS: usize = 8;
/// A failed or slow request is tried this many more times, waiting a little longer each time, so a
/// busy media server is given room instead of hammered.
const RETRIES: u32 = 2;
const RETRY_WAIT: Duration = Duration::from_millis(600);
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct RangeReader {
    transport: Arc<dyn Transport>,
    url: Url,
    headers: Vec<(String, String)>,
    runtime: Handle,
    len: Option<u64>,
    pos: u64,
    blocks: VecDeque<(u64, Arc<Vec<u8>>)>,
    /// The whole file, when the server ignored `Range`.
    whole: Option<Arc<Vec<u8>>>,
    requests: u32,
    bytes: u64,
}

impl std::fmt::Debug for RangeReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No URL or headers: they carry the token.
        f.debug_struct("RangeReader")
            .field("len", &self.len)
            .field("pos", &self.pos)
            .field("requests", &self.requests)
            .finish_non_exhaustive()
    }
}

impl RangeReader {
    /// A reader over `url`, sending `headers` (the media server's auth) with every request.
    pub fn new(
        transport: Arc<dyn Transport>,
        url: Url,
        headers: Vec<(String, String)>,
        runtime: Handle,
    ) -> RangeReader {
        RangeReader {
            transport,
            url,
            headers,
            runtime,
            len: None,
            pos: 0,
            blocks: VecDeque::new(),
            whole: None,
            requests: 0,
            bytes: 0,
        }
    }

    /// The file's size once a reply has told it.
    pub fn len(&self) -> Option<u64> {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == Some(0)
    }

    /// How many requests were sent and how many bytes came back (for the analysis cost report).
    pub fn cost(&self) -> (u32, u64) {
        (self.requests, self.bytes)
    }

    fn block(&mut self, index: u64) -> io::Result<Arc<Vec<u8>>> {
        if let Some(whole) = &self.whole {
            let start = (index * BLOCK_BYTES).min(whole.len() as u64) as usize;
            let end = ((index + 1) * BLOCK_BYTES).min(whole.len() as u64) as usize;
            return Ok(Arc::new(whole[start..end].to_vec()));
        }
        if let Some((_, b)) = self.blocks.iter().find(|(i, _)| *i == index) {
            return Ok(Arc::clone(b));
        }
        let start = index * BLOCK_BYTES;
        let mut headers = self.headers.clone();
        headers.push((
            "Range".into(),
            format!("bytes={start}-{}", start + BLOCK_BYTES - 1),
        ));
        let mut last: Option<SourceError> = None;
        for attempt in 0..=RETRIES {
            if attempt > 0 {
                std::thread::sleep(RETRY_WAIT * attempt);
            }
            let req = Request {
                method: Method::Get,
                url: self.url.clone(),
                headers: headers.clone(),
                body: None,
                timeout: TIMEOUT,
            };
            self.requests += 1;
            match self.runtime.block_on(self.transport.send(req)) {
                Ok(resp) => {
                    self.bytes += resp.body.len() as u64;
                    return match resp.status {
                        206 => {
                            if let Some(total) = resp.content_range_total {
                                self.len = Some(total);
                            }
                            let body = Arc::new(resp.body);
                            self.keep(index, Arc::clone(&body));
                            Ok(body)
                        }
                        // The server ignored Range and sent everything: keep it and serve from it.
                        200 => {
                            self.len = Some(resp.body.len() as u64);
                            self.whole = Some(Arc::new(resp.body));
                            self.block(index)
                        }
                        // Past the end of the file.
                        416 => Ok(Arc::new(Vec::new())),
                        // Busy or failing: another try, then the error.
                        500..=599 => {
                            last = Some(SourceError::Protocol(format!(
                                "the server answered {}",
                                resp.status
                            )));
                            continue;
                        }
                        other => Err(io::Error::other(format!("the server answered {other}"))),
                    };
                }
                Err(e) => last = Some(e),
            }
        }
        Err(io::Error::other(last.map_or_else(
            || "the request failed".to_owned(),
            |e| e.to_string(),
        )))
    }

    fn keep(&mut self, index: u64, body: Arc<Vec<u8>>) {
        if self.blocks.len() >= KEPT_BLOCKS {
            self.blocks.pop_front();
        }
        self.blocks.push_back((index, body));
    }
}

impl Read for RangeReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.len.is_some_and(|l| self.pos >= l) {
            return Ok(0);
        }
        let index = self.pos / BLOCK_BYTES;
        let block = self.block(index)?;
        let offset = (self.pos - index * BLOCK_BYTES) as usize;
        if offset >= block.len() {
            return Ok(0);
        }
        let n = out.len().min(block.len() - offset);
        out[..n].copy_from_slice(&block[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => {
                if self.len.is_none() {
                    // The first reply says how long the file is.
                    self.block(0)?;
                }
                self.len.and_then(|l| l.checked_add_signed(d))
            }
        };
        let target = target
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        self.pos = target;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::transport::{BoxFuture, Response};
    use std::sync::Mutex;

    /// A server over some bytes: answers `Range` with 206 (or, with `honours` false, ignores it).
    #[derive(Debug)]
    struct Server {
        data: Vec<u8>,
        honours: bool,
        fail_first: Mutex<u32>,
        seen: Mutex<Vec<String>>,
    }

    impl Server {
        fn new(len: usize, honours: bool) -> Arc<Server> {
            Arc::new(Server {
                data: (0..len).map(|i| (i % 251) as u8).collect(),
                honours,
                fail_first: Mutex::new(0),
                seen: Mutex::new(Vec::new()),
            })
        }
    }

    impl Transport for Server {
        fn send(&self, req: Request) -> BoxFuture<'_, Result<Response>> {
            let range = req
                .headers
                .iter()
                .find(|(k, _)| k == "Range")
                .map(|(_, v)| v.clone());
            self.seen
                .lock()
                .unwrap()
                .push(range.clone().unwrap_or_default());
            Box::pin(async move {
                {
                    let mut f = self.fail_first.lock().unwrap();
                    if *f > 0 {
                        *f -= 1;
                        return Ok(Response {
                            status: 503,
                            body: Vec::new(),
                            server_date_utc_ms: None,
                            content_range_total: None,
                        });
                    }
                }
                let total = self.data.len() as u64;
                let Some(range) = range.filter(|_| self.honours) else {
                    return Ok(Response {
                        status: 200,
                        body: self.data.clone(),
                        server_date_utc_ms: None,
                        content_range_total: None,
                    });
                };
                let (a, b) = range.trim_start_matches("bytes=").split_once('-').unwrap();
                let (a, b): (u64, u64) = (a.parse().unwrap(), b.parse().unwrap());
                if a >= total {
                    return Ok(Response {
                        status: 416,
                        body: Vec::new(),
                        server_date_utc_ms: None,
                        content_range_total: None,
                    });
                }
                let end = b.min(total - 1);
                Ok(Response {
                    status: 206,
                    body: self.data[a as usize..=end as usize].to_vec(),
                    server_date_utc_ms: None,
                    content_range_total: Some(total),
                })
            })
        }
    }

    fn reader(server: &Arc<Server>) -> (RangeReader, tokio::runtime::Runtime) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let transport: Arc<dyn Transport> = Arc::clone(server) as Arc<dyn Transport>;
        let r = RangeReader::new(
            transport,
            Url::parse("https://music.example/Audio/t1/stream?static=true").unwrap(),
            vec![],
            rt.handle().clone(),
        );
        (r, rt)
    }

    #[test]
    fn a_seek_into_the_middle_reads_only_what_is_asked() {
        let server = Server::new(10_000_000, true);
        let (mut r, _rt) = reader(&server);
        r.seek(SeekFrom::Start(4_000_000)).unwrap();
        let mut buf = vec![0u8; 700_000];
        r.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], (4_000_000 % 251) as u8);
        assert_eq!(buf[699_999], ((4_000_000 + 699_999) % 251) as u8);
        let (requests, bytes) = r.cost();
        assert!(
            requests <= 4,
            "{requests} requests for 700 KB at 256 KB a block"
        );
        assert!(bytes < 1_100_000, "{bytes} bytes fetched of a 10 MB file");
        assert_eq!(r.len(), Some(10_000_000));
    }

    #[test]
    fn the_end_of_the_file_is_known_and_read_as_the_end() {
        let server = Server::new(300_000, true);
        let (mut r, _rt) = reader(&server);
        let end = r.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(end, 300_000);
        r.seek(SeekFrom::End(-10)).unwrap();
        let mut rest = Vec::new();
        r.read_to_end(&mut rest).unwrap();
        assert_eq!(rest.len(), 10);
        assert_eq!(r.read(&mut [0u8; 4]).unwrap(), 0);
    }

    #[test]
    fn a_block_read_twice_is_fetched_once() {
        let server = Server::new(2_000_000, true);
        let (mut r, _rt) = reader(&server);
        let mut buf = [0u8; 1_000];
        r.read_exact(&mut buf).unwrap();
        r.seek(SeekFrom::Start(100)).unwrap();
        r.read_exact(&mut buf).unwrap();
        assert_eq!(server.seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_server_that_ignores_range_still_works() {
        let server = Server::new(900_000, false);
        let (mut r, _rt) = reader(&server);
        r.seek(SeekFrom::Start(500_000)).unwrap();
        let mut buf = vec![0u8; 1_000];
        r.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], (500_000 % 251) as u8);
        assert_eq!(
            server.seen.lock().unwrap().len(),
            1,
            "everything came in the first reply"
        );
        assert_eq!(r.len(), Some(900_000));
    }

    #[test]
    fn a_busy_server_is_given_a_second_chance() {
        let server = Server::new(1_000_000, true);
        *server.fail_first.lock().unwrap() = 1;
        let (mut r, _rt) = reader(&server);
        let mut buf = [0u8; 100];
        r.read_exact(&mut buf).unwrap();
        assert_eq!(
            server.seen.lock().unwrap().len(),
            2,
            "one 503, then the real reply"
        );
    }

    #[test]
    fn a_server_that_keeps_failing_is_an_error_not_a_hang() {
        let server = Server::new(1_000_000, true);
        *server.fail_first.lock().unwrap() = 99;
        let (mut r, _rt) = reader(&server);
        assert!(r.read(&mut [0u8; 100]).is_err());
        assert_eq!(server.seen.lock().unwrap().len(), (RETRIES + 1) as usize);
    }

    #[test]
    fn debug_never_shows_the_address_or_headers() {
        let server = Server::new(10, true);
        let (r, _rt) = reader(&server);
        let text = format!("{r:?}");
        assert!(
            !text.contains("music.example") && !text.contains("static"),
            "{text}"
        );
    }
}
