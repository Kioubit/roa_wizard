use std::cell::Cell;
use std::error::Error;
use std::fs::{File, read_dir};
use std::io;
use std::io::BufRead;
use std::net::IpAddr;
use std::path::Path;
use cidr_utils::cidr::{IpCidr, Ipv4Cidr, Ipv6Cidr};
use json::JsonValue;

extern crate cidr_utils;


type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub fn evaluate_filter_set(object_list: &mut Vec<RouteObject>, filter_set: &Vec<FilterSet>, is_v6: bool) {
    object_list.retain(|v| {
        let mut filter_set_iter = filter_set.iter();
        let mut bits: u8 = 0;
        let applicable_filter_set = filter_set_iter.find(|f| {
            if is_v6 {
                if f.prefix.contains(IpAddr::V6(v.prefix_v6.unwrap().first_as_ipv6_addr())) && f.prefix.contains(IpAddr::V6(v.prefix_v6.unwrap().last_as_ipv6_addr())) {
                    bits = v.prefix_v6.unwrap().get_bits();
                    return true;
                }
            } else {
                if f.prefix.contains(IpAddr::V4(v.prefix_v4.unwrap().first_as_ipv4_addr())) && f.prefix.contains(IpAddr::V4(v.prefix_v4.unwrap().last_as_ipv4_addr())) {
                    bits = v.prefix_v4.unwrap().get_bits();
                    return true;
                }
            }
            return false;
        });


        if applicable_filter_set.is_none() {
            return false;
        }

        if !applicable_filter_set.unwrap().allow {
            return false;
        }

        let obj_max_length = v.max_length.get();
        let max_allowed_max_length = applicable_filter_set.unwrap().max_len;
        let applicable_max_length: i32;

        if obj_max_length.is_none() {
            v.max_length.set(Some(max_allowed_max_length));
            applicable_max_length = max_allowed_max_length;
        } else {
            if v.origins.len() == 1  && v.origins.get(0).unwrap_or(&"".to_owned()) == "0" {
                v.max_length.set(Some(max_allowed_max_length));
                return true;
            }
            if obj_max_length.unwrap() > max_allowed_max_length {
                return false;
            }
            if obj_max_length.unwrap() < applicable_filter_set.unwrap().min_len {
                return false;
            }
            applicable_max_length = obj_max_length.unwrap();
        }

        if (bits as i32) > applicable_max_length {
            return false;
        }

        //if (bits as i32) < applicable_filter_set.unwrap().min_len {
        // Expected
        //   return false;
        //}
        return true;
    })
}


#[derive(Debug)]
pub struct FilterSet {
    priority: i32,
    allow: bool,
    prefix: IpCidr,
    min_len: i32,
    max_len: i32,
}

impl FilterSet {
    fn new(priority: Option<&str>, allow: Option<&str>, prefix: Option<&str>, min_len: Option<&str>, max_len: Option<&str>) -> BoxResult<Self> {
        let result = Self {
            priority: priority.ok_or("priority value missing")?.parse::<i32>()?,
            allow: allow.ok_or("allow value missing")? == "permit",
            prefix: IpCidr::from_str(prefix.ok_or("invalid prefix")?)?,
            min_len: min_len.ok_or("min_len value missing")?.parse::<i32>()?,
            max_len: max_len.ok_or("max_len value missing")?.parse::<i32>()?,
        };
        Ok(result)
    }
}

pub fn read_filter_set(file: String) -> BoxResult<Vec<FilterSet>> {
    let mut set: Vec<FilterSet> = Vec::new();
    let lines = read_lines(&file)?;
    for line in lines {
        let line_owned = line?.to_owned();
        if line_owned.starts_with("#") || line_owned.is_empty() {
            continue;
        }
        let mut entries_iter = line_owned.split_whitespace().into_iter();
        let priority = entries_iter.next();
        let allow = entries_iter.next();
        let prefix = entries_iter.next();
        let min_len = entries_iter.next();
        let max_len = entries_iter.next();

        let result = FilterSet::new(priority, allow, prefix, min_len, max_len);
        match result {
            Ok(r) => {
                set.push(r)
            }
            Err(err) => {
                eprintln!("Failed to parse filter.txt line: {}", err)
            }
        }
    }

    set.sort_by(|a, b| a.priority.cmp(&b.priority));
    Ok(set)
}


#[derive(Debug)]
pub struct RouteObject {
    prefix_v4: Option<Ipv4Cidr>,
    prefix_v6: Option<Ipv6Cidr>,
    origins: Vec<String>,
    max_length: Cell<Option<i32>>
}

