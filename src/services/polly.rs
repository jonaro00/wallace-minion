use aws_sdk_polly::model::{OutputFormat, VoiceId};
use serenity::{client::Context, framework::standard::CommandResult};

use crate::discord::get_polly;

pub async fn to_mp3(ctx: &Context, text: impl Into<String>) -> CommandResult<Vec<u8>> {
    let polly_client = get_polly(ctx).await;

    let resp = polly_client
        .synthesize_speech()
        .output_format(OutputFormat::Pcm)
        // .sample_rate("16000")
        .text(text)
        .voice_id(VoiceId::Brian)
        .send()
        .await
        .map_err(|_| "call to polly failed")?;
    // MP3 data from response
    let blob = resp
        .audio_stream
        .collect()
        .await
        .map_err(|_| "failed to read audio data")?;

    Ok(blob.to_vec())
}
