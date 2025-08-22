use plot::{PlotTask, TimeResolution};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slack::MessageInChannel;
use std::{io::Error, result::Result};

/// Slack JSON data structures and parsing
pub mod slack;

/// Plotting utilities
pub mod plot;

pub fn process_tasks(
    tasks: &[PlotTask],
    messages: &[MessageInChannel],
) -> Result<(), Error> {
    tasks.par_iter().for_each(|task| {
        println!("Task: {:?}", task);
            match task.metric {
                plot::Metric::MentionCount {
                    ref channel_pattern,
                    ref message_pattern,
                } => {
                    let message_counts = filter_and_count_messages(
                        &messages,
                        channel_pattern,
                        message_pattern,
                        &task.resolution,
                    );
                    plot::counter_plot(&task, &message_pattern, &message_counts)
                        .expect("Image generation failed.");
                }
                plot::Metric::StringMessageCountRatio {
                    ref channel_pattern,
                    ref message_pattern1,
                    ref message_pattern2,
                } => {
                    let message_counts1 = filter_and_count_messages(
                        &messages,
                        channel_pattern,
                        message_pattern1,
                        &task.resolution,
                    );
                    let message_counts2 = filter_and_count_messages(
                        &messages,
                        channel_pattern,
                        message_pattern2,
                        &task.resolution,
                    );

                    plot::ratio_plot(
                        &task,
                        &message_pattern1,
                        &message_counts1,
                        &message_pattern2,
                        &message_counts2,
                    )
                    .expect("Image generation failed.");
                }
            }
    });
    Ok(())
}

fn filter_and_count_messages(
    messages: &[MessageInChannel],
    channel_pattern: &str,
    message_pattern: &str,
    resolution: &TimeResolution,
) -> Vec<(String, usize)> {
    let messages_to_plot: Vec<&MessageInChannel> = messages
        .iter()
        .filter(|x| x.channel.contains(channel_pattern) && x.message.contains(message_pattern))
        .collect();
    println!("Found {} messages matching '{}'.", messages_to_plot.len(), message_pattern);
    group_messages_by_time(&messages_to_plot, resolution)
}

/// Group messages by `TimeResolution` and count them.
fn group_messages_by_time(
    messages_to_plot: &Vec<&MessageInChannel>,
    resolution: &TimeResolution,
) -> Vec<(String, usize)> {
    let mut message_counts: Vec<(String, usize)> = Vec::new();
    let mut last_count: usize = 0;
    let mut last_label: String = "".to_string();
    for (index, message) in messages_to_plot.iter().enumerate() {
        let time_label = time_by_resolution(message, resolution);
        if index == 0 {
            last_count = 1;
            last_label = time_label.clone();
        } else if time_label == last_label {
            last_count += 1;
        }
        if time_label != last_label {
            message_counts.push((last_label.clone(), last_count));
            last_count = 1;
            last_label = time_label.clone();
        }
        if index == messages_to_plot.len() - 1 {
            message_counts.push((last_label.clone(), last_count));
        }
    }
    
    message_counts
}

/// Convert the message time to a string based on the `TimeResolution`.
fn time_by_resolution(msg: &MessageInChannel, resolution: &TimeResolution) -> String {
    return match resolution {
        TimeResolution::Daily => msg.message.time().format("%Y-%m-%d").to_string(),
        TimeResolution::Monthly => msg.message.time().format("%Y-%m").to_string(),
        TimeResolution::Yearly => msg.message.time().format("%Y").to_string(),
    };
}