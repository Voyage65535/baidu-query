#[macro_use]
extern crate log;
extern crate crypto;
extern crate reqwest;
extern crate serde_json as json;
extern crate http_muncher as http;

use json::Value;
use std::io::Read;
use std::fs::File;

pub mod remote {
    use super::*;

    use std::thread;
    use std::sync::mpsc;
    use std::time::Duration;

    use crypto::md5::Md5;
    use crypto::digest::Digest;
    
    use reqwest::{Url, Client, Response};

    const THRD_CNT: usize = 4;
    const REQ_RETRY_CNT: usize = 3;
    const REQ_INTERVAL: Duration = Duration::from_millis(500);

    pub fn query_by_request(phone_path: &str) -> Vec<(u64, Value)> {
        let mut buffer = String::new();
        File::open(phone_path).and_then(|mut fd| fd.read_to_string(&mut buffer)).unwrap();
        
        let phones: Vec<_> = buffer.split('\n').map(|s| s.parse::<u64>()).collect();
        let mut phones: Vec<_> = phones.into_iter().filter_map(|x| x.ok()).collect();

        let (tx, rx) = mpsc::channel();
        for i in 0..THRD_CNT {
            // Fucking idiot split_off
            let len  = phones.len();
            let rest = phones.split_off(len / (THRD_CNT - i));
            let part = phones;
            phones = rest;

            let tx = tx.clone();
            thread::spawn(move || {
                let client = Client::new();
                tx.send(part.into_iter().filter_map(|i| {
                    let mut response = request(&client, i)?;
                    let json: Value = response.json().ok()?;

                    println!("{:?}", (i, &json));
                    thread::sleep(REQ_INTERVAL);
                    Some((i, json))
                }).collect()).unwrap()
            });
            thread::sleep(REQ_INTERVAL);
        }

        drop(tx);
        rx.iter().fold(Vec::new(), |mut r, mut e| { r.append(&mut e); r })
    }

    fn request(client: &Client, i: u64) -> Option<Response> {
        let url = Url::parse_with_params("https://passport.baidu.com/v2/?regphonecheck&apiver=v3", &[
                                        ("phone", &i.to_string()), ("moonshad", &salt(i))]).unwrap();
        let mut response = client.get(url.clone()).send();

        let mut retry = REQ_RETRY_CNT;
        while response.is_err() && retry > 0 {
            response = client.get(url.clone()).send();
            retry -= 1;
        }
        if response.is_err() {
            error!("Request Failed: {:?}", url.as_str());
        }
        response.ok()
    }

    fn salt(phone: u64) -> String {
        let mut md5 = Md5::new();
        md5.input_str(&format!("{}Moonshadow", phone));
        let hash = md5.result_str();
        // Baidu's foolish rule
        hash.replacen("d", "do", 1).replacen("a", "ad", 1)
    }
}

pub mod local {
    use super::*;
    use http::{Parser, ParserHandler};

    struct Handler {
        url:  Vec<u8>,
        body: Vec<u8>,
    }

    impl Handler {
        fn new() -> Self { Self { url:  Vec::new(), body: Vec::new(), } } 
    }

    impl ParserHandler for Handler {
        fn on_url(&mut self, _p: &mut Parser, url: &[u8]) -> bool {
            self.url.extend_from_slice(url);
            true
        }

        fn on_body(&mut self, _p: &mut Parser, body: &[u8]) -> bool {
            self.body.extend_from_slice(body);
            true
        }
    }

    pub fn query_by_file(session_path: &str, max_session: usize, num_width: usize) -> Vec<(u64,Value)> {
        (1..max_session + 1).filter_map(|i| {
            use std::fmt::Write;

            let mut request_path  = String::new();
            let mut response_path = String::new();
            write!(request_path,  "{}/{:02$}_c.txt", session_path, i, num_width).unwrap();
            write!(response_path, "{}/{:02$}_s.txt", session_path, i, num_width).unwrap();


            let url = String::from_utf8(http_parse(&request_path).url).unwrap();
            if url.starts_with("https://passport.baidu.com/v2/?regphonecheck") {
                let mut raw = String::from_utf8(http_parse(&response_path).body).unwrap();
                let json = json::from_str(strip(&mut raw)).unwrap();
                Some((phone(&url), json))
            } else { None }
        }).collect()
    }

    fn http_parse(path: &str) -> Handler {
        let mut ret = Handler::new();
        let mut parser = Parser::request_and_response();

        let mut buffer = Vec::new();
        File::open(path).and_then(|mut fd| fd.read_to_end(&mut buffer)).unwrap();

        parser.parse(&mut ret, buffer.as_mut_slice());
        if parser.has_error() {
            panic!("HTTP Parsing Error: {}", parser.error_description())
        }
        ret
    }

    fn strip(s: &mut str) -> &str {
        s.split('(').last().and_then(|r| r.split(')').next()).unwrap()
    }

    fn phone(url: &str) -> u64 {
        url.split('&').filter_map(|s| {
            if s.starts_with("phone=") { Some(s[6..].parse::<u64>().unwrap()) } else { None }
        }).next().unwrap()
    }
}


