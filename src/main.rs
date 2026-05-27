use std::env::var;
use teloxide::{dispatching::UpdateFilterExt, prelude::*, types::InputFile};

#[derive(Clone)]
struct BluePortal(Bot);

async fn handle_message(
    _bot: Bot,
    msg: Message,
    blue_portal: BluePortal,
    owner_id: ChatId,
) -> ResponseResult<()> {
    if let Some(from) = &msg.from {
        if from.id != owner_id {
            return Ok(());
        }
    }
    if let Some(text) = msg.text() {
        blue_portal.0.send_message(owner_id, text).await?;
    } else if let Some(doc) = msg.document() {
        let mut req = blue_portal
            .0
            .send_document(owner_id, InputFile::file_id(doc.file.id.clone()));
        if let Some(caption) = msg.caption() {
            req = req.caption(caption);
        }
        req.await?;
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
