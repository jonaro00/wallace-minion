use aws_sdk_polly::model::{OutputFormat, TextType, VoiceId};
use serenity::{client::Context, framework::standard::CommandResult};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::discord::{get_comprehend, get_polly};

#[derive(Debug, Display, EnumString, EnumIter)]
pub enum PollyLanguage {
    #[strum(serialize = "en")]
    English,
    #[strum(serialize = "ar")]
    Arabic,
    #[strum(serialize = "zh")]
    Chinese,
    #[strum(serialize = "da")]
    Danish,
    #[strum(serialize = "nl")]
    Dutch,
    #[strum(serialize = "fr")]
    French,
    #[strum(serialize = "de")]
    German,
    #[strum(serialize = "hi")]
    Hindi,
    #[strum(serialize = "is")]
    Icelandic,
    #[strum(serialize = "it")]
    Italian,
    #[strum(serialize = "ja")]
    Japanese,
    #[strum(serialize = "ko")]
    Korean,
    #[strum(serialize = "no")]
    Norwegian,
    #[strum(serialize = "pl")]
    Polish,
    #[strum(serialize = "pt")]
    Portugese,
    #[strum(serialize = "ro")]
    Romainian,
    #[strum(serialize = "ru")]
    Russian,
    #[strum(serialize = "es")]
    Spanish,
    #[strum(serialize = "sv")]
    Swedish,
    #[strum(serialize = "tr")]
    Turkish,
    #[strum(serialize = "cy")]
    Welsh,
}

impl PollyLanguage {
    pub fn flag(&self) -> &str {
        use PollyLanguage::*;
        match self {
            English => "🇬🇧",
            Arabic => "🇸🇦",
            Chinese => "🇨🇳",
            Danish => "🇩🇰",
            Dutch => "🇳🇱",
            French => "🇫🇷",
            German => "🇩🇪",
            Hindi => "🇮🇳",
            Icelandic => "🇮🇸",
            Italian => "🇮🇹",
            Japanese => "🇯🇵",
            Korean => "🇰🇷",
            Norwegian => "🇳🇴",
            Polish => "🇵🇱",
            Portugese => "🇧🇷",
            Romainian => "🇷🇴",
            Russian => "🇷🇺",
            Spanish => "🇪🇸",
            Swedish => "🇸🇪",
            Turkish => "🇹🇷",
            Welsh => ":wales:",
        }
    }
    pub fn to_list_string() -> String {
        Self::iter()
            .map(|l| format!("{} {:?} - {}\n", l.flag(), l, l.to_string()))
            .fold(String::new(), |mut a, s| {
                a.push_str(s.as_str());
                a
            })
    }
}

impl Default for PollyLanguage {
    fn default() -> Self {
        Self::English
    }
}

impl Into<VoiceId> for PollyLanguage {
    fn into(self) -> VoiceId {
        use PollyLanguage::*;
        use VoiceId::*;
        match self {
            English => Brian,
            Arabic => Zeina,
            Chinese => Zhiyu,
            Danish => Mads,
            Dutch => Ruben,
            French => Mathieu,
            German => Hans,
            Hindi => Aditi,
            Icelandic => Karl,
            Italian => Giorgio,
            Japanese => Takumi,
            Korean => Seoyeon,
            Norwegian => Liv,
            Polish => Jacek,
            Portugese => Ricardo,
            Romainian => Carmen,
            Russian => Maxim,
            Spanish => Enrique,
            Swedish => Astrid,
            Turkish => Filiz,
            Welsh => Gwyneth,
        }
    }
}

pub async fn to_mp3(
    ctx: &Context,
    text: impl Into<String>,
    lang: Option<PollyLanguage>,
) -> CommandResult<Vec<u8>> {
    let text = text.into();

    let lang = if let Some(l) = lang {
        l
    } else {
        // Use AWS Comprehend to check what language it is
        let comp_client = get_comprehend(ctx).await;
        let resp = comp_client
            .detect_dominant_language()
            .set_text(Some(text.clone()))
            .send()
            .await
            .map_err(|_| "call to comprehend failed")?;
        resp.languages()
            .and_then(|ls| ls.get(0))
            .and_then(|l| l.language_code())
            .unwrap_or_default()
            .parse()
            .unwrap_or_default()
    };

    // Map the language code to respective AWS Polly voice
    let voice = lang.into();

    // TTS the text with that voice
    let polly_client = get_polly(ctx).await;
    let mp3_bytes = polly_client
        .synthesize_speech()
        .output_format(OutputFormat::Mp3)
        .set_text_type(Some(TextType::Ssml))
        .text(format!(
            r#"<speak><prosody rate="125%">{}</prosody></speak>"#,
            text,
        ))
        .voice_id(voice)
        .send()
        .await
        .map_err(|_| "call to polly failed")?
        // MP3 data from response
        .audio_stream
        .collect()
        .await
        .map_err(|_| "failed to read audio data")?
        .to_vec();

    Ok(mp3_bytes)
}
