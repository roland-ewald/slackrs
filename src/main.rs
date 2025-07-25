/// slackrs: a simple command-line tool to create plots from Slack data exports.
use clap::Parser;
use slackrs::{plot, slack, plot::PlotTask, slack::MessageInChannel};
use std::{fs, io::Error, io::ErrorKind, path::PathBuf, result::Result};

#[derive(Parser)]
struct Cli {
    #[arg(
        short = 'i',
        long = "input-file",
        help = "The input file to analyze, in the ZIP format provided by Slack's export."
    )]
    input_file: PathBuf,

    #[arg(
        short = 'o',
        long = "output-dir",
        default_value = "./output",
        help = "The output directory for the plots."
    )]
    output_dir: PathBuf,

    #[arg(
        short = 'c',
        long = "task-file",
        default_value = "tasks.json",
        help = "The JSON file with the tasks to run (see README for examples)."
    )]
    task_file: PathBuf,
}

impl Cli {
    fn validate(self: &Cli) -> Result<(), Error> {
        if !self.input_file.is_file() {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("The input file '{:?}' is not a file.", self.input_file),
            ))
        } else if !self.task_file.is_file() {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("The task file '{:?}' is not a file.", self.task_file),
            ))
        } else if self.output_dir.is_file() {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("The output directory '{:?}' is a file.", self.output_dir),
            ))
        } else if !self.output_dir.exists() {
            println!(
                "Creating output directory '{:?}', as it does not yet exist.",
                self.output_dir
            );
            fs::create_dir_all(&self.output_dir)
        } else {
            Ok(())
        }
    }
}

fn main() {
    let args = Cli::parse();
    let validation_result = args.validate();
    if validation_result.is_err() {
        eprintln!(
            "Stopping, as input parameters are invalid: '{:?}'.",
            validation_result.err()
        );
    } else {
        // Start with reading tasks file, as this is faster and more likely to fail
        let tasks: Vec<PlotTask> = plot::read_tasks_from_file(args.task_file.to_str().unwrap())
        .expect("Failed to read tasks from file");
        println!(
            "Found {} tasks in task file '{:?}'.",
            tasks.len(),
            args.task_file.file_name().unwrap()
        );

        let messages: Vec<MessageInChannel> = slack::read_zip_contents(&args.input_file);
        let _ = slackrs::process_tasks(&tasks, &messages);
        println!("Done.");
    }
}

