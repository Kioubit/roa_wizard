mod output;
mod parse;

use std::error::Error;
use std::process::exit;
use std::thread;
use std::thread::JoinHandle;
use crate::output::{print_bird, print_json};
use crate::parse::{evaluate_filter_set, read_filter_set, read_route_objects, RouteObject};



fn show_usage() {
    println!("Usage: <path to registry root> <action>");
    println!("Where <action>:");
    println!("'v4' : bird2 v4 format");
    println!("'v6' : bird2 v6 format");
    println!("'json' : json format");
    exit(2)
}

fn main() {
    if std::env::args().len() == 1 {
        show_usage();
    }

    let base_path = std::env::args().nth(1).expect("no registry path given");
    let action =  std::env::args().nth(2).expect("no action given");

    match action.as_str() {
        "v4" => {
            print_bird(process(false, base_path));
        },
        "v6" => {
            print_bird(process(true, base_path));
        },
        "json" => {
            let handler_v4 = process_handler(false, base_path.to_owned());
            let handler_v6 = process_handler(true, base_path.to_owned());
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
            result_v4.as_mut().unwrap().append(result_v6.unwrap().as_mut());
            print_json(result_v4.unwrap());
        },
        _ => {
            println!("Second argument is unknown");
            show_usage();
        },
    }
}


fn process(is_v6: bool, base_path: String) -> Vec<RouteObject> {
    let handler = process_handler(is_v6, base_path.to_owned());
    let result = handler.join().expect("thread failed");
    if result.is_err() {
        eprintln!("{}", result.unwrap_err());
        exit(1);
    }
    return result.unwrap();
}

fn process_handler(is_v6: bool, base_path: String) -> JoinHandle<Result<Vec<RouteObject>, Box<dyn Error + Send + Sync>>> {
    let route_directory :String;
    let filter_txt: String;

    match is_v6 {
        true => {
            let route6_directory = base_path.to_owned() + "data/route6/";
            let filter6_txt = base_path.to_owned() + "data/filter6.txt";
            route_directory = route6_directory;
            filter_txt = filter6_txt;
        }
        false => {
            let route4_directory = base_path.to_owned() + "data/route/";
            let filter4_txt = base_path.to_owned() + "data/filter.txt";
            route_directory = route4_directory;
            filter_txt = filter4_txt;
        }
    }

    thread::spawn(move || {
        let thread_filter_set_parse = thread::spawn(move || {
            read_filter_set(filter_txt)
        });
        let mut objects = read_route_objects(route_directory, is_v6)?;
        let filters= thread_filter_set_parse.join().expect("evaluate_filters thread fail");
        evaluate_filter_set(objects.as_mut(), filters?.as_ref(), is_v6);
        Ok(objects)
    })
}
