use std::fs::{self, File};
use std::io::{prelude::*, BufReader, BufWriter, LineWriter};
use std::thread;

use chrono::{NaiveTime, Utc, Weekday};
use flate2::write::GzDecoder;
use inquire::{DateSelect, Password, Select, Text};
use reqwest::{header, Client, Method};

const PAPERTRAIL_URL: &str = "https://papertrailapp.com/api/v1/archives/YYYY-MM-DD-HH/download";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options: Vec<&str> = vec!["Retrieve logs", "Filter logs"];
    let ans: &str = Select::new("What do you want to do?", options).prompt()?;

    if ans == "Retrieve logs" {
        let token = Password::new("Papertrail api token?")
            .without_confirmation()
            .prompt()?;
        let selected_date = DateSelect::new("What day do you want to download?")
            .with_starting_date(Utc::now().date_naive())
            .with_week_start(Weekday::Mon)
            .prompt()?;
        let date_format = selected_date.format("%Y-%m-%d").to_string();
        match fs::create_dir(&date_format) {
            Ok(_) => {}
            Err(_) => {
                println!("Dir already exits")
            }
        }
        let (start_hour, end_hour) = get_start_and_end()?;

        let mut headers = header::HeaderMap::new();
        headers.insert("X-Papertrail-Token", header::HeaderValue::from_str(&token)?);
        let client = reqwest::Client::builder()
            .gzip(true)
            .default_headers(headers)
            .build()?;
        for hour in start_hour..end_hour {
            let time_format = NaiveTime::from_hms_milli_opt(hour, 0, 0, 0)
                .unwrap()
                .format("-%H")
                .to_string();
            let archive = date_format.to_string() + &time_format;
            let path = format!("./{}/{}.tsv", date_format, archive);
            match retrive_log(&client, archive.to_string(), path).await {
                Ok(_) => (),
                Err(_) => println!("Failed to find {}", archive),
            };
        }
    } else {
        filter_logs()?;
    }

    Ok(())
}

//Schema:
// id ?
// generated_at date
// received_at date
// source_id i32
// source_name str
// source_ip str
// facility_name str
// severity_name str
// program str
// message str
struct Line {
    id: u64,
    line: String,
}

// Parallelize https://stackoverflow.com/questions/51044467/how-can-i-perform-parallel-asynchronous-http-get-requests-with-reqwest
async fn retrive_log(client: &Client, archive: String, path: String) -> anyhow::Result<()> {
    println!("Dowloading archive for: {}", archive);
    let log_file = client
        .request(
            Method::GET,
            PAPERTRAIL_URL.replace("YYYY-MM-DD-HH", &archive),
        )
        .send()
        .await?;
    let file = File::create(path)?;
    let mut content = log_file.bytes().await?;
    let mut reader = BufWriter::new(GzDecoder::new(file));
    reader.write_all(&mut content)?;
    reader.flush()?;
    Ok(())
}

fn get_start_and_end() -> anyhow::Result<(u32, u32)> {
    let possible_starts = (0..24).collect::<Vec<u32>>();
    let start: u32 = Select::new("What start hour would you like?", possible_starts).prompt()?;
    let possible_ends = (start..24).collect::<Vec<u32>>();
    let end: u32 = Select::new("What end hour would you like?", possible_ends).prompt()?;

    Ok((start, end))
}

fn filter_logs() -> anyhow::Result<()> {
    let selected_date = DateSelect::new("What day do you want to filter?")
        .with_starting_date(Utc::now().date_naive())
        .with_week_start(Weekday::Mon)
        .prompt()?;
    let contain_string = Text::new("What would like the line to contain?").prompt()?;
    let date_format = selected_date.format("%Y-%m-%d").to_string();
    let file = File::create(format!("{}.tsv", Utc::now().format("%Y_%m_%d_%H_%M")))?;
    let mut exit_file = LineWriter::new(file);
    let dirs = fs::read_dir(date_format.to_string())?;
    let mut thread_handles: Vec<_> = Vec::new();
    for dir in dirs {
        let contain_string = contain_string.to_string();
        let handle = thread::spawn(move || {
            // some work here
            let file = File::open(dir.unwrap().path()).unwrap();
            let reader = BufReader::new(file);
            reader
                .lines()
                .filter_map(|line| {
                    let line = line.unwrap();
                    if line.contains(&contain_string) {
                        let (id, _) = line.split_once('\t').unwrap();
                        Some(Line {
                            id: id.parse::<u64>().unwrap(),
                            line,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<Line>>()
        });
        thread_handles.push(handle);
    }
    // some work here
    let mut lines: Vec<Line> = Vec::new();
    //How do you handle the ordering?
    for handle in thread_handles {
        let mut thread_lines = handle.join().unwrap();
        lines.append(&mut thread_lines)
    }
    lines.sort_by(|a, b| a.id.cmp(&b.id));
    exit_file.write(
        lines
            .into_iter()
            .map(|l| l.line)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )?;

    exit_file.flush()?;
    Ok(())
}
