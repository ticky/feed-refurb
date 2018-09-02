extern crate feed_refurb;
extern crate rocket;

use rocket::http::Status;
use rocket::local::Client;

#[test]
fn index_page() {
  let client = Client::new(feed_refurb::application()).expect("valid rocket instance");
  let response = client.get("/").dispatch();
  assert_eq!(response.status(), Status::Ok);
}

#[test]
fn not_found() {
  let client = Client::new(feed_refurb::application()).expect("valid rocket instance");
  let response = client
    .get("/not-a-valid-url-in-a-million-years-i-promise")
    .dispatch();
  assert_eq!(response.status(), Status::NotFound);
}