impl RouteObject {
    pub fn display_bird(self) -> String {
        let str_prefix: String;
        if self.prefix_v6.is_some() {
            str_prefix = self.prefix_v6.unwrap().to_string();
        } else {
            str_prefix = self.prefix_v4.unwrap().to_string();
        }
        let mut result: String = "".to_owned();
        for origin in self.origins {
            result.push_str(&*format!("route {prefix} max {max_length} as {origin};\n", prefix = str_prefix,
                                      max_length = self.max_length.get().unwrap(), origin = origin));
        }
        return result;
    }
    pub fn to_json(self) -> Vec<JsonValue> {
        let str_prefix: String;
        if self.prefix_v6.is_some() {
            str_prefix = self.prefix_v6.unwrap().to_string();
        } else {
            str_prefix = self.prefix_v4.unwrap().to_string();
        }
        let mut result :Vec<JsonValue> = Vec::new();
        for origin in self.origins {
            let mut data = JsonValue::new_object();
            data["prefix"] = str_prefix.to_owned().into();
            data["maxLength"] = self.max_length.get().unwrap().into();
            data["asn"] = origin.into();
            result.push(data);
        }
        return result;
    }
}

pub fn read_route_objects<P>(path: P, is_v6: bool) -> BoxResult<Vec<RouteObject>> where P: AsRef<Path> {
    #[derive(Debug)]
    struct RouteObjectBuilder<> {
        filename: String,
        prefix_v4: Option<String>,
        prefix_v6: Option<String>,
        origins: Vec<String>,
        max_length: Option<i32>,
        is_v6: bool,
    }
    impl RouteObjectBuilder {
        fn new(filename: String, is_v6: bool) -> Self {
            Self {
                filename,
                prefix_v4: None,
                prefix_v6: None,
                origins: Vec::new(),
                max_length: None,
                is_v6,
            }
        }
        fn validate_and_build(mut self) -> BoxResult<RouteObject> {
            if self.origins.len() == 0 {
                return Err("missing origin field in object")?;
            }

            for origin in &self.origins {
                if !origin.starts_with("AS") {
                    return Err("Invalid origin field")?;
                }
            }

            self.origins.iter_mut().for_each(|x| {
                *x = (x.replace("AS","")).to_owned();
            });

            for origin in &self.origins {
                if !origin.chars().all(char::is_numeric) {
                    return Err("Invalid origin field")?;
                }
            }

            let mut prefix_v4 = None;
            let mut prefix_v6 = None;

            if self.is_v6 {
                if self.prefix_v6.is_none() {
                    return Err("missing route field in object")?;
                }
                if self.filename.replace("_", "/") != self.prefix_v6.as_deref().unwrap() {
                    return Err("filename does not equal prefix field")?;
                }
                prefix_v6 = Some(Ipv6Cidr::from_str(self.prefix_v6.unwrap())?)
            } else {
                if self.prefix_v4.is_none() {
                    return Err("missing route field in object")?;
                }
                if self.filename.replace("_", "/") != self.prefix_v4.as_deref().unwrap() {
                    return Err("filename does not equal prefix field")?;
                }
                prefix_v4 = Some(Ipv4Cidr::from_str(self.prefix_v4.unwrap())?)
            }


            let result = RouteObject {
                prefix_v6,
                prefix_v4,
                origins: self.origins,
                max_length: Cell::new(self.max_length),
            };
            return Ok(result);
        }
    }

    let mut objects: Vec<RouteObject> = Vec::new();
    let dir = read_dir(path)?;
    for file_result in dir {
        let file = file_result?.path();
        let lines = read_lines(&file)?;
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        let mut object = RouteObjectBuilder::new(filename.to_owned(), is_v6);
        for line in lines {
            if let Some(result) = line?.split_once(":") {
                match result.0.trim() {
                    "route" => { object.prefix_v4 = Some(result.1.trim().to_owned()) }
                    "route6" => { object.prefix_v6 = Some(result.1.trim().to_owned()) }
                    "origin" => { object.origins.push(result.1.trim().to_owned()) }
                    "max-length" => { object.max_length = result.1.trim().to_owned().parse::<i32>().ok() }
                    &_ => {}
                }
            }
        }
        match object.validate_and_build() {
            Ok(result) => {
                objects.push(result);
            }
            Err(err) => {
                eprintln!("For file: {} {}", filename, err)
            }
        }
    };
    Ok(objects)
}


fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>> where P: AsRef<Path> {
    let file = File::open(path)?;
    return Ok(io::BufReader::new(file).lines());
}