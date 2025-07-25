use slackrs::{plot, plot::PlotTask, slack, slack::MessageInChannel};
use std::path::PathBuf;
use std::fs;

fn main() {
    let output_dir= &PathBuf::from("./target/example-output");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");
    let tasks: Vec<PlotTask> = plot::read_tasks_from_file(
        "tests/resources/plot_tasks.json",
        output_dir,
    )
    .expect("Failed to read tasks from sample file");
    let messages: Vec<MessageInChannel> =
        slack::read_zip_contents(&PathBuf::from("tests/resources/sample_export.zip"));
    let _ = slackrs::process_tasks(&tasks, &messages);
}
