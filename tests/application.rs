extern crate feed_refurb;
extern crate mockito;
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

fn mock_get_request(path: &str, content_type: &str, body: &str) -> mockito::Mock {
  mockito::mock("GET", path)
    .with_header("content-type", content_type)
    .with_body(body)
    .create()
}

#[test]
fn refurb() {
  let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
  let feed_path = "/feed.rss";
  let article_path = "/articles/latest-cool-article-123";

  let feed_request = mock_get_request(
    feed_path,
    "application/xml",
    &format!(
      r#"<?xml version="1.0" encoding="UTF-8" ?>
  <rss version="2.0">
  <channel>
    <title>My Test Feed</title>
    <link>{host}</link>
    <description>Test feed for testing purposes!</description>
    <language>en-us</language>
    <item>
      <title><![CDATA[My Latest Cool Article]]></title>
      <description><![CDATA[bad and not good article summary, click for more]]></description>
      <link>{host}{article}</link>
      <author>webmaster@{hostname}</author>
      <pubDate>Fri, 31 Aug 2018 03:59:52 +0000</pubDate>
      <guid>{host}{article}</guid>
    </item>
  </channel>
</rss>
"#,
      hostname = mockito::SERVER_ADDRESS,
      host = server_host,
      article = article_path,
    ),
  );

  let article_request = mock_get_request(article_path, "text/html", &r#"<!DOCTYPE html>
<head>
  <title>My Latest Cool Article</title>
  <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
  <nav>
    <a href="/">Home</a>
    <a href="/articles">Articles</a>
  </nav>
  <section>
    <h1>My Latest Cool Article</h1>
    <img class="main-image" src="/images/latest-cool-article-123-main.jpg" />
    <article>
      <p>Here is my latest cool article. It's very good and full of cool and useful information.</p>
      <p>My RSS feed is truncated, so you'd better click my links! I'd better see you in Google Analytics.</p>
    </article>
  </section>
</body>"#);

  let client = Client::new(feed_refurb::application()).expect("valid rocket instance");
  let mut response = client
    .get(format!(
      "/refurb?feed={}{}&description_selector=.main-image,article",
      server_host, feed_path
    )).dispatch();

  assert_eq!(response.status(), Status::Ok);
  assert_eq!(
    response.body_string().expect("processed feed response body"),
    format!(
      r#"<rss version="2.0"><channel><title>My Test Feed</title><link>{host}</link><description>Test feed for testing purposes!</description><language>en-us</language><item><title>My Latest Cool Article</title><link>{host}{article}</link><description><![CDATA[<img class="main-image" src="/images/latest-cool-article-123-main.jpg"><br><article>
      <p>Here is my latest cool article. It's very good and full of cool and useful information.</p>
      <p>My RSS feed is truncated, so you'd better click my links! I'd better see you in Google Analytics.</p>
    </article>]]></description><author>webmaster@{hostname}</author><guid>{host}{article}</guid><pubDate>Fri, 31 Aug 2018 03:59:52 +0000</pubDate></item></channel></rss>"#,
      hostname = mockito::SERVER_ADDRESS,
      host = server_host,
      article = article_path,
    )
  );

  feed_request.assert();
  article_request.assert();
}
