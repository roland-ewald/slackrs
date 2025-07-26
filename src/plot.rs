use csv::Writer;
use plotters::prelude::*;
use serde::Deserialize;
use std::{collections::HashSet, error::Error, fs, path::PathBuf};

const DEFAULT_IMAGE_DIM: (u32, u32) = (2048, 1024);

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Metric {
    MentionCount {
        channel_pattern: String,
        message_pattern: String,
    },
    StringMessageCountRatio {
        channel_pattern: String,
        message_pattern1: String,
        message_pattern2: String,
    },
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum TimeResolution {
    Daily,
    Monthly,
    Yearly,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct PlotTask {
    pub metric: Metric,
    pub resolution: TimeResolution,
    pub output_file_name: String,
    pub colors: Option<Vec<String>>,
}
impl PlotTask {
    fn rgb_from_hex(hex_str: &str) -> Result<RGBColor, Box<dyn Error>> {
        let hex = hex_str.trim_start_matches('#');
        if hex.len() != 6 {
            return Err("RGB color string must be 6 characters long (e.g., 'FFFFFF')".into());
        }
        let r = u8::from_str_radix(&hex[0..2], 16)?;
        let g = u8::from_str_radix(&hex[2..4], 16)?;
        let b = u8::from_str_radix(&hex[4..6], 16)?;
        Ok(RGBColor(r, g, b))
    }
    pub fn custom_color(&self, index: usize) -> RGBColor {
        if let Some(colors) = &self.colors {
            if index < colors.len() {
                return PlotTask::rgb_from_hex(&colors[index]).unwrap_or(BLUE);
            }
        }
        BLUE
    }
    pub fn with_output_dir(&self, output_dir: &PathBuf) -> PlotTask {
        PlotTask {
            output_file_name: output_dir.join(&self.output_file_name).to_str().unwrap().to_string(),
            ..self.clone()
        }
    }
}

pub fn read_tasks_from_file(file_path: &str, output_dir: &PathBuf) -> Result<Vec<PlotTask>, Box<dyn Error>> {
    let file_content = fs::read_to_string(file_path)?;
    let tasks: Vec<PlotTask> = serde_json::from_str(&file_content)?;
    let tasks_with_output_dir: Vec<PlotTask> = tasks.iter().map(|task| {
        task.with_output_dir(output_dir)
    }).collect();
    Ok(tasks_with_output_dir)
}

fn calculate_max_y_axis(message_counts: &Vec<(String, usize)>) -> usize {
    (message_counts.iter().map(|x| x.1).max().unwrap_or(0) as f64 * 1.1) as usize
}

fn calculate_time_series_ratios(
    labels: &Vec<String>,
    message_counts1: &Vec<(String, usize)>,
    message_counts2: &Vec<(String, usize)>,
) -> Vec<(String, f64)> {
    labels
        .iter()
        .zip(message_counts1.iter())
        .zip(message_counts2.iter())
        .map(|((label, (_, count1)), (_, count2))| {
            let ratio = if *count2 + *count1 == 0 {
                0.0
            } else {
                *count1 as f64 / (*count1 + *count2) as f64
            };
            (label.clone(), ratio)
        })
        .collect()
}

fn label_set(message_counts: &Vec<(String, usize)>) -> HashSet<String> {
    message_counts
        .iter()
        .map(|(label, _count)| (label.clone()))
        .collect()
}

fn consolidate_labels(
    message_counts1: Vec<(String, usize)>,
    message_counts2: Vec<(String, usize)>,
) -> (Vec<(String, usize)>, Vec<(String, usize)>) {
    let labels1: HashSet<String> = label_set(&message_counts1);
    let labels2: HashSet<String> = label_set(&message_counts2);
    let shared_labels: HashSet<String> = labels1.intersection(&labels2).cloned().collect();
    let filtered_message_counts1 = message_counts1
        .into_iter()
        .filter(|(label, _value)| shared_labels.contains(label))
        .collect();
    let filtered_message_counts2 = message_counts2
        .into_iter()
        .filter(|(label, _value)| shared_labels.contains(label))
        .collect();
    (filtered_message_counts1, filtered_message_counts2)
}

fn write_message_counts_to_csv(
    description: Option<&str>,
    output_file_name: &str,
    message_counts: &Vec<(String, usize)>,
) -> Result<(), Box<dyn Error>> {
    let csv_output_file_name: String = description.map_or_else(
        || String::from(output_file_name) + ".csv",
        |desc| String::from(output_file_name) + "-" + desc + ".csv",
    );

    #[cfg(debug_assertions)]
    dbg!(format!(
        "Writing message counts to '{}'.",
        &csv_output_file_name
    ));

    let mut wtr = Writer::from_path(csv_output_file_name)?;
    for (name, count) in message_counts.iter() {
        wtr.serialize((name, count))?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn counter_plot(
    task: &PlotTask,
    message_pattern: &str,
    message_counts: &Vec<(String, usize)>,
) -> Result<(), Box<dyn Error>> {
    println!(
        "Plotting {} messages mentioning '{}' to '{}'.",
        message_counts.len(),
        message_pattern,
        task.output_file_name
    );
    let max_y_axis: usize = calculate_max_y_axis(message_counts);
    let labels: Vec<String> = message_counts
        .iter()
        .map(|(time_label, _)| time_label.clone())
        .collect();

    write_message_counts_to_csv(Option::None, &task.output_file_name, message_counts)?;

    let root = BitMapBackend::new(&task.output_file_name, DEFAULT_IMAGE_DIM).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .margin(20)
        .caption(
            format!("Slack messages mentioning '{}' over time", message_pattern),
            ("sans-serif", 30).into_font(),
        )
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(labels.into_segmented(), 0..max_y_axis)?;

    chart
        .configure_mesh()
        .x_label_style(("sans-serif", 25).into_text_style(&root))
        .y_label_style(("sans-serif", 25).into_text_style(&root))
        .draw()?;

    chart
        .draw_series(
            Histogram::vertical(&chart)
                .margin(calculate_margin(0.2, labels.len()))
                .style(task.custom_color(0).filled())
                .data(
                    labels
                        .iter()
                        .zip(message_counts.iter())
                        .map(|(label, (_, count))| (label, *count)),
                ),
        )
        .unwrap();
    root.present()?;
    Ok(())
}

pub fn ratio_plot(
    task: &PlotTask,
    message_pattern1: &str,
    msg_counts1: &Vec<(String, usize)>,
    message_pattern2: &str,
    msg_counts2: &Vec<(String, usize)>,
) -> Result<(), Box<dyn Error>> {
    let (message_counts1, message_counts2) =
        consolidate_labels(msg_counts1.clone(), msg_counts2.clone());
    let shared_labels: Vec<String> = message_counts1
        .iter()
        .map(|(label, _)| label.clone())
        .collect();
    println!(
        "Plotting ratio between {} (mentioning '{}') and {} messages (mentioning '{}') to '{}'.",
        message_counts1.len(),
        message_pattern1,
        message_counts2.len(),
        message_pattern2,
        task.output_file_name,
    );

    write_message_counts_to_csv(
        Option::Some("counts-pattern1"),
        &task.output_file_name,
        &message_counts1,
    )?;
    write_message_counts_to_csv(
        Option::Some("counts-pattern2"),
        &task.output_file_name,
        &message_counts2,
    )?;

    let time_series: Vec<(String, f64)> =
        calculate_time_series_ratios(&shared_labels, &message_counts1, &message_counts2);

    #[cfg(debug_assertions)]
    dbg!(format!(
        "Time series for file '{}' has {} elements.",
        &task.output_file_name,
        time_series.len()
    ));

    let line_series_data: Vec<(usize, f64)> = time_series
        .iter()
        .enumerate() // Gives you (index, &(String, f64))
        .map(|(i, (_, val))| (i, *val)) // Map to (index, f64)
        .collect();
    let max_y_axis: f64 = time_series
        .iter()
        .map(|x| x.1)
        .fold(0.0, |acc: f64, x| acc.max(x))
        * 1.1;

    let root = BitMapBackend::new(&task.output_file_name, DEFAULT_IMAGE_DIM).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .margin(calculate_margin(0.1, message_counts1.len()))
        .caption(
            format!(
                "Slack ratio between '{}' and '{}' over time",
                message_pattern1, message_pattern2
            ),
            ("sans-serif", 30).into_font(),
        )
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0..(shared_labels.len() - 1), 0.0..max_y_axis)?;

    chart
        .configure_mesh()
        .x_label_style(("sans-serif", 25).into_text_style(&root))
        .y_label_style(("sans-serif", 25).into_text_style(&root))
        .x_label_formatter(&|x| {
            let index: usize = *x;
            if index < shared_labels.len() {
                shared_labels[index].clone()
            } else {
                String::from("")
            }
        })
        .draw()?;
    chart.draw_series(LineSeries::new(line_series_data, task.custom_color(0)))?;
    root.present()?;
    Ok(())
}

fn calculate_margin(ratio: f64, num_labels: usize) -> u32 {
    (ratio * ((DEFAULT_IMAGE_DIM.0 as f64 * 0.9) / (num_labels as f64))) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_analysis_tasks_from_file() {
        let file_path = "tests/resources/plot_tasks.json";
        let tasks = read_tasks_from_file(file_path, &PathBuf::from("./tests/output")).expect("Failed to read tasks");

        assert_eq!(tasks.len(), 2);
        match &tasks[0].metric {
            Metric::MentionCount {
                channel_pattern,
                message_pattern,
            } => {
                assert_eq!(channel_pattern, "");
                assert_eq!(message_pattern, "@group");
            }
            _ => panic!("Unexpected metric type"),
        }
        assert_eq!(tasks[0].resolution, TimeResolution::Daily);
        assert_eq!(tasks[0].output_file_name, "./tests/output/group-mentions.png");
        assert_eq!(tasks[0].colors, 
            Some(vec!["#e27505".to_string(), "#55332c".to_string(), "#505050".to_string()]));

        match &tasks[1].metric {
            Metric::StringMessageCountRatio {
                channel_pattern,
                message_pattern1,
                message_pattern2,
            } => {
                assert_eq!(channel_pattern, "sample");
                assert_eq!(message_pattern1, "special");
                assert_eq!(message_pattern2, "message");
            }
            _ => panic!("Unexpected metric type"),
        }
        assert_eq!(tasks[1].resolution, TimeResolution::Daily);
        assert_eq!(tasks[1].output_file_name, "./tests/output/sample-rate.png");
        assert_eq!(tasks[1].colors, None);
    }

    #[test]
    fn test_consolidate_labels() {
        let (result1, result2) = consolidate_labels(
            vec![("2024-03".to_string(), 1), ("2024-04".to_string(), 2)],
            vec![("2024-04".to_string(), 3)],
        );
        assert_eq!(
            result1,
            vec![("2024-04".to_string(), 2)],
            "Consolidated labels for message counts 1"
        );
        assert_eq!(
            result2,
            vec![("2024-04".to_string(), 3)],
            "Consolidated labels for message counts 2"
        );

        let (result3, result4) = consolidate_labels(
            vec![("2024-04".to_string(), 4)],
            vec![("2024-03".to_string(), 5), ("2024-04".to_string(), 6)],
        );
        assert_eq!(
            result3,
            vec![("2024-04".to_string(), 4)],
            "Consolidated labels for message counts 3"
        );
        assert_eq!(
            result4,
            vec![("2024-04".to_string(), 6)],
            "Consolidated labels for message counts 4"
        );
    }

    #[test]
    fn test_rgb_from_hex() {
        let result = PlotTask::rgb_from_hex("#007f94");
        assert_eq!(result.unwrap(), RGBColor(0, 127, 148), "RGB color from hex");
    }

    #[test]
    fn test_rgb_from_hex_invalid() {
        assert!(PlotTask::rgb_from_hex("#007f9").is_err()); // Invalid length
    }
}
