mod output;
mod parse;

use crate::output::{print_bird, print_json};
use crate::parse::{evaluate_filter_set, read_filter_set, read_route_objects, RouteObject, STRICT_MODE};
use std::process::exit;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn show_usage() {
    println!("roa_wizard {}", VERSION);
    println!("Usage: <path to registry root> <action> [flag]");
    println!();
    println!("Where <action>:");
    println!("'v4' : bird2 v4 format");
    println!("'v6' : bird2 v6 format");
    println!("'json' : json format");
    println!();
    println!("Where <flag>:");
    println!("'' : No flag");
    println!("'strict' : Abort program if an error was found in a file");
    exit(2)
}

fn main() {
    if std::env::args().len() < 3 {
        println!("Missing commandline arguments");
        show_usage();
    }


    let base_path = std::env::args().nth(1).expect("no registry path given");
    let action = std::env::args().nth(2).expect("no action given");
    let strict = std::env::args().nth(3).unwrap_or_default() == "strict";
    STRICT_MODE.store(strict, Ordering::Relaxed);


    match action.as_str() {
        "v4" => {
            let objects = process(false, base_path);
            if objects.is_err() {
                eprintln!("{}", objects.unwrap_err());
                exit(1);
            }
            print_bird(objects.unwrap());
        }
        "v6" => {
            let objects = process(true, base_path);
            if objects.is_err() {
                eprintln!("{}", objects.unwrap_err());
                exit(1);
            }
            print_bird(objects.unwrap());
        }
        "json" => {
            let handler_v4 = process_handler(false,base_path.to_owned());
            let handler_v6 = process_handler(true,base_path);
            let mut result_v4 = handler_v4.join().expect("thread failed");
            let result_v6 = handler_v6.join().expect("thread failed");

            if result_v4.is_err() {
                eprintln!("{}", result_v4.unwrap_err());
                exit(1);
            }
            if result_v6.is_err() {
                eprintln!("{}", result_v6.unwrap_err());
                exit(1);
            }
            result_v4
                .as_mut()
                .unwrap()
                .append(result_v6.unwrap().as_mut());
            print_json(result_v4.unwrap());
        }
        _ => {
            println!("unknown argument for <action>");
            show_usage();
        }
    }
}

fn process(is_v6: bool, base_path: String) -> Result<Vec<RouteObject>, String> {
    let route_directory: String;
    let filter_txt: String;
    match is_v6 {
        true => {
            let route6_directory = base_path.to_owned() + "data/route6/";
            let filter6_txt = base_path + "data/filter6.txt";
            route_directory = route6_directory;
            filter_txt = filter6_txt;
        }
        false => {
            let route4_directory = base_path.to_owned() + "data/route/";
            let filter4_txt = base_path + "data/filter.txt";
            route_directory = route4_directory;
            filter_txt = filter4_txt;
        }
    }
    let mut objects = read_route_objects(route_directory, is_v6)?;
    let filters = read_filter_set(filter_txt);
    evaluate_filter_set(objects.as_mut(), filters?.as_ref(), is_v6);
    Ok(objects)
}

fn process_handler(is_v6: bool, base_path: String) -> JoinHandle<Result<Vec<RouteObject>,String>>{
    thread::spawn(move || {
        process(is_v6,base_path)
    })
}