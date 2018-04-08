extern crate env_logger;
extern crate baidu_query as bq;

use bq::local::query_by_file;
use bq::remote::query_by_request;

use std::env;
use std::fs::File;
use std::io::Write;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 { panic!("Invalid argument!"); }

    let sessions = match args[1].as_str() {
        "r" => query_by_request(&args[2]),
        "l" => {
            if args.len() < 5 { panic!("Invalid argument!"); }
            let num_width   = args[4].parse::<usize>().unwrap();
            let max_session = args[3].parse::<usize>().unwrap();
            query_by_file(&args[2], max_session, num_width)
        },
        _   => panic!("Invalid mode option!")
    };

    let mut omitted = File::create("omitted.txt").unwrap();
    let mut result  = File::create("result.txt").unwrap();

    for (phone, json) in sessions {
        let errcode = &json["errInfo"]["no"];
        if errcode == "0" || errcode == "130020" {
            write!(omitted, "{}\n", phone).unwrap();
        }
        if errcode == "400005" {
            write!(result, "{}\t{}\n", phone, &json["errInfo"]["username"]).unwrap();
        }
    }
}
