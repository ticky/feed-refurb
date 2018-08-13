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

  let mut parsed_feed = Channel::read_from(BufReader::new(
    http_client.get(configuration.feed.as_str()).send().unwrap()
  )).unwrap();

  for item in parsed_feed.items_mut().iter_mut() {
    let new_description = match item.link() {
      None => continue,
      Some(url) => {
        let document = scraper::Html::parse_document(&http_client.get(url).send().unwrap().text().unwrap());

        let selected_items: Vec<String> = document.select(&configuration.description_selector.0).map(|i| { i.html() }).collect();

        // TODO: Make sure the URLs in the document are reassociated

        selected_items.join("<br/>")
      }
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
