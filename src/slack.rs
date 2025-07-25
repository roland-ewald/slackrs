use chrono::prelude::*;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use zip::ZipArchive;

lazy_static! {
    /// The file pattern of the JSON files with the slack messages (there are other JSON files in the export ZIP).
    static ref JSON_FILE_NAME: Regex = Regex::new(r".*\/\d{4}-\d{2}-\d{2}.json$").unwrap();
}

/// Represents a user profile, part of a Slack `Message`.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct UserProfile {
    avatar_hash: String,
    /** URL to avatar image. */
    image_72: String,
    first_name: String,
    real_name: String,
    display_name: String,
    team: String,
    name: String,
    is_restricted: bool,
    is_ultra_restricted: bool,
}

/// Represents a Slack message.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct Message {
    user: Option<String>,
    #[serde(rename = "type")]
    json_type: String,
    ts: String,
    client_msg_id: Option<String>,
    pub text: String,
    team: Option<String>,
    user_team: Option<String>,
    source_team: Option<String>,
    user_profile: Option<UserProfile>,
    thread_ts: Option<String>,
    parent_user_id: Option<String>,
    attachments: Option<Vec<MessageAttachment>>,
    blocks: Option<Vec<MessageBlock>>,
}
impl Message {
    #[cfg(test)]
    fn new(user: &str, timestamp: &str, text: &str) -> Message {
        Message {
            user: Option::Some(user.into()),
            json_type: "message".into(),
            ts: timestamp.into(),
            client_msg_id: Option::None,
            text: text.into(),
            team: Option::None,
            user_team: Option::None,
            source_team: Option::None,
            user_profile: Option::None,
            thread_ts: Option::None,
            parent_user_id: Option::None,
            attachments: Option::None,
            blocks: Option::None,
        }
    }

    /// Returns the timestamp of the message as a `chrono::DateTime<Utc>`.
    /// We ignore the partial seconds of the timestamp, as we are interested in longer time scales.
    pub fn time(&self) -> chrono::DateTime<chrono::Utc> {
        let seconds: i64 = self
            .ts
            .split(".")
            .next()
            .expect("Invalid timestamp format.")
            .parse::<i64>()
            .expect("First part of timestamp is not an integer.");
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    /// Checks if the message contains a given pattern in its text or in any of its `MessageAttachment`s.
    pub fn contains(&self, pattern: &str) -> bool {
        if self.text.contains(pattern) {
            return true;
        }
        for attachment in self.attachments.iter().flatten() {
            if attachment.contains(pattern) {
                return true;
            }
        }
        for block in self.blocks.iter().flatten() {
            if block.contains(pattern) {
                return true;
            }
        }
        return false;
    }
}

/// Represents a message attachment, part of a Slack `Message`.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MessageAttachment {
    id: Option<u64>,
    text: Option<String>,
}
impl MessageAttachment {
    /// Returns true if the attachment text contains the given pattern.
    pub fn contains(&self, pattern: &str) -> bool {
        if let Some(text) = &self.text {
            return text.contains(pattern);
        }
        false
    }
}

/// Represents a message block, part of a Slack `Message`. Blocks can be nested.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MessageBlock {
    #[serde(rename = "type")]
    json_type: String,
    block_id: Option<String>,
    text: Option<String>,
    elements: Option<Vec<MessageBlock>>,
}
impl MessageBlock {
    /// Returns true if block (or any sub-block) contains the given pattern in its text.
    pub fn contains(&self, pattern: &str) -> bool {
        if let Some(text) = &self.text {
            return text.contains(pattern);
        }
        if let Some(elements) = &self.elements {
            for element in elements {
                if element.contains(pattern) {
                    return true;
                }
            }
        }
        false
    }
}

/// Represents a message in a channel.
///
/// Channels can only be inferred from the file path in the ZIP,
/// so this needs to be added to a message after reading the file.
#[derive(Debug)]
pub struct MessageInChannel {
    pub channel: String,
    pub message: Message,
}
impl MessageInChannel {
    pub fn new(channel: &str, message: Message) -> MessageInChannel {
        MessageInChannel {
            channel: channel.into(),
            message,
        }
    }
}

fn read_file(file_name: &str, file_content: &str) -> Vec<Message> {
    match serde_json::from_str(file_content) {
        Ok(x) => x,
        Err(x) => {
            eprint!("Could not deserialize '{}': {}.", file_name, x.to_string());
            Vec::new()
        }
    }
}

/// Read ZIP contents.
pub fn read_zip_contents(zip_path: &PathBuf) -> Vec<MessageInChannel> {
    let file = File::open(zip_path).expect("Cannot open file");
    let mut archive: ZipArchive<File> = ZipArchive::new(file).expect("ZIP file invalid.");
    let mut result: Vec<MessageInChannel> = Vec::new();
    println!("Number of files in archive: {}", archive.len());
    let mut counter: u32 = 0;

    for i in 0..archive.len() {
        let mut file: zip::read::ZipFile<'_, File> =
            archive.by_index(i).expect("ZIP file invalid.");
        if !file.is_dir() && JSON_FILE_NAME.is_match(file.name()) {
            counter += 1;
            println!("Analyzing file #{}: {}", counter, file.name());
            let mut buffer: String = String::new();
            let read_result = file.read_to_string(&mut buffer);
            if read_result.is_ok() {
                let messages: Vec<Message> = read_file(file.name(), buffer.as_str());
                println!(
                    "Read {:?} bytes into {} messages.",
                    read_result.unwrap_or(0),
                    messages.len()
                );
                let messages_in_channel: Vec<MessageInChannel> = messages
                    .into_iter()
                    .map(|x| MessageInChannel::new(file.name(), x))
                    .collect();
                result.extend(messages_in_channel);
            }
        }
    }
    println!(
        "Read {} messages from {} files in archive at '{}', sorting by time.",
        result.len(),
        counter,
        zip_path.to_str().unwrap()
    );
    let mut sorted_results: Vec<MessageInChannel> = result.into_iter().collect();
    sorted_results.sort_by_key(|x| x.message.time().timestamp_micros());
    return sorted_results;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_to_datetime_ok() {
        let msg1 = Message::new("tester", "123.456", "");
        assert_eq!(
            msg1.time(),
            Utc.with_ymd_and_hms(1970, 1, 1, 0, 2, 3).unwrap()
        );
        let msg2 = Message::new("tester", "1234567", "");
        assert_eq!(
            msg2.time(),
            Utc.with_ymd_and_hms(1970, 1, 15, 6, 56, 7).unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "First part of timestamp is not an integer")]
    fn ts_to_datetime_err() {
        let invalid_time = Message::new("tester", "", "");
        invalid_time.time();
    }
}
