use futures::TryStreamExt;
use std::env::var;
use teloxide::{
    dispatching::UpdateFilterExt,
    net::Download,
    prelude::*,
    types::{ChatId, FileId, InputFile},
};

#[derive(Clone)]
struct BluePortal(Bot);

fn is_from_owner(msg: &Message, owner_id: ChatId) -> bool {
    msg.from.as_ref().map_or(false, |f| f.id == owner_id)
}

async fn download(bot: &Bot, file_id: String) -> Option<Vec<u8>> {
    let Ok(file) = bot.get_file(FileId(file_id.clone())).await else {
        log::error!("get_file failed for {file_id}");
        return None;
    };
    bot.download_file_stream(&file.path)
        .try_fold(Vec::new(), |mut v, chunk| async move {
            v.extend_from_slice(&chunk);
            Ok(v)
        })
        .await
        .map_err(|e| log::error!("Download failed: {e}"))
        .ok()
}

async fn handle_message(
    orange_bot: Bot,
    msg: Message,
    blue_portal: BluePortal,
    owner_id: ChatId,
) -> ResponseResult<()> {
    if !is_from_owner(&msg, owner_id) {
        return Ok(());
    }

    let bot = &blue_portal.0;
    let cap = msg.caption();

    if let Some(text) = msg.text() {
        bot.send_message(owner_id, text).await?;
    } else if let Some(photo) = msg.photo().and_then(|p| p.last()) {
        if let Some(bytes) = download(&orange_bot, photo.file.id.0.clone()).await {
            let mut req = bot.send_photo(owner_id, InputFile::memory(bytes));
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(doc) = msg.document() {
        if let Some(bytes) = download(&orange_bot, doc.file.id.0.clone()).await {
            let mut file = InputFile::memory(bytes);
            if let Some(name) = &doc.file_name {
                file = file.file_name(name.clone());
            }
            let mut req = bot.send_document(owner_id, file);
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(video) = msg.video() {
        if let Some(bytes) = download(&orange_bot, video.file.id.0.clone()).await {
            let mut req = bot.send_video(owner_id, InputFile::memory(bytes));
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(audio) = msg.audio() {
        if let Some(bytes) = download(&orange_bot, audio.file.id.0.clone()).await {
            let mut req = bot.send_audio(owner_id, InputFile::memory(bytes));
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(voice) = msg.voice() {
        if let Some(bytes) = download(&orange_bot, voice.file.id.0.clone()).await {
            let mut req = bot.send_voice(owner_id, InputFile::memory(bytes));
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(sticker) = msg.sticker() {
        bot.send_sticker(owner_id, InputFile::file_id(sticker.file.id.clone())).await?;
    } else if let Some(animation) = msg.animation() {
        if let Some(bytes) = download(&orange_bot, animation.file.id.0.clone()).await {
            let mut req = bot.send_animation(owner_id, InputFile::memory(bytes));
            if let Some(c) = cap {
                req = req.caption(c);
            }
            req.await?;
        }
    } else if let Some(vn) = msg.video_note() {
        if let Some(bytes) = download(&orange_bot, vn.file.id.0.clone()).await {
            bot.send_video_note(owner_id, InputFile::memory(bytes))
                .await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenvy::dotenv().ok();

    let blue_portal_bot = Bot::new(
        var("BLUE_PORTAL_BOT_TOKEN").expect("BLUE_PORTAL_BOT_TOKEN not set in environment"),
    );
    let orange_portal_bot = Bot::new(
        var("ORANGE_PORTAL_BOT_TOKEN").expect("ORANGE_PORTAL_BOT_TOKEN not set in environment"),
    );
    let owner_id = ChatId(
        var("OWNER_CHAT_ID")
            .expect("OWNER_CHAT_ID not set in environment")
            .parse::<i64>()
            .expect("OWNER_CHAT_ID must be a number"),
    );

    log::info!("Opening portals...");

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(orange_portal_bot, handler)
        .dependencies(dptree::deps![BluePortal(blue_portal_bot), owner_id])
        .build()
        .dispatch()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_message(from_id: i64, extra: serde_json::Value) -> Message {
        let mut base = json!({
            "message_id": 1,
            "date": 0,
            "chat": {"id": from_id, "type": "private"},
            "from": {"id": from_id, "is_bot": false, "first_name": "Test"}
        });
        if let (Some(base_map), serde_json::Value::Object(extra_map)) =
            (base.as_object_mut(), extra)
        {
            base_map.extend(extra_map);
        }
        serde_json::from_value(base).unwrap()
    }

    fn make_message_no_sender(extra: serde_json::Value) -> Message {
        let mut base = json!({
            "message_id": 1,
            "date": 0,
            "chat": {"id": 999, "type": "channel"}
        });
        if let (Some(base_map), serde_json::Value::Object(extra_map)) =
            (base.as_object_mut(), extra)
        {
            base_map.extend(extra_map);
        }
        serde_json::from_value(base).unwrap()
    }

    const OWNER: i64 = 42;
    const STRANGER: i64 = 99;

    #[test]
    fn rejects_non_owner() {
        let msg = make_message(STRANGER, json!({}));
        assert!(!is_from_owner(&msg, ChatId(OWNER)));
    }

    #[test]
    fn rejects_message_with_no_sender() {
        let msg = make_message_no_sender(json!({}));
        assert!(!is_from_owner(&msg, ChatId(OWNER)));
    }

    #[test]
    fn accepts_owner() {
        let msg = make_message(OWNER, json!({}));
        assert!(is_from_owner(&msg, ChatId(OWNER)));
    }
}
