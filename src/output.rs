use crate::parse::RouteObject;
use std::time::SystemTime;


pub fn output_bird(objects: Vec<RouteObject>) -> String {
    let mut result = format!("# roa_wizard {} - Kioubit.dn42\n", crate::VERSION);
    result.push_str(&format!("# Created: {}\n", get_sys_time_in_secs()));
    for object in objects {
        result.push_str(&object.get_bird_format());
    }
    result
}

pub fn output_json(objects: Vec<RouteObject>) -> String {
    let mut top = json::JsonValue::new_object();
    let mut metadata = json::JsonValue::new_object();

    let mut data = json::JsonValue::new_array();
    let mut count = 0;
    for object in objects {
        for v in object.get_json_objects() {
            data.push(v).expect("Error converting data to JSON");
            count += 1;
        }
    }

    metadata["counts"] = count.into();
    let now = get_sys_time_in_secs();
    metadata["generated"] = now.into();
    metadata["valid"] = (now + 604800).into(); // 7 days

    top["metadata"] = metadata;
    top["roas"] = data;

    top.dump()
}

fn get_sys_time_in_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("SystemTime before UNIX EPOCH").as_secs()
}
