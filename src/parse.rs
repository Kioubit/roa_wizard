use std::cell::Cell;
use std::fs::{File, read_dir};
use std::io;
use std::io::BufRead;
use std::path::Path;
use std::str::FromStr;
use cidr_utils::cidr::IpCidr;
use json::JsonValue;

extern crate cidr_utils;


pub fn evaluate_filter_set(object_list: &mut Vec<RouteObject>, filter_set: &[FilterSet]) {
    object_list.retain(|v| {
        let mut filter_set_iter = filter_set.iter();
        let mut bits: u8 = 0;
        let applicable_filter_set = filter_set_iter.find(|f| {
            if f.prefix.contains(&v.prefix.unwrap().first_address()) && f.prefix.contains(&v.prefix.unwrap().last_address()) {
                bits = v.prefix.unwrap().network_length();
                return true;
            }
            false
        });


        if applicable_filter_set.is_none() {
            return false;
        }

        if !applicable_filter_set.unwrap().allow {
            return false;
        }

        let filter_max_length = applicable_filter_set.unwrap().max_len;
        let filter_min_length = applicable_filter_set.unwrap().min_len;
        let applicable_max_length: i32;


        if let Some(mut obj_max_length) = v.max_length.get(){
            if obj_max_length > filter_max_length {
                obj_max_length = filter_max_length;
                v.max_length.set(Some(filter_max_length));
            }
            if obj_max_length < filter_min_length {
                obj_max_length = filter_min_length;
                v.max_length.set(Some(filter_min_length));
            }
            applicable_max_length = obj_max_length;
        } else {
            v.max_length.set(Some(filter_max_length));
            applicable_max_length = filter_max_length;
        }

        if (bits as i32) > applicable_max_length {
            return false;
        }
        true
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
    fn new(priority: Option<&str>, allow: Option<&str>, prefix: Option<&str>, min_len: Option<&str>, max_len: Option<&str>)  -> Result<Self,String>{
        let result = Self {
            priority: priority.ok_or("priority value missing")?.parse::<i32>().ok().ok_or("Failed to parse priority as i32")?,
            allow: allow.ok_or("allow value missing")? == "permit",
            prefix: IpCidr::from_str(prefix.ok_or("invalid prefix")?).ok().ok_or("Failed to parse prefix")?,
            min_len: min_len.ok_or("min_len value missing")?.parse::<i32>().ok().ok_or("Failed to parse min_length as i32")?,
            max_len: max_len.ok_or("max_len value missing")?.parse::<i32>().ok().ok_or("Failed to parse max_length as i32")?,
        };
        Ok(result)
    }
}

pub fn read_filter_set(file: String) -> Result<(Vec<FilterSet>, Vec<String>),String> {
    let mut warnings: Vec<String> = Vec::new();
    let mut set: Vec<FilterSet> = Vec::new();
    let lines = read_lines(file).map_err(|e|
        format!("Error reading filter set file: {}",e)
    )?;
    for line_result in lines {
        let line = line_result.map_err(|e|
            format!("Error reading filter set line: {}",e)
        )?;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut entries_iter = line.split_whitespace();
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
                let error_message = format!("Failed to parse filter.txt line: {} Error: {}",line, err);
                warnings.push(error_message);
            }
        }
    }

    set.sort_by(|a, b| a.priority.cmp(&b.priority));
    Ok((set,warnings))
}


#[derive(Debug)]
pub struct RouteObject {
    prefix: Option<IpCidr>,
    origins: Vec<String>,
    max_length: Cell<Option<i32>>,
}

impl RouteObject {
    pub fn get_bird_format(self) -> String {
        let mut result: String = "".to_owned();
        for origin in &self.origins {
            result.push_str(&format!("route {prefix} max {max_length} as {origin};\n", prefix = self.prefix.unwrap(),
                                      max_length = self.max_length.get().unwrap(), origin = origin));
        }
        result
    }
    pub fn get_json_objects(self) -> Vec<JsonValue> {
        let mut result :Vec<JsonValue> = Vec::new();
        for origin in &self.origins {
            let mut data = JsonValue::new_object();
            data["prefix"] = self.prefix.unwrap().to_string().to_owned().into();
            data["maxLength"] = self.max_length.get().unwrap().into();
            data["asn"] = origin.to_owned().into();
            result.push(data);
        }
        result
    }
}

pub fn read_route_objects<P>(path: P) -> Result<(Vec<RouteObject>, Vec<String>),String> where P: AsRef<Path> {
    #[derive(Debug)]
    struct RouteObjectBuilder<> {
        filename: String,
        prefix: Option<String>,
        origins: Vec<String>,
        max_length: Option<String>,
    }
    impl RouteObjectBuilder {
        fn new(filename: String) -> Self {
            Self {
                filename,
                prefix: None,
                origins: Vec::new(),
                max_length: None,
            }
        }
        fn validate_and_build(mut self) -> Result<RouteObject,String> {
            if self.origins.is_empty() {
                return Err("missing origin field in object")?;
            }

            for origin in &self.origins {
                if !origin.starts_with("AS") {
                    return Err("Invalid origin field")?;
                }
            }

            self.origins.iter_mut().for_each(|x| {
                *x = x.replace("AS","");
            });

            for origin in &self.origins {
                if !origin.chars().all(char::is_numeric) {
                    return Err("Invalid origin field")?;
                }
            }


            if self.prefix.is_none() {
                return Err("missing route field in object")?;
            }
            if self.filename.replace('_', "/") != self.prefix.as_deref().unwrap() {
                return Err("filename does not equal prefix field")?;
            }
            let prefix = Some(IpCidr::from_str(&self.prefix.unwrap()).map_err(|e|
                format!("Unable to parse IP CIDR: {}", e)
            )?);


            let max_length = self.max_length.map_or(Ok(None), |s|
                if let Ok(parsed) = s.parse::<i32>() {
                    Ok(Some(parsed))
                } else {
                    Err("Failed to parse max_length value as i32")
                }
            )?;

            let result = RouteObject {
                prefix,
                origins: self.origins,
                max_length: Cell::new(max_length),
            };
            Ok(result)
        }
    }

    let mut objects: Vec<RouteObject> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let dir = read_dir(path).map_err(|e|
        format!("Unable to read directory: {}", e)
    )?;
    for file_result in dir {
        let file = file_result.map_err(|e|
            format!("Unable to read directory file: {}", e)
        )?.path();
        let lines = read_lines(&file).map_err(|e|
            format!("Unable to open file: {}", e)
        )?;
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        let mut object = RouteObjectBuilder::new(filename.to_owned());
        for line in lines {
            if let Some(result) = line.map_err(|e|
                format!("Unable to read file line: {}", e)
            )?.split_once(':') {
                match result.0.trim_end() {
                    "route" => { object.prefix = Some(result.1.trim().to_owned()) }
                    "route6" => { object.prefix = Some(result.1.trim().to_owned()) }
                    "origin" => { object.origins.push(result.1.trim().to_owned()) }
                    "max-length" => { object.max_length = Some(result.1.trim().to_owned()) }
                    &_ => {}
                }
            }
        }
        match object.validate_and_build() {
            Ok(result) => {
                objects.push(result);
            }
            Err(err) => {
                let error_message = format!("Error in file: {}: {}", filename, err);
                warnings.push(error_message);
            }
        }
    };
    Ok((objects, warnings))
}


fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>> where P: AsRef<Path> {
    let file = File::open(path)?;
    Ok(io::BufReader::new(file).lines())
}