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

pub async fn get_emote_png_gif_url(query: &str) -> Result<String, reqwest::Error> {
    let mut h = HeaderMap::new();
    h.append("content-type", "application/json".parse().unwrap());
    let c = ClientBuilder::new().default_headers(h).build().unwrap();
    let q = if query.len() >= 24 {
        &query[query.chars().count() - 24..]
    } else {
        query
    };
    let explicit_id = q.chars().all(|c| c.is_ascii_hexdigit());
    let req_body = if explicit_id {
        format!(
            r#"{{"operationName":"Emote","variables":{{"id":"{q}"}},"query":"query Emote($id: ObjectID!) {{ emote(id: $id) {{ id animated name }} }} " }}"#
        )
    } else {
        let (e, q) = if q.chars().count() >= 3
            && ((q.starts_with('"') && q.ends_with('"'))
                || (q.starts_with('\'') && q.ends_with('\'')))
        {
            ("true", q.trim_matches(|c| c == '"' || c == '\''))
        } else {
            ("false", q)
        };
        format!(
            r#"{{"operationName":"SearchEmotes","variables":{{"query":"{q}","limit":1,"filter":{{"exact_match":{e},"case_sensitive":{e},"ignore_tags":true}}}},"query":"query SearchEmotes($query: String!, $limit: Int, $filter: EmoteSearchFilter) {{ emotes(query: $query, limit: $limit, filter: $filter) {{ items {{ id animated name }} }} }} " }}"#
        )
    };
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
