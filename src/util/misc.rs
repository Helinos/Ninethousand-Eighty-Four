use serenity::{
    Result as SerenityResult,
    model::channel::Message
};

use std::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;
use fasthash::city;

// Checks that a message was successfully sent; if not, then logs why to stdout.
pub fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

pub fn to_string<T: Display>(a: T) -> String{
    format!("{}", a)
}

pub fn hash(content: &str) -> u128{
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(?m)<(.*?)>|[^a-zA-Z0-9]").unwrap();
    }
    let content = content.to_lowercase();
    let content = RE.replace_all(&content, "").to_string();
    
    city::hash128(content)
}

pub fn seconds_to_string(secs: u64) -> String {
    let mut sstr = String::from("");
    let mut mstr = String::from("");
    let mut hstr = String::from("");
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 60) / 60;
    if s != 0 {
        sstr = format!("{}s", s);
    }
    if m != 0 {
        mstr = format!("{}m ", m);
    }
    if h != 0 {
        hstr = format!("{}h ", h);
    }

    format!("{}{}{}", hstr, mstr, sstr)
}