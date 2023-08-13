#![no_std]
#![feature(pattern)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use core::str::pattern::Pattern;

use futures::prelude::*;
use futures::task::LocalSpawnExt;

use mbedtls::ssl::async_io::{AsyncIo, AsyncIoExt, ClosedError};

use sel4_async_network_mbedtls::{mbedtls, TcpSocketWrapper};
use sel4_async_network::{SharedNetwork, TcpSocket, TcpSocketError};
use sel4_async_single_threaded_executor::LocalSpawner;
use sel4_async_timers::SharedTimers;
use tests_capdl_http_server_components_test_cpiofs as cpiofs;

mod client_test;

const PORT: u16 = 80;

const NUM_SIMULTANEOUS_CONNECTIONS: usize = 1000;

pub async fn run_server(
    network_ctx: SharedNetwork,
    timers_ctx: SharedTimers,
    blk_device: impl cpiofs::IO + 'static,
    spawner: LocalSpawner,
) -> ! {
    client_test::run(network_ctx.clone(), timers_ctx.clone()).await;

    let index = cpiofs::Index::create(blk_device).await;

    let server = Rc::new(Server { index });

    for _ in 0..NUM_SIMULTANEOUS_CONNECTIONS {
        let network_ctx = network_ctx.clone();
        let server = server.clone();
        spawner
            .spawn_local(async move {
                loop {
                    let socket = network_ctx.new_tcp_socket();
                    if let Err(err) = server.use_socket(socket).await {
                        log::warn!("error: {err:?}");
                    }
                }
            })
            .unwrap()
    }

    future::pending().await
}

struct Server<T> {
    index: cpiofs::Index<T>,
}

impl<T: cpiofs::IO> Server<T> {
    async fn use_socket(&self, socket: TcpSocket) -> Result<(), ClosedError<TcpSocketError>> {
        let port = PORT;
        let mut conn = TcpSocketWrapper::new(socket);
        conn.inner_mut().accept(port).await?;
        self.handle_connection(&mut conn).await?;
        conn.inner_mut().close().await?;
        Ok(())
    }

    async fn handle_connection<U: AsyncIo>(&self, conn: &mut U) -> Result<(), ClosedError<U::Error>> {
        let mut buf = vec![0; 1024 * 16];
        let mut i = 0;
        loop {
            let n = conn.recv(&mut buf[i..]).await?;
            assert_ne!(n, 0);
            i += n;
            if is_request_complete(&buf[..i]).unwrap_or(false) {
                break;
            }
        }
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut req = httparse::Request::new(&mut headers);
        match req.parse(&buf) {
            Ok(status) => {
                assert!(status.is_complete());
                self.handle_request(conn, req.path.unwrap()).await?;
            }
            Err(err) => {
                log::warn!("error parsing request: {err:?}");
            }
        }
        Ok(())
    }

    async fn handle_request<U: AsyncIo>(
        &self,
        conn: &mut U,
        request_path: &str,
    ) -> Result<(), ClosedError<U::Error>> {
        match self.lookup_request_path(request_path).await {
            RequestPathStatus::Ok { file_path, entry } => {
                let content_type = content_type_from_name(&file_path);
                self.serve_file(conn, content_type, &entry).await?;
            }
            RequestPathStatus::MovedPermanently { location } => {
                self.serve_moved_permanently(conn, &location).await?;
            }
            RequestPathStatus::NotFound => {
                self.serve_not_found(conn).await?;
            }
        }
        Ok(())
    }

    async fn serve_file<U: AsyncIo>(
        &self,
        conn: &mut U,
        content_type: &str,
        entry: &cpiofs::Entry,
    ) -> Result<(), ClosedError<U::Error>> {
        self.start_response_headers(conn, 200, "OK").await?;
        self.send_response_header(conn, "Content-Type", content_type.as_bytes())
            .await?;
        self.send_response_header(
            conn,
            "Content-Length",
            entry.data_size().to_string().as_bytes(),
        )
        .await?;
        self.finish_response_headers(conn).await?;
        {
            let mut buf = vec![0; 2048];
            let mut pos = 0;
            while pos < entry.data_size() {
                let n = buf.len().min(entry.data_size() - pos);
                self.index.read_data(entry, pos, &mut buf[..n]).await;
                conn.send_all(&buf[..n]).await?;
                pos += n;
            }
        }
        Ok(())
    }

    async fn serve_moved_permanently<U: AsyncIo>(
        &self,
        conn: &mut U,
        location: &str,
    ) -> Result<(), ClosedError<U::Error>> {
        let phrase = "Moved Permanently";
        self.start_response_headers(conn, 301, phrase).await?;
        self.send_response_header(conn, "Content-Type", b"text/plain")
            .await?;
        self.send_response_header(
            conn,
            "Content-Length",
            phrase.len().to_string().as_bytes(),
        )
        .await?;
        self.send_response_header(conn, "Location", location.as_bytes())
            .await?;
        self.finish_response_headers(conn).await?;
        conn.send_all(phrase.as_bytes()).await?;
        Ok(())
    }

