use std::fs::File;
use std::io::Write;
// use clap::lazy_static::lazy_static;
use scraper::{Html, Selector};
use serde_json::Value;
use crate::tui::SearchResult;
// use google_youtube3::{hyper, hyper_rustls, oauth2, YouTube};

pub(crate) struct YtSearch {
    // client: YouTube<>
}

// impl YtSearch {
//     fn new() -> Self {
//         let secret: oauth2::ApplicationSecret = oauth2::ApplicationSec
//         let auth = oauth2::InstalledFlowAuthenticator::builder(
//
//         );
//         let mut hub = YouTube::new(
//             hyper::Client::builder().build(
//                 hyper_rustls::HttpsConnectorBuilder::new()
//                     .with_native_roots()
//                     .https_or_http()
//                     .enable_http1()
//                     .enable_http2()
//                     .build()
//             )
//         );
//     }
// }

pub(crate) fn search(query: &str) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let response = reqwest::blocking::get(format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAQ%253D%253D",
        urlencoding::encode(&query.replace(" ", "+")),
    ))?
        .text()?;

    let mut f = File::create("1.txt").unwrap();

    let html = Html::parse_document(&response);
    let mut f = File::create("2.txt").unwrap();
    let script_selector = Selector::parse("script").unwrap();
    let mut f = File::create("3.txt").unwrap();

    let data = html.select(&script_selector)
        .filter(|t| t.inner_html().starts_with("var ytInitialData"))
        .map(|t| t.inner_html())
        .next()
        .ok_or("No ytInitialData found")?;

    let mut f = File::create("4.txt").unwrap();

    let data: Value = serde_json::from_str(&data[20..data.len() - 1])?;

    let mut f = File::create("5.txt").unwrap();

    fn parse_video(video: &Value) -> Option<SearchResult> {
        Some(SearchResult {
            title: video.get("title")?
                .get("runs")?
                .as_array()?
                .get(0)?
                .get("text")?
                .as_str()?
                .to_string(),
            uploader: video.get("ownerText")?
                .get("runs")?
                .as_array()?
                .get(0)?
                .get("text")?
                .as_str()?
                .to_string(),
            path: format!(
                "https://www.youtube.com/watch?v={}",
                video.get("navigationEndpoint")?
                    .get("watchEndpoint")?
                    .get("videoId")?
                    .as_str()?
            ),
        })
    }

    // let mut f = File::create("test.json").unwrap();

    Ok(
        data.get("contents")
            .ok_or("contents (0) not found")?
            .get("twoColumnSearchResultsRenderer")
            .ok_or("twoColumnSearchResultsRenderer not found")?
            .get("primaryContents")
            .ok_or("primaryContents not found")?
            .get("sectionListRenderer")
            .ok_or("sectionListRenderer not found")?
            .get("contents")
            .ok_or("contents (1) not found")?
            .as_array()
            .ok_or("not an array (0)")?
            .get(0)
            .ok_or("no elements in array")?
            .get("itemSectionRenderer")
            .ok_or("itemSectionRenderer not found")?
            .get("contents")
            .ok_or("contents (2) not found")?
            .as_array()
            .ok_or("not an array (1)")?
            .iter()
            .flat_map(|v| v.get("videoRenderer"))
            // .inspect(|v| {
            //     f.write(v.to_string().as_bytes());
            // })
            .flat_map(parse_video)
            .collect()
    )
}

