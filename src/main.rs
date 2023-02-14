#![feature(fs_try_exists)]
use std::fs::{File, self};
use std::io::{prelude::*, BufReader, LineWriter, BufWriter};

use chrono::{Utc, Weekday, NaiveTime};
use flate2::write::GzDecoder;
use inquire::{DateSelect, Select};
use reqwest::{Client, Method, header};

const PAPERTRAIL_URL: &str = "https://papertrailapp.com/api/v1/archives/YYYY-MM-DD-HH/download";
const PAPERTRAIL_TOKEN: &str = "<INSERT_TOKEN>";

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let options: Vec<&str> = vec!["Retrieve logs", "Filter logs"];

    let ans: &str = Select::new("What's your favorite fruit?", options).prompt()?;

    if ans == "Retrieve logs" {
        let selected_date = DateSelect::new("What day do you want to download?")
            .with_starting_date(Utc::now().date_naive())
            .with_week_start(Weekday::Mon)
            .prompt()?;
        let date_format = selected_date.format("%Y-%m-%d").to_string();
        match fs::try_exists(&date_format) {
            Ok(exists) => {
                if !exists {
                    fs::create_dir(&date_format)?;
                }
            }
            Err(_) => {
                fs::create_dir(&date_format)?;
            }
        }
        let mut headers = header::HeaderMap::new();
        headers.insert("X-Papertrail-Token", header::HeaderValue::from_static(PAPERTRAIL_TOKEN));
        let client = reqwest::Client::builder().gzip(true).default_headers(headers).build()?;
        for hour in 0..24 {
            let time_format = NaiveTime::from_hms_milli_opt(hour, 0, 0, 0).unwrap().format("-%H").to_string();
            let archive = date_format.to_string() + &time_format;
            let path = format!("./{}/{}.tsv", date_format, archive);
            match retrive_log(&client, archive.to_string(), path).await {
                Ok(_) => (),
                Err(_) => println!("Failed to find {}", archive)
            };
            // let date_format = date_format.to_string();
            // let new_client = client.clone();
            // tokio::spawn(async move { async_function(new_client, hour, date_format).await });
        }
    }
    else {
        filter_logs()?;
    }

    Ok(())
}
// async fn async_function(client: Client, hour: u32, date_format: String) {
//     let time_format = NaiveTime::from_hms_milli_opt(hour, 0, 0, 0).unwrap().format("-%H").to_string();
//     let archive = date_format.to_string() + &time_format;
//     let path = format!("./{}/{}.tsv", date_format, archive);
//     match retrive_log(&client, archive.to_string(), path).await {
//         Ok(_) => (),
//         Err(_) => println!("Failed to find {}", archive)
//     };
// }

async fn retrive_log(client: &Client, archive: String, path: String) -> anyhow::Result<()> {
    println!("Dowloading archive for: {}", archive);
    let log_file = client
        .request(Method::GET, PAPERTRAIL_URL.replace("YYYY-MM-DD-HH", &archive))
        .send()
        .await?;
    let file = File::create(path)?;
    let mut content = log_file.bytes().await?;
    let mut reader = BufWriter::new(GzDecoder::new(file));
    reader.write_all(&mut content)?;
    reader.flush()?;
    Ok(())
}

fn filter_logs() -> anyhow::Result<()> {
    let selected_date = DateSelect::new("What day do you want to filter?")
        .with_starting_date(Utc::now().date_naive())
        .with_week_start(Weekday::Mon)
        .prompt()?;
    let date_format = selected_date.format("%Y-%m-%d").to_string();
    let dirs = fs::read_dir(date_format.to_string())?;
    let file = File::create(format!("{}_filtered.tsv", date_format))?;
    let mut exit_file = LineWriter::new(file);
    for dir in dirs {
        let file = File::open(dir?.path())?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if line.contains("904bc338-a438-416a-b3ca-a0c93254afbb") && !line.contains("/mapdata") {
                exit_file.write_fmt(format_args!("{}\n", line))?
            }
        }
        exit_file.flush()?;
    }
    
    Ok(())
}
