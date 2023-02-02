use std::time::SystemTime;
use crate::parse::RouteObject;

pub fn print_bird(objects : Vec<RouteObject>) {
    println!("# roa_wizard - Kioubit.dn42");
    println!("# Created: {}",get_sys_time_in_secs().to_string());
    for object in objects {
       print!("{}",object.display_bird());
    }
}

pub fn print_json(objects : Vec<RouteObject>) {
    let mut top = json::JsonValue::new_object();
    let mut metadata = json::JsonValue::new_object();

    let mut data = json::JsonValue::new_array();
    let mut count = 0;
    for object in objects {
        for v in object.to_json() {
            data.push(v).expect("Error converting data to JSON");
            count+=1;
        }
    }

    metadata["counts"] = count.into();
    let now = get_sys_time_in_secs();
    metadata["generated"] = now.into();
    metadata["valid"] = (now + 86400).into();

    top["metadata"] = metadata;
    top["roas"] = data;

    println!("{}",top.dump());
}

fn get_sys_time_in_secs() -> u64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}