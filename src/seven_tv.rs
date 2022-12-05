use std::error::Error;

use reqwest::header::HeaderMap;
use reqwest::ClientBuilder;
use serde::Deserialize;

#[derive(Deserialize)]
struct StvResponse {
    data: StvData,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum StvData {
    StvEmoteData(StvEmoteData),
    StvEmotesData(StvEmotesData),
}
#[derive(Deserialize)]
struct StvEmotesData {
    emotes: StvEmotes,
}
#[derive(Deserialize)]
struct StvEmoteData {
    emote: StvItem,
}
#[derive(Deserialize)]
struct StvEmotes {
    items: Vec<StvItem>,
}
#[derive(Deserialize, Clone)]
struct StvItem {
    id: String,
    animated: bool,
    // name: String,
}

pub async fn get_emote_png_gif_url(q: &str) -> Result<String, Box<dyn Error + Sync + Send>> {
    if q.chars().count() > 200 {
        return Err("Query too long.".into());
    };
    let (exact, q) = if q.chars().count() >= 2
        && ((q.starts_with('"') && q.ends_with('"')) || (q.starts_with('\'') && q.ends_with('\'')))
    {
        let mut s = q.to_string();
        s.pop();
        s.remove(0);
        ("true", s)
    } else {
        ("false", q.to_owned())
    };
    if q.is_empty() {
        return Err("Query is emtpy.".into());
    };
    if q.chars().any(|c| c == '"' || c == '\\') || !q.chars().all(|c| c.is_ascii_graphic()) {
        return Err("Query contains invalid characters.".into());
    };
    let explicit_id = q.chars().count() == 24 && q.chars().all(|c| c.is_ascii_hexdigit());
    let req_body = if explicit_id {
        format!(
            r#"{{"operationName":"Emote","variables":{{"id":"{q}"}},"query":"query Emote($id: ObjectID!) {{ emote(id: $id) {{ id animated name }} }} " }}"#
        )
    } else {
        format!(
            r#"{{"operationName":"SearchEmotes","variables":{{"query":"{q}","limit":1,"filter":{{"exact_match":{exact},"case_sensitive":{exact},"ignore_tags":true}}}},"query":"query SearchEmotes($query: String!, $limit: Int, $filter: EmoteSearchFilter) {{ emotes(query: $query, limit: $limit, filter: $filter) {{ items {{ id animated name }} }} }} " }}"#
        )
    };
    let mut h = HeaderMap::new();
    h.append("content-type", "application/json".parse().unwrap());
    let c = ClientBuilder::new().default_headers(h).build().unwrap();
    let r: StvResponse = c
        .post("https://7tv.io/v3/gql")
        .body(req_body)
        .send()
        .await?
        .json()
        .await?;
    let emote = match r.data {
        StvData::StvEmoteData(e) => e.emote,
        StvData::StvEmotesData(e) => e.emotes.items.first().unwrap().to_owned(),
    };
    let emote_id = &emote.id;
    let file_type = if emote.animated { "gif" } else { "png" };
    Ok(format!(
        "https://cdn.7tv.app/emote/{emote_id}/4x.{file_type}",
    ))
}