    async fn serve_not_found<U: AsyncIo>(&self, conn: &mut U) -> Result<(), ClosedError<U::Error>> {
        let phrase = "Not Found";
        self.start_response_headers(conn, 404, phrase).await?;
        self.send_response_header(conn, "Content-Type", b"text/plain")
            .await?;
        self.send_response_header(
            conn,
            "Content-Length",
            phrase.len().to_string().as_bytes(),
        )
        .await?;
        self.finish_response_headers(conn).await?;
        conn.send_all(phrase.as_bytes()).await?;
        Ok(())
    }

    async fn start_response_headers<U: AsyncIo>(
        &self,
        conn: &mut U,
        status_code: usize,
        reason_phrase: &str,
    ) -> Result<(), ClosedError<U::Error>> {
        conn.send_all(b"HTTP/1.1 ").await?;
        conn.send_all(&status_code.to_string().as_bytes()).await?;
        conn.send_all(b" ").await?;
        conn.send_all(reason_phrase.as_bytes()).await?;
        conn.send_all(b"\r\n").await?;
        Ok(())
    }

    async fn send_response_header<U: AsyncIo>(
        &self,
        conn: &mut U,
        name: &str,
        value: &[u8],
    ) -> Result<(), ClosedError<U::Error>> {
        conn.send_all(name.as_bytes()).await?;
        conn.send_all(b": ").await?;
        conn.send_all(value).await?;
        conn.send_all(b"\r\n").await?;
        Ok(())
    }

    async fn finish_response_headers<U: AsyncIo>(&self, conn: &mut U) -> Result<(), ClosedError<U::Error>> {
        conn.send_all(b"\r\n").await?;
        Ok(())
    }

    async fn lookup_request_path(&self, request_path: &str) -> RequestPathStatus {
        if !"/".is_prefix_of(request_path) {
            return RequestPathStatus::NotFound;
        }
        let has_trailing_slash = "/".is_suffix_of(request_path);
        let normalized = request_path.trim_matches('/');
        if normalized.len() == 0 {
            let file_path = "index.html";
            if let Some(location) = self.index.lookup(file_path) {
                let entry = self.index.read_entry(location).await;
                match entry.ty() {
                    cpiofs::EntryType::RegularFile => {
                        return RequestPathStatus::Ok {
                            file_path: file_path.to_owned(),
                            entry,
                        };
                    }
                    _ => {}
                }
            }
        } else {
            if let Some(location) = self.index.lookup(normalized) {
                let entry = self.index.read_entry(location).await;
                match entry.ty() {
                    cpiofs::EntryType::RegularFile => {
                        return RequestPathStatus::Ok {
                            file_path: normalized.to_owned(),
                            entry,
                        };
                    }
                    cpiofs::EntryType::Directory => {
                        if !has_trailing_slash {
                            return RequestPathStatus::MovedPermanently {
                                location: format!("{}/", request_path),
                            };
                        }
                        let normalized_with_index_html = format!("{}/index.html", normalized);
                        if let Some(location) = self.index.lookup(&normalized_with_index_html) {
                            let entry = self.index.read_entry(location).await;
                            return RequestPathStatus::Ok {
                                file_path: normalized_with_index_html,
                                entry,
                            };
                        }
                    }
                    // TODO handle symlinks
                    _ => {}
                }
            }
        }
        RequestPathStatus::NotFound
    }
}

#[derive(Debug)]
enum RequestPathStatus {
    Ok {
        file_path: String,
        entry: cpiofs::Entry,
    },
    MovedPermanently {
        location: String,
    },
    NotFound,
}

fn is_request_complete(buf: &[u8]) -> Result<bool, httparse::Error> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = httparse::Request::new(&mut headers);
    req.parse(buf).map(|status| status.is_complete())
}

fn content_type_from_name(name: &str) -> &'static str {
    for (ext, ty) in MIME_ASSOCS {
        if ext.is_suffix_of(name) {
            return ty;
        }
    }
    DEFAULT_MIME_TYPE
}

const DEFAULT_MIME_TYPE: &str = "application/octet-stream";

const MIME_ASSOCS: &[(&str, &str)] = &[
    (".css", "text/css"),
    (".html", "text/html; charset=utf-8"),
    (".ico", "image/vnd.microsoft.icon"),
    (".jpg", "image/jpeg"),
    (".js", "text/javascript; charset=utf-8"),
    (".mp4", "video/mp4"),
    (".pdf", "application/pdf"),
    (".png", "image/png"),
    (".svg", "image/svg+xml"),
    (".ttf", "font/ttf"),
    (".txt", "text/plain; charset=utf-8"),
    (".woff", "font/woff"),
    (".woff2", "font/woff2"),
    (".zip", "application/zip"),
];
