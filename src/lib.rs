mod parse;
mod output;

use crate::output::{print_bird, print_json};
use crate::parse::{evaluate_filter_set, read_filter_set, read_route_objects, RouteObject};
use std::process::exit;
use std::thread;
use std::thread::JoinHandle;


pub fn generate_bird(base_path: String, strict: bool, is_v6: bool) {
    let objects = process(is_v6, base_path, strict);
    if objects.is_err() {
        eprintln!("Error: {}", objects.unwrap_err());
        exit(1);
    }
    print_bird(objects.unwrap());
}

pub fn generate_json(base_path: String, strict: bool) {
    let handler_v4 = process_handler(false, base_path.to_owned(), strict);
    let handler_v6 = process_handler(true, base_path, strict);
    let mut result_v4 = handler_v4.join().expect("thread failed");
    let result_v6 = handler_v6.join().expect("thread failed");

    if result_v4.is_err() {
        eprintln!("Error: {}", result_v4.unwrap_err());
        exit(1);
    }
    if result_v6.is_err() {
        eprintln!("Error: {}", result_v6.unwrap_err());
        exit(1);
    }
    result_v4
        .as_mut()
        .unwrap()
        .append(result_v6.unwrap().as_mut());
    print_json(result_v4.unwrap());
}

fn process(is_v6: bool, base_path: String, strict: bool) -> Result<Vec<RouteObject>, String> {
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
    let (mut objects, warnings) = read_route_objects(route_directory)?;
    check_warnings(warnings, strict)?;
    let (filters, warnings) = read_filter_set(filter_txt)?;
    check_warnings(warnings, strict)?;

    evaluate_filter_set(objects.as_mut(), filters.as_ref());
    Ok(objects)
}

fn process_handler(is_v6: bool, base_path: String, strict: bool) -> JoinHandle<Result<Vec<RouteObject>, String>> {
    thread::spawn(move || {
        process(is_v6, base_path, strict)
    })
}

fn check_warnings(warnings: Vec<String>, strict: bool) -> Result<(), String> {
    let mut had_warning: bool = false;
    for warning in warnings {
        eprintln!("Warning: {}", warning);
        had_warning = true;
    }
    if strict && had_warning {
        return Err("Warnings occurred and strict mode is enabled".to_string());
    }

    Ok(())
}