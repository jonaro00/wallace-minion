use std::error::Error;

use reqwest::header::{HeaderMap, CONTENT_TYPE};
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
    name: String,
}

pub async fn get_emote_name_url(q: &str) -> Result<(String, String), Box<dyn Error + Sync + Send>> {
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
        return Err("Query is empty.".into());
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
    h.append(CONTENT_TYPE, "application/json".parse().unwrap());
    let c = ClientBuilder::new().default_headers(h).build().unwrap();
    let r: StvResponse = c
        .post("https://7tv.io/v3/gql")
        .body(req_body)
        .send()
        .await
        .map_err(|_| "Request to 7TV failed.")?
        .json()
        .await
        .map_err(|_| "Invalid API response.")?;
    let emote = match r.data {
        StvData::StvEmoteData(e) => e.emote,
        StvData::StvEmotesData(e) => match e.emotes.items.first() {
            Some(i) => i,
            None => return Err("No emote found.".into()),
        }
        .to_owned(),
    };
    let emote_id = &emote.id;
    let file_type = if emote.animated { "gif" } else { "png" };
    Ok((emote.name, format!(
        "https://cdn.7tv.app/emote/{emote_id}/4x.{file_type}",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn catjam() {
        let (name, url) = get_emote_name_url("catjam").await.unwrap();
        assert_eq!(name, "catJAM");
        assert!(url.starts_with("https://"));
        assert!(url.contains("60ae7316f7c927fad14e6ca2"));
        assert!(url.ends_with(".gif"));
    }
    #[tokio::test]
    async fn catjam_quoted() {
        let (name, url) = get_emote_name_url("\"catJAM\"").await.unwrap();
        assert_eq!(name, "catJAM");
        assert!(url.starts_with("https://"));
        assert!(url.contains("60ae7316f7c927fad14e6ca2"));
        assert!(url.ends_with(".gif"));
    }
    #[tokio::test]
    async fn sadge() {
        let (name, url) = get_emote_name_url("sadge").await.unwrap();
        assert_eq!(name, "Sadge");
        assert!(url.starts_with("https://"));
        assert!(url.contains("603cac391cd55c0014d989be"));
        assert!(url.ends_with(".png"));
    }
    #[tokio::test]
    async fn no_query() {
        assert!(get_emote_name_url("").await.is_err());
    }
    #[tokio::test]
    async fn too_long_query() {
        let s = "omegalul".repeat(30);
        assert!(get_emote_name_url(&s).await.is_err());
    }
    #[tokio::test]
    async fn invalid_query() {
        assert!(get_emote_name_url(" ").await.is_err());
        assert!(get_emote_name_url("\"\"\"").await.is_err());
        assert!(get_emote_name_url("\"\'\"").await.is_err());
        assert!(get_emote_name_url("\\").await.is_err());
        assert!(get_emote_name_url("\n").await.is_err());
    }
    #[tokio::test]
    async fn not_found() {
        assert!(get_emote_name_url("somethingthatwillprobablyneverexist").await.is_err());
    }
}
