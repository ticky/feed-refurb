#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate rss;
extern crate scraper;

use rocket::http::RawStr;
use rocket::Request;
use rocket::request::FromFormValue;
use rocket::response::content::Xml;
use rocket_contrib::Template;
use rss::Channel;
use scraper::Selector;
use std::io::BufReader;

#[get("/")]
fn index() -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", "/");
  Template::render("index", &map)
}

struct CSSSelector(Selector);

impl<'v> FromFormValue<'v> for CSSSelector {
  type Error = &'v RawStr;

  fn from_form_value(form_value: &'v RawStr) -> Result<CSSSelector, &'v RawStr> {
    match form_value.url_decode() {
      Ok(decoded) => {
        match Selector::parse(&decoded) {
          Ok(selector) => Ok(CSSSelector(selector)),
          _ => Err(form_value),
        }
      },
      _ => Err(form_value)
    }
  }
}

#[derive(FromForm)]
struct FeedConfiguration {
  feed: String,
  description_selector: CSSSelector
}

#[get("/refurb?<configuration>")]
fn refurb(configuration: FeedConfiguration) -> Xml<String> {
  let http_client = reqwest::Client::builder().build().unwrap();

  let feed = http_client.get(configuration.feed.as_str()).send().unwrap();
  let feed_buffer = BufReader::new(feed);
  let mut parsed_feed = Channel::read_from(feed_buffer).unwrap();

  for item in parsed_feed.items_mut().iter_mut() {
    let new_description = match item.description() {
      Some(description) => format!("{} (hi!)", description),
      None => String::from("No description? weird!")
    };

    item.set_description(new_description);
  }

  Xml(parsed_feed.to_string())
}

#[catch(404)]
fn not_found(req: &Request) -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", req.uri().as_str());
  Template::render("error/404", &map)
}

fn main() {
  rocket::ignite()
    .attach(Template::fairing())
    .catch(catchers![not_found])
    .mount("/", routes![index, refurb])
    .launch();
}
