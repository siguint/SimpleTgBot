use sedregex::find_and_replace;
use teloxide::prelude::*;

pub fn sed_request(msg: Message) -> bool {
    if let Some(x) = msg.reply_to_message() {
        return find_and_replace(
            x.text().unwrap_or_default(),
            [msg.text().unwrap_or_default()],
        )
        .is_ok();
    }
    false
}
