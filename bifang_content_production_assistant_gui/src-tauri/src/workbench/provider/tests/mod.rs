use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use super::*;
use bifang_content_production_assistant_core::{
    ImageGenerationParameters, TextGenerationParameters,
};

/// 启动只处理一个请求的本地 HTTP 夹具。
fn serve_once(status: &str, headers: &[(&str, &str)], body: &str) -> String {
    serve_once_capturing(status, headers, body).0
}

/// 启动一个请求夹具，并把完整 HTTP 请求交给测试断言。
fn serve_once_capturing(
    status: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (String, Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = mpsc::channel();
    let status = status.to_string();
    let headers = headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    let body = body.to_string();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let read = stream.read(&mut buffer).unwrap_or(0);
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
        let _ = request_sender.send(request);
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        response.push_str(&body);
        let _ = stream.write_all(response.as_bytes());
    });
    (format!("http://{address}/v1"), request_receiver)
}

/// 返回完整响应头后只发送部分正文并立即断开。
fn serve_truncated_body(body_prefix: &'static str, declared_length: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n{body_prefix}"
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{address}/v1")
}

/// 返回响应头和部分正文后保持连接，触发客户端正文读取超时。
fn serve_stalling_body(body_prefix: &'static str, declared_length: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {declared_length}\r\nConnection: keep-alive\r\n\r\n{body_prefix}"
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        thread::sleep(Duration::from_millis(1_250));
    });
    format!("http://{address}/v1")
}

/// 接收完整请求后不返回响应头，用于模拟 POST 已发出但结果未知。
fn serve_delayed_response() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        thread::sleep(Duration::from_millis(1_250));
    });
    format!("http://{address}/v1")
}

/// 用 chunked 编码返回无 Content-Length 的响应正文。
fn serve_chunked_body(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let headers = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(headers);
        let _ = stream.write_all(format!("{:x}\r\n", body.len()).as_bytes());
        let _ = stream.write_all(&body);
        let _ = stream.write_all(b"\r\n0\r\n\r\n");
    });
    format!("http://{address}/v1")
}

/// 启动一个二进制下载夹具，并把收到的 GET 请求交给断言。
fn serve_binary_once(
    status: &str,
    content_type: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> (String, Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = mpsc::channel();
    let status = status.to_string();
    let content_type = content_type.to_string();
    let headers = headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let read = stream.read(&mut buffer).unwrap_or(0);
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        let _ = request_sender.send(request);
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(&body);
    });
    (
        format!("http://{address}/generated.png?x-tos-signature=test"),
        request_receiver,
    )
}

/// 生成测试使用的完整 Provider 配置。
fn config(base_url: String) -> ProviderConfig {
    ProviderConfig {
        id: "provider".into(),
        name: "测试服务".into(),
        revision: 1,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url,
        text_model: "text-model".into(),
        article_model: Some("article-model".into()),
        summary_model: Some("summary-model".into()),
        image_model: "image-model".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    }
}

mod ark;
mod image;
mod text;
