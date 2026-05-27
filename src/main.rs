use std::env::var;
use teloxide::{dispatching::UpdateFilterExt, prelude::*, types::{FileId, InputFile}};

#[derive(Clone)]
struct BluePortal(Bot);

pub enum Action {
    Ignore,
    ForwardText(String),
    ForwardDocument { file_id: FileId, caption: Option<String> },
}

pub fn classify_message(msg: &Message, owner_id: ChatId) -> Action {
    match &msg.from {
        Some(from) if from.id == owner_id => {}
        _ => return Action::Ignore,
    }
    if let Some(text) = msg.text() {
        Action::ForwardText(text.to_string())
    } else if let Some(doc) = msg.document() {
        Action::ForwardDocument {
            file_id: doc.file.id.clone(),
            caption: msg.caption().map(str::to_string),
        }
    } else {
        Action::Ignore
    }
}

async fn handle_message(
    _bot: Bot,
    msg: Message,
    blue_portal: BluePortal,
    owner_id: ChatId,
) -> ResponseResult<()> {
    match classify_message(&msg, owner_id) {
        Action::Ignore => {}
        Action::ForwardText(text) => {
            blue_portal.0.send_message(owner_id, text).await?;
        }
        Action::ForwardDocument { file_id, caption } => {
            let mut req = blue_portal
                .0
                .send_document(owner_id, InputFile::file_id(file_id));
            if let Some(cap) = caption {
                req = req.caption(cap);
            }
            req.await?;
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
    fn ignore_message_from_non_owner() {
        let msg = make_message(STRANGER, json!({"text": "hello"}));
        assert!(matches!(classify_message(&msg, ChatId(OWNER)), Action::Ignore));
    }

    #[test]
    fn ignore_message_with_no_sender() {
        let msg = make_message_no_sender(json!({"text": "hello"}));
        assert!(matches!(classify_message(&msg, ChatId(OWNER)), Action::Ignore));
    }

    #[test]
    fn forward_text_from_owner() {
        let msg = make_message(OWNER, json!({"text": "hello world"}));
        match classify_message(&msg, ChatId(OWNER)) {
            Action::ForwardText(text) => assert_eq!(text, "hello world"),
            _ => panic!("expected ForwardText"),
        }
    }

    #[test]
    fn forward_document_without_caption() {
        let msg = make_message(OWNER, json!({
            "document": {"file_id": "abc123", "file_unique_id": "u1", "file_size": 100}
        }));
        match classify_message(&msg, ChatId(OWNER)) {
            Action::ForwardDocument { file_id, caption } => {
                assert_eq!(file_id.0, "abc123");
                assert!(caption.is_none());
            }
            _ => panic!("expected ForwardDocument"),
        }
    }

    #[test]
    fn forward_document_with_caption() {
        let msg = make_message(OWNER, json!({
            "document": {"file_id": "abc123", "file_unique_id": "u1", "file_size": 100},
            "caption": "my file"
        }));
        match classify_message(&msg, ChatId(OWNER)) {
            Action::ForwardDocument { file_id, caption } => {
                assert_eq!(file_id.0, "abc123");
                assert_eq!(caption.as_deref(), Some("my file"));
            }
            _ => panic!("expected ForwardDocument"),
        }
    }

    #[test]
    fn ignore_unsupported_content_from_owner() {
        let msg = make_message(OWNER, json!({
            "photo": [{"file_id": "p1", "file_unique_id": "u1", "width": 100, "height": 100}]
        }));
        assert!(matches!(classify_message(&msg, ChatId(OWNER)), Action::Ignore));
    }
}
